use crate::{
    config::AppConfig,
    db::Database,
    event::Event,
    registry::{CertificateRegistry, RouteRegistry},
};
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::sync::RwLock;

pub struct AppState {
    pub config: AppConfig,
    pub db: Database,
    pub routes: ArcSwap<RouteRegistry>,
    pub certs: ArcSwap<CertificateRegistry>,
    pub events: tokio::sync::broadcast::Sender<Event>,
    pub acme_challenges: RwLock<HashMap<String, String>>,
    pub stats: crate::stats::Stats,
    pub active_tunnels: std::sync::Arc<tokio::sync::RwLock<HashMap<String, quinn::Connection>>>,
    pub active_route_listeners: std::sync::Mutex<std::collections::HashSet<String>>,
    pub ca: std::sync::Arc<crate::tunnel::ca::TunnelCa>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        db: Database,
        routes: RouteRegistry,
        certs: CertificateRegistry,
        ca: crate::tunnel::ca::TunnelCa,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(100);
        Self {
            config,
            db,
            routes: ArcSwap::from_pointee(routes),
            certs: ArcSwap::from_pointee(certs),
            events,
            acme_challenges: RwLock::new(HashMap::new()),
            stats: crate::stats::Stats::default(),
            active_tunnels: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            active_route_listeners: std::sync::Mutex::new(std::collections::HashSet::new()),
            ca: std::sync::Arc::new(ca),
        }
    }
}
