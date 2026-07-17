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

    let tls_settings =
        TlsSettings::with_callbacks(Box::new(DynamicTlsAcceptor::new(state.clone())))?;
    // Keep downstream traffic on HTTP/1.1 for now. Tunnel-backed upstreams are
    // HTTP/1.1 streams, and multiplexing multiple stateful downstream requests
    // over HTTP/2 can cause session responses to race (for example, Nextcloud
    // repeatedly replacing its session cookie and redirecting back to login).
    proxy_service.add_tls_with_settings(&state.config.https_addr, None, tls_settings);

    Ok(proxy_service)
}
