use crate::{config::AppConfig, db::Database, event::Event, registry::{certs::CertificateRegistry, routes::RouteRegistry}};
use arc_swap::ArcSwap;

pub struct AppState {
    pub config: AppConfig,
    pub db: Database,
    pub routes: ArcSwap<RouteRegistry>,
    pub certs: ArcSwap<CertificateRegistry>,
    pub events: tokio::sync::broadcast::Sender<Event>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        db: Database,
        routes: RouteRegistry,
        certs: CertificateRegistry,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(100);
        Self {
            config,
            db,
            routes: ArcSwap::from_pointee(routes),
            certs: ArcSwap::from_pointee(certs),
            events,
        }
    }
}