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
    pub upstream_tls: Option<bool>,
    pub hsts: Option<bool>,
    pub cors_origins: Option<String>,
    pub forward_ip: Option<bool>,
}

// GET /api/routes
pub async fn get_routes(State(state): State<Arc<AppState>>) -> Json<Vec<Route>> {
    Json(route_snapshot(&state))
}

pub fn route_snapshot(state: &AppState) -> Vec<Route> {
    state.routes.load().all().to_vec()
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
    let upstream_tls = payload.upstream_tls.unwrap_or(false);
    let hsts = payload.hsts.unwrap_or(false);
    let cors_origins = payload.cors_origins;
    let forward_ip = payload.forward_ip.unwrap_or(true);

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
        tls: tls.unwrap_or_default(),
        upstream_tls,
        hsts,
        cors_origins,
        forward_ip,
    };

    let route_key = format!(
        "{}{}",
        new_route.hostname,
        new_route.path_prefix.as_deref().unwrap_or("")
    );

    match state.db.save_route(&new_route) {
        Ok(_) => match state.db.load_routes() {
            Ok(routes_list) => {
                let registry = crate::registry::RouteRegistry::new(routes_list.clone());
                state.routes.store(Arc::new(registry));
                let _ = state.events.send(crate::event::Event::RouteAdded {
                    key: route_key,
                    routes: routes_list,
                });
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
                let registry = crate::registry::RouteRegistry::new(routes_list.clone());
                state.routes.store(Arc::new(registry));
                let _ = state.events.send(crate::event::Event::RouteDeleted {
                    key: route_key,
                    routes: routes_list,
                });
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
