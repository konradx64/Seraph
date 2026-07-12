use super::handler::WebProxyHandler;
use super::tls::DynamicTlsAcceptor;
use crate::state::AppState;
use crate::tunnel::listener::QuicTunnelService;
use pingora::listeners::tls::TlsSettings;
use pingora::server::Server;
use pingora_proxy::http_proxy_service;
use std::sync::Arc;
use pingora::services::background::background_service;

pub struct WebProxyServer {
    state: Arc<AppState>,
}

impl WebProxyServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn run(self) -> anyhow::Result<()> {
        let handler = WebProxyHandler::new(self.state.clone());

        let mut server = Server::new(None)?;
        server.bootstrap();

        // Register the QUIC Tunnel background service with Pingora Server
        let tunnel_service = background_service(
            "quic_tunnel",
            QuicTunnelService::new(self.state.clone())
        );
        server.add_service(tunnel_service);

        let mut proxy_service = http_proxy_service(&server.configuration, handler);

        proxy_service.add_tcp(&self.state.config.http_addr);

        let mut tls_settings =
            TlsSettings::with_callbacks(Box::new(DynamicTlsAcceptor::new(self.state.clone())))?;
        tls_settings.enable_h2();
        proxy_service.add_tls_with_settings(&self.state.config.https_addr, None, tls_settings);

        server.add_service(proxy_service);

        tracing::info!("web proxy server starting");

        server.run_forever()
    }
}
