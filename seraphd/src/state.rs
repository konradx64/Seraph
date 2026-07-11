use crate::{cert_registry::CertificateRegistry, config::AppConfig, route_registry::RouteRegistry, event::EventBus};
use std::path::PathBuf;
use arc_swap::ArcSwap;

#[derive(Debug)]
pub struct AppState {
    pub config: AppConfig,
    pub routes: ArcSwap<RouteRegistry>,
    pub certs: ArcSwap<CertificateRegistry>,
    pub events: EventBus,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn new(config: AppConfig, routes: RouteRegistry, config_path: PathBuf) -> Self {
        Self {
            config,
            routes: ArcSwap::from_pointee(routes),
            certs: ArcSwap::from_pointee(CertificateRegistry::new()),
            events: EventBus::new(),
            config_path,
        }
    }

    pub fn save_config(&self) -> anyhow::Result<()> {
        let active_routes = self.routes.load().all().to_vec();
        let mut app_config = self.config.clone();
        app_config.hostnames = active_routes
            .into_iter()
            .map(|route| {
                (
                    route.hostname.clone(),
                    crate::config::RouteConfig {
                        path_prefix: route.path_prefix,
                        upstream: route.upstream,
                        tunnel: route.tunnel,
                        tls: route.tls,
                    },
                )
            })
            .collect();
        app_config.save_to_file(&self.config_path)?;
        Ok(())
    }
}