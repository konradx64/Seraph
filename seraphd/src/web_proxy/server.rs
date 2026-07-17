use super::handler::WebProxyHandler;
use super::tls::DynamicTlsAcceptor;
use crate::state::AppState;
use pingora::listeners::tls::TlsSettings;
use pingora_proxy::http_proxy_service;
use std::sync::Arc;

pub fn create_proxy_service(
    configuration: &Arc<pingora::server::configuration::ServerConf>,
    state: Arc<AppState>,
) -> anyhow::Result<impl pingora::services::Service + 'static> {
    let handler = WebProxyHandler::new(state.clone());
    let mut proxy_service = http_proxy_service(configuration, handler);

    proxy_service.add_tcp(&state.config.http_addr);

    let mut tls_settings =
        TlsSettings::with_callbacks(Box::new(DynamicTlsAcceptor::new(state.clone())))?;
    if state.config.http2 {
        tls_settings.enable_h2();
    }
    proxy_service.add_tls_with_settings(&state.config.https_addr, None, tls_settings);

    Ok(proxy_service)
}
