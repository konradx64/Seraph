use super::handler::WebProxyHandler;
use super::tls::DynamicTlsAcceptor;
use crate::state::AppState;
use pingora::listeners::tls::TlsSettings;
use pingora::server::Server;
use pingora_proxy::http_proxy_service;
use std::sync::Arc;

pub struct WebProxyServer {
    state: Arc<AppState>,
}

impl WebProxyServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn run(self) -> anyhow::Result<()> {
        let handler = WebProxyHandler::new(self.state.clone());

        // 1. Create Pingora Server
        let mut server = Server::new(None)?;
        server.bootstrap();

        // 2. Create the Proxy Service
        let mut proxy_service = http_proxy_service(&server.configuration, handler);

        // 3. Bind HTTP listener
        proxy_service.add_tcp(&self.state.config.http_addr);

        // 3b. Bind HTTPS listener with dynamic TLS settings
        let mut tls_settings = TlsSettings::with_callbacks(Box::new(DynamicTlsAcceptor::new(self.state.clone())))?;
        tls_settings.enable_h2();
        proxy_service.add_tls_with_settings(&self.state.config.https_addr, None, tls_settings);

        // 4. Register service with server
        server.add_service(proxy_service);

        tracing::info!("web proxy server starting");

        // 5. Run the server
        server.run_forever()
    }
}
