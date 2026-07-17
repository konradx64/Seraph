use crate::state::AppState;
use async_trait::async_trait;
use pingora::listeners::TlsAccept;
use pingora::tls::ssl::NameType;
use pingora::tls::ssl::SslRef;
use std::sync::Arc;

pub struct DynamicTlsAcceptor {
    state: Arc<AppState>,
}

impl DynamicTlsAcceptor {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl TlsAccept for DynamicTlsAcceptor {
    async fn certificate_callback(&self, ssl: &mut SslRef) {
        if let Some(sni) = ssl.servername(NameType::HOST_NAME).map(str::to_owned) {
            tracing::info!("TLS handshake for SNI: {}", sni);
            let certs = self.state.certs.load();
            if let Some(pair) = certs.get(&sni) {
                use pingora::tls::ext;
                if let Err(error) = ext::ssl_use_certificate(ssl, &pair.cert) {
                    tracing::error!("Failed to configure TLS certificate for {}: {}", sni, error);
                    return;
                }
                for certificate in &pair.chain {
                    if let Err(error) = ext::ssl_add_chain_cert(ssl, certificate) {
                        tracing::error!(
                            "Failed to configure TLS certificate chain for {}: {}",
                            sni,
                            error
                        );
                        return;
                    }
                }
                if let Err(error) = ext::ssl_use_private_key(ssl, &pair.key) {
                    tracing::error!("Failed to configure TLS private key for {}: {}", sni, error);
                }
            } else {
                tracing::warn!("No certificate registered for SNI: {}", sni);
            }
        } else {
            tracing::warn!("TLS handshake without SNI");
        }
    }
}
