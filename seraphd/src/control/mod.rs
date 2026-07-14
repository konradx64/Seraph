use crate::state::AppState;
use async_trait::async_trait;
use axum::{Router, middleware, routing::get};
use pingora::services::background::BackgroundService;
use std::sync::Arc;
use tracing::info;

mod auth;
pub mod certs;
pub mod dashboard;
pub mod routes;
pub mod sse;
pub mod tunnels;

pub struct AdminService {
    state: Arc<AppState>,
}

impl AdminService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl BackgroundService for AdminService {
    async fn start(&self, mut shutdown: pingora::server::ShutdownWatch) {
        let admin_addr = self.state.config.admin_addr.clone();

        // Spawn periodic stats streaming worker
        let state_clone = self.state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            let mut last_requests = state_clone.stats.get_snapshot().total_requests;
            let mut ticks = 0_u8;
            loop {
                interval.tick().await;
                state_clone.stats.flush_events();
                let snap = state_clone.stats.get_snapshot();
                let rps = snap.total_requests.saturating_sub(last_requests);
                last_requests = snap.total_requests;

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

                ticks += 1;
                if ticks == 10 {
                    ticks = 0;
                    let persisted = state_clone.stats.persisted_snapshot();
                    if let Err(error) = state_clone.db.save_stats(&persisted) {
                        tracing::error!("failed to persist statistics: {}", error);
                    }
                }
            }
        });

        let protected = Router::new()
            .route(
                "/api/routes",
                get(routes::get_routes)
                    .post(routes::add_route)
                    .delete(routes::delete_route),
            )
            .route(
                "/api/certs",
                get(certs::get_certs).post(certs::register_cert),
            )
            .route(
                "/api/certs/refresh",
                axum::routing::post(certs::refresh_cert),
            )
            .route(
                "/api/certs/generate",
                axum::routing::post(certs::generate_cert),
            )
            .route(
                "/api/certs/acme",
                axum::routing::post(certs::generate_acme_cert),
            )
            .route(
                "/api/tunnels",
                get(tunnels::get_tunnels)
                    .post(tunnels::create_tunnel)
                    .delete(tunnels::delete_tunnel),
            )
            .route("/api/status", get(tunnels::get_status))
            .route("/api/events", get(sse::get_events))
            .fallback(dashboard::serve_asset)
            .layer(middleware::from_fn_with_state(
                auth::AdminAuth::new(self.state.config.admin_key.clone()),
                auth::require_admin_auth,
            ));

        let app = Router::new()
            .route(
                "/api/tunnels/enroll",
                axum::routing::post(tunnels::enroll_tunnel),
            )
            .merge(protected)
            .with_state(self.state.clone());

        let listener = match tokio::net::TcpListener::bind(&admin_addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(
                    "Failed to bind admin TCP listener on {}: {:?}",
                    admin_addr,
                    e
                );
                return;
            }
        };

        info!("Admin server listening on: http://{}", admin_addr);

        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown.changed().await;
            info!("Admin server received shutdown signal");
        });

        if let Err(e) = server.await {
            tracing::error!("Admin server error: {:?}", e);
        }
    }
}
