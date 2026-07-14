//! QUIC Tunnel Listener
//!
//! Bootstraps a Quinn QUIC endpoint with mTLS authentication.

use anyhow::{Context, Result};
use async_trait::async_trait;
use pingora::services::background::BackgroundService;
use quinn::{Endpoint, Incoming, ServerConfig};
use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use super::ca::TunnelCa;

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

        let tunnel_addr = self
            .state
            .config
            .tunnel_addr
            .parse::<SocketAddr>()
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to parse config tunnel_addr: {:?}. Falling back to 0.0.0.0:7700",
                    e
                );
                "0.0.0.0:7700".parse().unwrap()
            });

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
        let server_config =
            build_server_config(ca).context("Failed to build QUIC server TLS config")?;

        let endpoint =
            Endpoint::server(server_config, addr).context("Failed to bind QUIC endpoint")?;

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

// Accept and dispatch tunnel connections.
async fn handle_connection(
    incoming: Incoming,
    _tunnels_dir: PathBuf,
    state: Arc<crate::state::AppState>,
) {
    let remote = incoming.remote_address();

    match incoming.await {
        Ok(conn) => {
            if let Some(agent_id) = extract_agent_id(&conn) {
                tracing::info!("Tunnel agent '{}' connected from {}", agent_id, remote);

                {
                    let mut tunnels = state.active_tunnels.write().await;
                    tunnels.insert(agent_id.clone(), conn.clone());
                }
                if let Ok(tunnels) = crate::control::tunnels::tunnel_snapshot(&state).await {
                    let _ = state.events.send(crate::event::Event::TunnelConnected {
                        id: agent_id.clone(),
                        tunnels,
                        status: crate::control::tunnels::status_snapshot(&state),
                    });
                }

                let _ = conn.closed().await;

                {
                    let mut tunnels = state.active_tunnels.write().await;
                    tunnels.remove(&agent_id);
                }
                if let Ok(tunnels) = crate::control::tunnels::tunnel_snapshot(&state).await {
                    let _ = state.events.send(crate::event::Event::TunnelDisconnected {
                        id: agent_id.clone(),
                        tunnels,
                        status: crate::control::tunnels::status_snapshot(&state),
                    });
                }

                tracing::info!("Tunnel agent '{}' disconnected", agent_id);
            } else {
                tracing::warn!(
                    "Rejecting connection from {}: could not extract agent ID",
                    remote
                );
            }
        }
        Err(e) => {
            tracing::warn!("Tunnel connection from {} rejected: {}", remote, e);
        }
    }
}

fn extract_agent_id(conn: &quinn::Connection) -> Option<String> {
    let identity = conn.peer_identity()?;
    let certs = identity.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    let cert_der = certs.first()?;

    let (_, cert) = x509_parser::parse_x509_certificate(cert_der).ok()?;
    cert.subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(|s| s.to_string())
}

fn build_server_config(ca: &TunnelCa) -> Result<ServerConfig> {
    let server_key = rcgen::KeyPair::generate()?;
    let mut server_cert_params = rcgen::CertificateParams::new(vec!["seraph-tunnel".to_string()])?;
    server_cert_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Seraph Tunnel Server");

    let server_cert = server_cert_params.signed_by(&server_key, &ca.cert, &ca.key_pair)?;
    let server_cert_der: CertificateDer<'static> = CertificateDer::from(server_cert.der().to_vec());
    let server_key_der = PrivateKeyDer::try_from(server_key.serialize_der())
        .map_err(|e| anyhow::anyhow!("Failed to parse server private key: {}", e))?;

    let ca_cert_der = CertificateDer::from(ca.cert.der().to_vec());
    let mut root_store = RootCertStore::empty();
    root_store
        .add(ca_cert_der)
        .context("Failed to add CA cert to trust store")?;

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

    let transport = Arc::get_mut(&mut server_config.transport)
        .context("Cannot get mutable transport config")?;
    transport.max_idle_timeout(Some(quinn::VarInt::from_u32(30_000).into()));
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(10)));
    transport.max_concurrent_bidi_streams(quinn::VarInt::from_u32(2048));

    Ok(server_config)
}
