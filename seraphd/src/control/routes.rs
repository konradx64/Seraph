use crate::route::{Route, TlsMode};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct DeleteParams {
    pub key: String,
}

#[derive(Serialize)]
pub struct CommandResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct AddRoutePayload {
    pub key: String,
    pub upstream: String,
    pub tls: Option<TlsMode>,
    pub tunnel: Option<String>,
}

// GET /api/routes
pub async fn get_routes(State(state): State<Arc<AppState>>) -> Json<Vec<Route>> {
    let routes = state.routes.load().all().to_vec();
    Json(routes)
}

// POST /api/routes
pub async fn add_route(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddRoutePayload>,
) -> (StatusCode, Json<CommandResponse>) {
    let key = payload.key;
    let upstream = payload.upstream;
    let tls = payload.tls;
    let tunnel = payload.tunnel;

    let (hostname, path_prefix) = if let Some(idx) = key.find('/') {
        (key[..idx].to_string(), Some(key[idx..].to_string()))
    } else {
        (key, None)
    };

    let new_route = Route {
        hostname,
        path_prefix,
        upstream,
        tunnel,
        tls: tls.unwrap_or(TlsMode::Auto),
    };

    let route_key = format!(
        "{}{}",
        new_route.hostname,
        new_route.path_prefix.as_deref().unwrap_or("")
    );

    match state.db.save_route(&new_route) {
        Ok(_) => match state.db.load_routes() {
            Ok(routes_list) => {
                let registry = crate::registry::routes::RouteRegistry::new(routes_list);
                state.routes.store(Arc::new(registry));
                let _ = state.events.send(crate::event::Event::RouteAdded { key: route_key });
                (
                    StatusCode::CREATED,
                    Json(CommandResponse {
                        success: true,
                        message: "Route added successfully".to_string(),
                    }),
                )
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CommandResponse {
                    success: false,
                    message: format!("Failed to reload routes from database: {}", e),
                }),
            ),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CommandResponse {
                success: false,
                message: format!("Failed to save route to database: {}", e),
            }),
        ),
    }
}

// DELETE /api/routes
pub async fn delete_route(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DeleteParams>,
) -> (StatusCode, Json<CommandResponse>) {
    let key = params.key;
    let route_key = key.clone();
    let (hostname, path_prefix) = if let Some(idx) = key.find('/') {
        (key[..idx].to_string(), Some(key[idx..].to_string()))
    } else {
        (key, None)
    };

    match state.db.delete_route(&hostname, path_prefix.as_deref()) {
        Ok(true) => match state.db.load_routes() {
            Ok(routes_list) => {
                let registry = crate::registry::routes::RouteRegistry::new(routes_list);
                state.routes.store(Arc::new(registry));
                let _ = state.events.send(crate::event::Event::RouteDeleted { key: route_key });
                (
                    StatusCode::OK,
                    Json(CommandResponse {
                        success: true,
                        message: "Route deleted successfully".to_string(),
                    }),
                )
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CommandResponse {
                    success: false,
                    message: format!("Failed to reload routes: {}", e),
                }),
            ),
        },
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(CommandResponse {
                success: false,
                message: "Route not found".to_string(),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CommandResponse {
                success: false,
                message: format!("Failed to delete route: {}", e),
            }),
        ),
    }
}
