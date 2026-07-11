use super::route::Route;
use crate::web_proxy::WebProxyServer;
use crate::{config::AppConfig, route_registry::RouteRegistry, state::AppState};
use std::sync::Arc;

pub fn run() -> anyhow::Result<()> {
    let config_path = std::path::Path::new("config.toml");
    let config = if config_path.exists() {
        tracing::info!("loading config from {:?}", config_path);
        AppConfig::load_from_file(config_path)?
    } else {
        tracing::info!("config.toml not found, generating default config");
        let route = Route::new("localhost", "http://127.0.0.1:3000");
        let mut default_config = AppConfig::default();
        default_config.hostnames.insert(
            route.hostname.clone(),
            crate::config::RouteConfig {
                path_prefix: route.path_prefix.clone(),
                upstream: route.upstream.clone(),
                tunnel: route.tunnel.clone(),
                tls: route.tls.clone(),
            },
        );
        default_config.save_to_file(config_path)?;
        default_config
    };

    let routes = RouteRegistry::new(config.routes());
    let state = Arc::new(AppState::new(config, routes, config_path.to_path_buf()));

    tracing::info!("seraphd starting");
    tracing::info!("http listener: {}", state.config.http_addr);
    tracing::info!("https listener: {}", state.config.https_addr);
    tracing::info!("admin listener: {}", state.config.admin_addr);

    crate::control::start(state.clone())?;

    let web_proxy = WebProxyServer::new(state.clone());
    web_proxy.run()?;

    Ok(())
}
