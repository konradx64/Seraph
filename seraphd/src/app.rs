use super::route::Route;
use crate::db::Database;
use crate::{config::AppConfig, state::AppState, registry::{CertificateRegistry, RouteRegistry}};
use std::sync::Arc;
use pingora::server::Server;
use pingora::services::background::background_service;
use crate::tunnel::listener::QuicTunnelService;
use crate::control::AdminService;
use crate::web_proxy::create_proxy_service;

pub fn run() -> anyhow::Result<()> {
    let config_path = std::path::Path::new("config.toml");
    let config = if config_path.exists() {
        tracing::info!("loading config from {:?}", config_path);
        AppConfig::load_from_file(config_path)?
    } else {
        tracing::info!("config.toml not found, generating default config");
        let default_config = AppConfig::default();
        default_config.save_to_file(config_path)?;
        default_config
    };

    // Initialize Database
    tracing::info!("opening database at {}", config.database_path);
    let db = Database::open(&config.database_path)?;

    // Load dynamic state from DB
    let mut routes_list = db.load_routes()?;
    if routes_list.is_empty() {
        tracing::info!("database is empty, inserting default route");
        let default_route = Route::new("localhost", "http://127.0.0.1:3000");
        db.save_route(&default_route)?;
        routes_list.push(default_route);
    }

    let certs_list = db.load_certs()?;

    // Populate registries
    let routes = RouteRegistry::new(routes_list);
    let mut certs = CertificateRegistry::new();
    for db_cert in certs_list {
        if let Err(e) = certs.register(&db_cert.sni, &db_cert.cert_pem, &db_cert.key_pem) {
            tracing::error!("failed to load certificate for {}: {}", db_cert.sni, e);
        }
    }

    // Initialize Tunnel CA
    let data_dir = std::path::Path::new("data");
    let ca = crate::tunnel::ca::TunnelCa::load_or_create(data_dir)?;

    let state = Arc::new(AppState::new(config, db, routes, certs, ca));

    let mut server = Server::new(None)?;
    server.bootstrap();

    // Register QUIC Tunnel Service
    let tunnel_service = background_service(
        "quic_tunnel",
        QuicTunnelService::new(state.clone())
    );
    server.add_service(tunnel_service);

    // Register Admin/Control Service
    let admin_service = background_service(
        "admin_control",
        AdminService::new(state.clone())
    );
    server.add_service(admin_service);

    // Register Web Proxy Service
    let proxy_service = create_proxy_service(&server.configuration, state.clone())?;
    server.add_service(proxy_service);

    tracing::info!("seraphd starting");
    tracing::info!("http listener: {}", state.config.http_addr);
    tracing::info!("https listener: {}", state.config.https_addr);
    tracing::info!("admin listener: {}", state.config.admin_addr);
    tracing::info!("tunnel listener: {}", state.config.tunnel_addr);

    server.run_forever();
}
