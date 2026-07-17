use super::routes::CommandResponse;
use crate::state::AppState;
use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use std::sync::Arc;

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

    match state
        .cert_store
        .save(&sni, cert_pem.as_bytes(), key_pem.as_bytes(), None)
    {
        Ok(_) => {
            let mut certs = (**state.certs.load()).clone();
            match certs.register(&sni, cert_pem.as_bytes(), key_pem.as_bytes()) {
                Ok(_) => {
                    state.certs.store(Arc::new(certs));
                    if let Ok(certs) = cert_snapshot(&state) {
                        let _ = state.events.send(crate::event::Event::CertRegistered {
                            sni: sni.clone(),
                            certs,
                        });
                    }
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
                message: format!("Failed to save certificate to disk: {}", e),
            }),
        ),
    }
}

pub async fn get_certs(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Vec<String>>) {
    match cert_snapshot(&state) {
        Ok(snis) => (StatusCode::OK, Json(snis)),
        Err(e) => {
            tracing::error!("Failed to load certificates: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::new()))
        }
    }
}

pub fn cert_snapshot(state: &AppState) -> anyhow::Result<Vec<String>> {
    match state.cert_store.load_all() {
        Ok(certs_list) => {
            let snis: Vec<String> = certs_list.into_iter().map(|c| c.sni).collect();
            Ok(snis)
        }
        Err(e) => Err(e),
    }
}

#[derive(Deserialize)]
pub struct RefreshCertPayload {
    pub sni: String,
}

pub async fn refresh_cert(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RefreshCertPayload>,
) -> (StatusCode, Json<CommandResponse>) {
    let sni = payload.sni.clone();
    crate::acme::trigger_refresh(state, sni.clone(), None).await;
    (
        StatusCode::ACCEPTED,
        Json(CommandResponse {
            success: true,
            message: format!("Certificate renewal triggered for {}", sni),
        }),
    )
}

#[derive(Deserialize)]
pub struct GenerateCertPayload {
    pub sni: String,
}

pub async fn generate_cert(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateCertPayload>,
) -> (StatusCode, Json<CommandResponse>) {
    let sni = payload.sni;

    // Generate a self-signed certificate with rcgen
    let result = (|| -> anyhow::Result<(String, String)> {
        let mut params = rcgen::CertificateParams::new(vec![sni.clone()])?;
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, &sni);
        let key_pair = rcgen::KeyPair::generate()?;
        let cert = params.self_signed(&key_pair)?;
        Ok((cert.pem(), key_pair.serialize_pem()))
    })();

    match result {
        Ok((cert_pem, key_pem)) => {
            if let Err(e) =
                state
                    .cert_store
                    .save(&sni, cert_pem.as_bytes(), key_pem.as_bytes(), None)
            {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CommandResponse {
                        success: false,
                        message: format!("Failed to save generated cert: {}", e),
                    }),
                );
            }

            let mut certs = (**state.certs.load()).clone();
            if let Err(e) = certs.register(&sni, cert_pem.as_bytes(), key_pem.as_bytes()) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CommandResponse {
                        success: false,
                        message: format!("Failed to register generated cert: {}", e),
                    }),
                );
            }
            state.certs.store(Arc::new(certs));
            if let Ok(certs) = cert_snapshot(&state) {
                let _ = state.events.send(crate::event::Event::CertRegistered {
                    sni: sni.clone(),
                    certs,
                });
            }

            (
                StatusCode::CREATED,
                Json(CommandResponse {
                    success: true,
                    message: format!("Self-signed certificate generated for {}", sni),
                }),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CommandResponse {
                success: false,
                message: format!("Failed to generate certificate: {}", e),
            }),
        ),
    }
}

#[derive(Deserialize)]
pub struct GenerateAcmeCertPayload {
    pub sni: String,
    pub email: String,
}

pub async fn generate_acme_cert(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateAcmeCertPayload>,
) -> (StatusCode, Json<CommandResponse>) {
    let sni = payload.sni;
    let email = payload.email;
    let response_sni = sni.clone();

    crate::acme::trigger_refresh(state, sni, Some(email)).await;

    (
        StatusCode::ACCEPTED,
        Json(CommandResponse {
            success: true,
            message: format!(
                "Let's Encrypt TLS request initiated for {}. Check request logs for challenge status.",
                response_sni
            ),
        }),
    )
}
