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
use std::sync::Arc;

use super::ca::TunnelCa;

/// Default UDP port for the QUIC tunnel listener.
pub const TUNNEL_PORT: u16 = 7700;

/// Wraps the Quinn endpoint and exposes an `accept` method.
pub struct TunnelListener {
    endpoint: Endpoint,
}

impl TunnelListener {
    /// Create a new QUIC endpoint and start listening.
    ///
    /// The server generates a self-signed TLS certificate for its own identity
    /// (agents verify this against the CA cert they received at registration).
    /// The server in turn requires a valid client certificate signed by the
    /// Seraph Tunnel CA (`ca`) for every incoming connection.
    pub fn bind(addr: SocketAddr, ca: &TunnelCa) -> Result<Self> {
        let server_config = build_server_config(ca)
            .context("Failed to build QUIC server TLS config")?;

        let endpoint = Endpoint::server(server_config, addr)
            .context("Failed to bind QUIC endpoint")?;

        tracing::info!("Tunnel QUIC listener bound on {}", addr);
        Ok(Self { endpoint })
    }

    /// Accept the next incoming QUIC connection.
    pub async fn accept(&self) -> Option<Incoming> {
        self.endpoint.accept().await
    }

    /// Spawn a long-running task that accepts and dispatches tunnel connections.
    pub async fn run(self) {
        tracing::info!("Tunnel listener running");
        while let Some(incoming) = self.endpoint.accept().await {
            tokio::spawn(handle_connection(incoming));
        }
        tracing::warn!("Tunnel listener stopped accepting connections");
    }
}

// ---------------------------------------------------------------------------
// Connection handler
// ---------------------------------------------------------------------------

async fn handle_connection(incoming: Incoming) {
    let remote = incoming.remote_address();

    match incoming.await {
        Ok(conn) => {
            let agent_id = extract_agent_id(&conn);
            tracing::info!(
                "Tunnel agent '{}' connected from {}",
                agent_id.as_deref().unwrap_or("<unknown>"),
                remote,
            );
            handle_streams(conn, agent_id).await;
        }
        Err(e) => {
            // Connection rejected at the TLS handshake (e.g. invalid cert)
            tracing::warn!("Tunnel connection from {} rejected: {}", remote, e);
        }
    }
}

async fn handle_streams(conn: quinn::Connection, agent_id: Option<String>) {
    loop {
        match conn.accept_bi().await {
            Ok((send, recv)) => {
                let id = agent_id.clone();
                tokio::spawn(async move {
                    if let Err(e) = dispatch_stream(send, recv, id).await {
                        tracing::warn!("Tunnel stream error: {}", e);
                    }
                });
            }
            Err(e) => {
                tracing::info!(
                    "Agent '{}' disconnected: {}",
                    agent_id.as_deref().unwrap_or("<unknown>"),
                    e
                );
                break;
            }
        }
    }
}

async fn dispatch_stream(
    _send: quinn::SendStream,
    mut recv: quinn::RecvStream,
    agent_id: Option<String>,
) -> Result<()> {
    // Read the first 4 bytes as a frame type tag
    let mut tag = [0u8; 4];
    recv.read_exact(&mut tag).await
        .context("Failed to read stream tag")?;

    tracing::debug!(
        "Stream from agent '{}': tag={:?}",
        agent_id.as_deref().unwrap_or("<unknown>"),
        tag,
    );

    // TODO: dispatch to tunnel protocol handlers based on `tag`
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

    Ok(server_config)
}
