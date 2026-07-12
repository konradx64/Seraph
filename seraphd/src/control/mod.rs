use crate::state::AppState;
use axum::{
    routing::get,
    Router,
};
use std::sync::Arc;
use tracing::info;

pub mod dashboard;
pub mod routes;
pub mod certs;
pub mod tunnels;
pub mod sse;

pub fn start(state: Arc<AppState>) -> anyhow::Result<()> {
    let admin_addr = state.config.admin_addr.clone();
    
    // Bind synchronously on the main thread to catch socket binding errors early.
    let std_listener = std::net::TcpListener::bind(&admin_addr)?;
    std_listener.set_nonblocking(true)?;

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            tokio::spawn(crate::acme::start_acme_worker(state.clone()));

            // Spawn periodic stats streaming worker
            let state_clone = state.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                let mut last_requests = 0;
                loop {
                    interval.tick().await;
                    let snap = state_clone.stats.get_snapshot();
                    let current_requests = snap.total_requests;
                    let rps = current_requests.saturating_sub(last_requests);
                    last_requests = current_requests;

                    let _ = state_clone.events.send(crate::event::Event::StatsUpdate {
                        total_requests: snap.total_requests,
                        status_2xx: snap.status_2xx,
                        status_3xx: snap.status_3xx,
                        status_4xx: snap.status_4xx,
                        status_5xx: snap.status_5xx,
                        rps,
                        routes: snap.routes,
                        tunnels: snap.tunnels,
                    });
                }
            });

            let app = Router::new()
                .route(
                    "/api/routes",
                    get(routes::get_routes)
                        .post(routes::add_route)
                        .delete(routes::delete_route),
                )
                .route("/api/certs", get(certs::get_certs).post(certs::register_cert))
                .route("/api/certs/refresh", axum::routing::post(certs::refresh_cert))
                .route("/api/certs/generate", axum::routing::post(certs::generate_cert))
                .route("/api/certs/acme", axum::routing::post(certs::generate_acme_cert))
                .route(
                    "/api/tunnels",
                    get(tunnels::get_tunnels)
                        .post(tunnels::create_tunnel)
                        .delete(tunnels::delete_tunnel),
                )
                .route("/api/tunnels/enroll", axum::routing::post(tunnels::enroll_tunnel))
                .route("/api/events", get(sse::get_events))
                .fallback(dashboard::serve_asset)
                .with_state(state);

            let listener = tokio::net::TcpListener::from_std(std_listener).unwrap();
            info!("Admin server listening on: http://{}", admin_addr);
            axum::serve(listener, app).await.unwrap();
        });
    });
    Ok(())
}

