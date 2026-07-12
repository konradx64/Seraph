use super::route::Route;
use crate::web_proxy::WebProxyServer;
use crate::db::Database;
use crate::{config::AppConfig, state::AppState, registry::{certs::CertificateRegistry, routes::RouteRegistry}};
use std::sync::Arc;

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
    for (sni, cert_pem, key_pem) in certs_list {
        if let Err(e) = certs.register(&sni, &cert_pem, &key_pem) {
            tracing::error!("failed to load certificate for {}: {}", sni, e);
        }
    }

    let state = Arc::new(AppState::new(config, db, routes, certs));

    tracing::info!("seraphd starting");
    tracing::info!("http listener: {}", state.config.http_addr);
    tracing::info!("https listener: {}", state.config.https_addr);
    tracing::info!("admin listener: {}", state.config.admin_addr);

    crate::control::start(state.clone())?;

    let web_proxy = WebProxyServer::new(state.clone());
    web_proxy.run()?;

    Ok(())
}

