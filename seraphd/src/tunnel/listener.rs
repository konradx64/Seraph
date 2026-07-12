//! QUIC Tunnel Listener
//!
//! Bootstraps a Quinn QUIC endpoint on UDP port 7700.
//! All connections must present a valid client certificate signed by the
//! Seraph Tunnel CA (mTLS). Unauthenticated or unknown clients are rejected
//! at the TLS handshake before any application data is exchanged.

use anyhow::{Context, Result};
use quinn::{Endpoint, Incoming, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::RootCertStore;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use pingora::services::background::BackgroundService;
use async_trait::async_trait;

use super::ca::TunnelCa;

/// Default UDP port for the QUIC tunnel listener.
pub const TUNNEL_PORT: u16 = 7700;

pub struct QuicTunnelService {
    state: Arc<crate::state::AppState>,
}

impl QuicTunnelService {
    pub fn new(state: Arc<crate::state::AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl BackgroundService for QuicTunnelService {
    async fn start(&self, mut shutdown: pingora::server::ShutdownWatch) {
        let tunnels_dir = std::path::PathBuf::from("tunnels");
        let _ = std::fs::create_dir_all(&tunnels_dir);

        let tunnel_addr: std::net::SocketAddr = "0.0.0.0:7700".parse().unwrap();
        match TunnelListener::bind(tunnel_addr, &self.state.ca) {
            Ok(tunnel_listener) => {
                tokio::select! {
                    _ = tunnel_listener.run(tunnels_dir, self.state.clone()) => {}
                    _ = shutdown.changed() => {
                        tracing::info!("QUIC tunnel background service received shutdown signal");
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind QUIC tunnel endpoint: {:?}", e);
            }
        }
    }
}

/// Wraps the Quinn endpoint and exposes an `accept` method.
pub struct TunnelListener {
    endpoint: Endpoint,
}

impl TunnelListener {
    /// Create a new QUIC endpoint and start listening.
    pub fn bind(addr: SocketAddr, ca: &TunnelCa) -> Result<Self> {
        let server_config = build_server_config(ca)
            .context("Failed to build QUIC server TLS config")?;

        let endpoint = Endpoint::server(server_config, addr)
            .context("Failed to bind QUIC endpoint")?;

        tracing::info!("Tunnel QUIC listener bound on {}", addr);
        Ok(Self { endpoint })
    }

    /// Spawn a long-running task that accepts and dispatches tunnel connections.
    pub async fn run(self, tunnels_dir: PathBuf, state: Arc<crate::state::AppState>) {
        tracing::info!("Tunnel listener running");
        while let Some(incoming) = self.endpoint.accept().await {
            let dir = tunnels_dir.clone();
            let state_clone = state.clone();
            tokio::spawn(handle_connection(incoming, dir, state_clone));
        }
        tracing::warn!("Tunnel listener stopped accepting connections");
    }
}

// ---------------------------------------------------------------------------
// Connection handler
// ---------------------------------------------------------------------------

async fn handle_connection(incoming: Incoming, _tunnels_dir: PathBuf, state: Arc<crate::state::AppState>) {
    let remote = incoming.remote_address();

    match incoming.await {
        Ok(conn) => {
            if let Some(agent_id) = extract_agent_id(&conn) {
                tracing::info!("Tunnel agent '{}' connected from {}", agent_id, remote);

                // Register connection in active tunnels map
                {
                    let mut tunnels = state.active_tunnels.write().await;
                    tunnels.insert(agent_id.clone(), conn.clone());
                }
                let _ = state.events.send(crate::event::Event::TunnelConnected { id: agent_id.clone() });

                // Keep connection alive, wait until disconnected
                let _ = conn.closed().await;

                // Deregister connection
                {
                    let mut tunnels = state.active_tunnels.write().await;
                    tunnels.remove(&agent_id);
                }
                let _ = state.events.send(crate::event::Event::TunnelDisconnected { id: agent_id.clone() });

                tracing::info!("Tunnel agent '{}' disconnected", agent_id);
            } else {
                tracing::warn!("Rejecting connection from {}: could not extract agent ID", remote);
            }
        }
        Err(e) => {
            tracing::warn!("Tunnel connection from {} rejected: {}", remote, e);
        }
    }
}

// ---------------------------------------------------------------------------
// Dynamic Route-specific Unix Socket Listener Spawning
// ---------------------------------------------------------------------------

pub fn ensure_route_listener(
    state: &Arc<crate::state::AppState>,
    hostname: &str,
    upstream: &str,
    tunnel_id: &str,
    tunnels_dir: &std::path::Path,
) -> Result<()> {
    {
        let mut listeners = state.active_route_listeners.lock().unwrap();
        if listeners.contains(hostname) {
            return Ok(());
        }
        listeners.insert(hostname.to_string());
    }

    let socket_path = tunnels_dir.join(format!("route-{}.sock", hostname));
    let _ = std::fs::remove_file(&socket_path);

    let listener = tokio::net::UnixListener::bind(&socket_path)
        .context("Failed to bind route Unix socket")?;

    let state_clone = state.clone();
    let hostname_clone = hostname.to_string();
    let upstream_clone = upstream.to_string();
    let tunnel_id_clone = tunnel_id.to_string();
    let socket_path_clone = socket_path.clone();

    tokio::spawn(async move {
        tracing::info!("Started Unix listener for route '{}' targeting upstream '{}' via tunnel '{}'", hostname_clone, upstream_clone, tunnel_id_clone);

        while let Ok((mut local_stream, _)) = listener.accept().await {
            let state = state_clone.clone();
            let tunnel_id = tunnel_id_clone.clone();
            let upstream = upstream_clone.clone();

            tokio::spawn(async move {
                // Fetch the active QUIC connection for this tunnel_id
                let conn_opt = {
                    let tunnels = state.active_tunnels.read().await;
                    tunnels.get(&tunnel_id).cloned()
                };

                if let Some(conn) = conn_opt {
                    match conn.open_bi().await {
                        Ok((mut send_stream, recv_stream)) => {
                            let header = format!("{}\n", upstream);
                            if let Err(e) = send_stream.write_all(header.as_bytes()).await {
                                tracing::error!("Failed to write upstream header to tunnel stream: {}", e);
                                return;
                            }

                            // Wrap QUIC streams in byte counters
                            let mut counting_recv = CountingReader {
                                inner: recv_stream,
                                tunnel_id: tunnel_id.clone(),
                                state: state.clone(),
                            };
                            let mut counting_send = CountingWriter {
                                inner: send_stream,
                                tunnel_id: tunnel_id.clone(),
                                state: state.clone(),
                            };

                            // Bridge the local connection and the QUIC stream
                            let (mut local_read, mut local_write) = local_stream.split();
                            let bridge_read = tokio::io::copy(&mut counting_recv, &mut local_write);
                            let bridge_write = tokio::io::copy(&mut local_read, &mut counting_send);

                            let _ = tokio::join!(bridge_read, bridge_write);
                        }
                        Err(e) => {
                            tracing::error!("Failed to open QUIC stream on tunnel '{}': {}", tunnel_id, e);
                        }
                    }
                } else {
                    tracing::warn!("Tunnel '{}' is offline; dropping local stream", tunnel_id);
                }
            });
        }

        // Cleanup on stop
        let mut listeners = state_clone.active_route_listeners.lock().unwrap();
        listeners.remove(&hostname_clone);
        let _ = std::fs::remove_file(socket_path_clone);
        tracing::info!("Stopped Unix listener for route '{}'", hostname_clone);
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// mTLS — extract the agent ID from the peer certificate CN field
// ---------------------------------------------------------------------------

fn extract_agent_id(conn: &quinn::Connection) -> Option<String> {
    let identity = conn.peer_identity()?;
    let certs = identity.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    let cert_der = certs.first()?;

    // Parse the DER-encoded certificate to extract the Common Name
    let (_, cert) = x509_parser::parse_x509_certificate(cert_der).ok()?;
    cert.subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// TLS configuration builder
// ---------------------------------------------------------------------------

fn build_server_config(ca: &TunnelCa) -> Result<ServerConfig> {
    // Build a new self-signed server keypair for the tunnel endpoint itself
    let server_key = rcgen::KeyPair::generate()?;
    let mut server_cert_params = rcgen::CertificateParams::new(vec!["seraph-tunnel".to_string()])?;
    server_cert_params.distinguished_name.push(rcgen::DnType::CommonName, "Seraph Tunnel Server");

    // Sign the server cert with our CA so agents can verify it
    let server_cert = server_cert_params.signed_by(&server_key, &ca.cert, &ca.key_pair)?;
    let server_cert_der: CertificateDer<'static> = CertificateDer::from(server_cert.der().to_vec());
    let server_key_der = PrivateKeyDer::try_from(server_key.serialize_der())
        .map_err(|e| anyhow::anyhow!("Failed to parse server private key: {}", e))?;

    // Build the CA trust store for verifying agent client certs
    let ca_cert_der = CertificateDer::from(ca.cert.der().to_vec());
    let mut root_store = RootCertStore::empty();
    root_store.add(ca_cert_der).context("Failed to add CA cert to trust store")?;

    // Require client certificates signed by our CA
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()
        .context("Failed to build mTLS client verifier")?;

    let tls_config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(vec![server_cert_der], server_key_der)
        .context("Failed to configure server TLS")?;

    let mut server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)
            .context("Failed to build Quinn server config")?,
    ));

    // Keep idle connections alive for up to 30 seconds
    let transport = Arc::get_mut(&mut server_config.transport)
        .context("Cannot get mutable transport config")?;
    transport.max_idle_timeout(Some(quinn::VarInt::from_u32(30_000).into()));
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(10)));
    transport.max_concurrent_bidi_streams(quinn::VarInt::from_u32(2048).into());

    Ok(server_config)
}

struct CountingReader<R> {
    inner: R,
    tunnel_id: String,
    state: Arc<crate::state::AppState>,
}

impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for CountingReader<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let res = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        if let std::task::Poll::Ready(Ok(())) = &res {
            let after = buf.filled().len();
            let n = after - before;
            if n > 0 {
                self.state.stats.record_tunnel_traffic(&self.tunnel_id, 0, n as u64);
            }
        }
        res
    }
}

struct CountingWriter<W> {
    inner: W,
    tunnel_id: String,
    state: Arc<crate::state::AppState>,
}

impl<W: tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite for CountingWriter<W> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let res = std::pin::Pin::new(&mut self.inner).poll_write(cx, buf);
        if let std::task::Poll::Ready(Ok(n)) = &res {
            if *n > 0 {
                self.state.stats.record_tunnel_traffic(&self.tunnel_id, *n as u64, 0);
            }
        }
        res
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

