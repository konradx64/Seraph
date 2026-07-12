use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tracing::info;

pub mod dashboard;
pub mod routes;
pub mod certs;
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
            let app = Router::new()
                .route(
                    "/api/routes",
                    get(routes::get_routes)
                        .post(routes::add_route)
                        .delete(routes::delete_route),
                )
                .route("/api/certs", post(certs::register_cert))
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

