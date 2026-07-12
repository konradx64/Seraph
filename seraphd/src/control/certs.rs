use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::state::AppState;
use super::routes::CommandResponse;

#[derive(Deserialize)]
pub struct RegisterCertPayload {
    pub sni: String,
    pub cert_pem: String,
    pub key_pem: String,
}

pub async fn register_cert(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterCertPayload>,
) -> (StatusCode, Json<CommandResponse>) {
    let sni = payload.sni;
    let cert_pem = payload.cert_pem;
    let key_pem = payload.key_pem;

    match state.db.save_cert(&sni, cert_pem.as_bytes(), key_pem.as_bytes()) {
        Ok(_) => {
            let mut certs = (**state.certs.load()).clone();
            match certs.register(&sni, cert_pem.as_bytes(), key_pem.as_bytes()) {
                Ok(_) => {
                    state.certs.store(Arc::new(certs));
                    let _ = state.events.send(crate::event::Event::CertRegistered {
                        sni: sni.clone(),
                    });
                    (
                        StatusCode::CREATED,
                        Json(CommandResponse {
                            success: true,
                            message: format!("Certificate registered successfully for {}", sni),
                        }),
                    )
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CommandResponse {
                        success: false,
                        message: format!("Failed to register certificate in registry: {}", e),
                    }),
                ),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CommandResponse {
                success: false,
                message: format!("Failed to save certificate to database: {}", e),
            }),
        ),
    }
}
