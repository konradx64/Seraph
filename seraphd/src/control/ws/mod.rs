use crate::state::AppState;
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use std::sync::Arc;
use tracing::{info, warn};

pub mod certs;
pub mod routes;
pub mod types;

use types::{ApiRequest, ApiResponse};

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

fn map_domain_event_to_api_response(event: crate::event::Event) -> ApiResponse {
    ApiResponse::SystemEvent {
        event_type: event.event_type().to_string(),
        message: event.to_string(),
    }
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    info!("New admin WebSocket connection");
    
    let mut event_rx = state.events.subscribe();

    // Send initial routes
    let routes = state.routes.load().all().to_vec();
    if send_response(&mut socket, &ApiResponse::RoutesList(routes)).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_client_message(&mut socket, &state, text).await;
                    }
                    Some(Err(_)) | None => {
                        break;
                    }
                    _ => {}
                }
            }
            res = event_rx.recv() => {
                match res {
                    Ok(event) => {
                        let api_resp = map_domain_event_to_api_response(event);
                        if send_response(&mut socket, &api_resp).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        warn!("WebSocket client lagged on event stream");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }
    info!("Admin WebSocket connection closed");
}

async fn handle_client_message(socket: &mut WebSocket, state: &Arc<AppState>, text: String) {
    let req: Result<ApiRequest, _> = serde_json::from_str(&text);
    match req {
        Ok(ApiRequest::GetRoutes) => {
            let routes = state.routes.load().all().to_vec();
            let _ = send_response(socket, &ApiResponse::RoutesList(routes)).await;
        }
        Ok(ApiRequest::AddRoute { key, upstream, tls, tunnel }) => {
            let (success, message) = routes::handle_add_route(state, key, upstream, tls, tunnel);
            let _ = send_response(socket, &ApiResponse::CommandResult { success, message }).await;
            let routes = state.routes.load().all().to_vec();
            let _ = send_response(socket, &ApiResponse::RoutesList(routes)).await;
        }
        Ok(ApiRequest::DeleteRoute { key }) => {
            let (success, message) = routes::handle_delete_route(state, key);
            let _ = send_response(socket, &ApiResponse::CommandResult { success, message }).await;
            let routes = state.routes.load().all().to_vec();
            let _ = send_response(socket, &ApiResponse::RoutesList(routes)).await;
        }
        Ok(ApiRequest::RegisterCert { sni, cert_pem, key_pem }) => {
            let (success, message) = certs::handle_register_cert(state, sni, cert_pem, key_pem);
            let _ = send_response(socket, &ApiResponse::CommandResult { success, message }).await;
        }
        Err(e) => {
            warn!("Failed to parse admin request JSON: {}", e);
            let _ = send_response(socket, &ApiResponse::CommandResult {
                success: false,
                message: format!("Invalid message: {}", e),
            }).await;
        }
    }
}

async fn send_response(socket: &mut WebSocket, resp: &ApiResponse) -> Result<(), axum::Error> {
    let text = serde_json::to_string(resp).unwrap();
    socket.send(Message::Text(text)).await
}
