use crate::route::{Route, TlsMode};
use crate::state::AppState;
use std::sync::Arc;

pub fn handle_add_route(
    state: &Arc<AppState>,
    key: String,
    upstream: String,
    tls: Option<TlsMode>,
    tunnel: Option<String>,
) -> (bool, String) {
    // Split key into hostname and optional path prefix
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
    let mut routes = (**state.routes.load()).clone();
    routes.register(new_route);
    state.routes.store(Arc::new(routes));

    match state.save_config() {
        Ok(_) => {
            state
                .events
                .publish(crate::event::Event::RouteAdded { key: route_key });
            (true, "Route added successfully".to_string())
        }
        Err(e) => (false, format!("Failed to save config: {}", e)),
    }
}

pub fn handle_delete_route(state: &Arc<AppState>, key: String) -> (bool, String) {
    let route_key = key.clone();
    let (hostname, path_prefix) = if let Some(idx) = key.find('/') {
        (key[..idx].to_string(), Some(key[idx..].to_string()))
    } else {
        (key, None)
    };

    let mut routes = (**state.routes.load()).clone();
    let removed = routes.remove(&hostname, path_prefix.as_deref());
    if removed {
        state.routes.store(Arc::new(routes));
        match state.save_config() {
            Ok(_) => {
                state
                    .events
                    .publish(crate::event::Event::RouteDeleted { key: route_key });
                (true, "Route deleted successfully".to_string())
            }
            Err(e) => (false, format!("Failed to save config: {}", e)),
        }
    } else {
        (false, "Route not found".to_string())
    }
}
