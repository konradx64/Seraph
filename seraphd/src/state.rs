use crate::{
    cert_store::CertificateStore,
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
    pub cert_store: CertificateStore,
    pub routes: ArcSwap<RouteRegistry>,
    pub certs: ArcSwap<CertificateRegistry>,
    pub events: tokio::sync::broadcast::Sender<Event>,
    pub acme_challenges: RwLock<HashMap<String, String>>,
    pub stats: crate::stats::Stats,
    pub active_tunnels: std::sync::Arc<tokio::sync::RwLock<HashMap<String, quinn::Connection>>>,
    pub ca: std::sync::Arc<crate::tunnel::ca::TunnelCa>,
    pub geoip: std::sync::Arc<crate::geoip::GeoIpService>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        db: Database,
        cert_store: CertificateStore,
        routes: RouteRegistry,
        certs: CertificateRegistry,
        ca: crate::tunnel::ca::TunnelCa,
        stats: crate::stats::Stats,
        geoip: crate::geoip::GeoIpService,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(100);
        Self {
            config,
            db,
            cert_store,
            routes: ArcSwap::from_pointee(routes),
            certs: ArcSwap::from_pointee(certs),
            events,
            acme_challenges: RwLock::new(HashMap::new()),
            stats,
            active_tunnels: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            ca: std::sync::Arc::new(ca),
            geoip: std::sync::Arc::new(geoip),
        }
    }
}
