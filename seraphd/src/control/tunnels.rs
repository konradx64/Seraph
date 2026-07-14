use crate::state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Duration, Utc};
use openssl::sha::sha256;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const ENROLLMENT_TOKEN_TTL_MINUTES: i64 = 10;

#[derive(Debug, Serialize, Clone)]
pub struct TunnelListItem {
    pub id: String,
    pub token: Option<String>,
    pub client_cert: Option<String>,
    pub created_at: String,
    pub status: String,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

pub async fn tunnel_snapshot(state: &AppState) -> Result<Vec<TunnelListItem>, String> {
    let tunnels = state
        .db
        .load_tunnels()
        .map_err(|e| format!("Failed to load tunnels: {:?}", e))?;

    let active = state.active_tunnels.read().await;

    let items = tunnels
        .into_iter()
        .map(|t| {
            let status = if active.contains_key(&t.id) {
                "Online".to_string()
            } else {
                "Offline".to_string()
            };

            let (bytes_sent, bytes_received) = {
                let guard = state.stats.tunnel_stats.read().unwrap();
                if let Some(tstats) = guard.get(&t.id) {
                    (
                        tstats.bytes_sent.load(std::sync::atomic::Ordering::Relaxed),
                        tstats
                            .bytes_received
                            .load(std::sync::atomic::Ordering::Relaxed),
                    )
                } else {
                    (0, 0)
                }
            };

            TunnelListItem {
                id: t.id,
                token: None,
                client_cert: t.client_cert,
                created_at: t.created_at,
                status,
                bytes_sent,
                bytes_received,
            }
        })
        .collect();

    Ok(items)
}

pub async fn get_tunnels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TunnelListItem>>, (StatusCode, String)> {
    tunnel_snapshot(&state)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Deserialize)]
pub struct CreateTunnelPayload {
    pub id: String,
}

#[derive(Serialize)]
pub struct CreateTunnelResponse {
    pub id: String,
    pub token: String,
}

pub async fn create_tunnel(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTunnelPayload>,
) -> Result<Json<CreateTunnelResponse>, (StatusCode, String)> {
    let id = payload.id.trim();
    if id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Tunnel ID cannot be empty".to_string(),
        ));
    }

    use rand::Rng;
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let token_hash = hash_enrollment_token(&token);

    let now = Utc::now();
    let created_at = now.to_rfc3339();
    let enrollment_expires_at =
        (now + Duration::minutes(ENROLLMENT_TOKEN_TTL_MINUTES)).to_rfc3339();

    state
        .db
        .save_tunnel(id, &token_hash, &created_at, &enrollment_expires_at)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save tunnel: {:?}", e),
            )
        })?;

    if let Ok(tunnels) = tunnel_snapshot(&state).await {
        let _ = state.events.send(crate::event::Event::TunnelCreated {
            id: id.to_string(),
            tunnels,
            status: status_snapshot(&state),
        });
    }

    Ok(Json(CreateTunnelResponse {
        id: id.to_string(),
        token,
    }))
}

#[derive(Deserialize)]
pub struct DeleteTunnelParams {
    pub id: String,
}

pub async fn delete_tunnel(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DeleteTunnelParams>,
) -> Result<Json<bool>, (StatusCode, String)> {
    let deleted = state.db.delete_tunnel(&params.id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete tunnel: {:?}", e),
        )
    })?;

    {
        let mut active = state.active_tunnels.write().await;
        if let Some(conn) = active.remove(&params.id) {
            conn.close(0u32.into(), b"Tunnel deleted");
        }
    }

    if deleted && let Ok(tunnels) = tunnel_snapshot(&state).await {
        let _ = state.events.send(crate::event::Event::TunnelDeleted {
            id: params.id.clone(),
            tunnels,
            status: status_snapshot(&state),
        });
    }

    Ok(Json(deleted))
}

#[derive(Deserialize)]
pub struct EnrollPayload {
    pub token: String,
    pub csr: String,
}

#[derive(Serialize)]
pub struct EnrollResponse {
    pub certificate: String,
    pub ca_certificate: String,
}

pub async fn enroll_tunnel(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EnrollPayload>,
) -> Result<Json<EnrollResponse>, (StatusCode, String)> {
    let token_hash = hash_enrollment_token(&payload.token);
    let db_tunnel = state
        .db
        .get_tunnel_by_token_hash(&token_hash)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {:?}", e),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid enrollment token".to_string(),
            )
        })?;

    if db_tunnel.client_cert.is_some() || db_tunnel.enrollment_used_at.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "Tunnel is already enrolled. Rotate the enrollment key to enroll again.".to_string(),
        ));
    }

    let expires_at = db_tunnel
        .enrollment_expires_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "Enrollment token has no valid expiry and must be rotated.".to_string(),
            )
        })?;

    if Utc::now() > expires_at {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Enrollment token expired. Generate a new enrollment key.".to_string(),
        ));
    }

    let csr_params = rcgen::CertificateSigningRequestParams::from_pem(&payload.csr)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid CSR PEM: {:?}", e)))?;

    let mut params = rcgen::CertificateParams::new(vec![]).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Params build failed: {:?}", e),
        )
    })?;

    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, &db_tunnel.id);
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "Seraph Agent");
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
    params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];

    params.not_before = time::OffsetDateTime::now_utc();
    params.not_after = time::OffsetDateTime::now_utc()
        .checked_add(time::Duration::days(365))
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Date overflow".to_string(),
            )
        })?;

    let cert = params
        .signed_by(&csr_params.public_key, &state.ca.cert, &state.ca.key_pair)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Certificate signing failed: {:?}", e),
            )
        })?;

    let cert_pem = cert.pem();
    let cert_fingerprint = certificate_fingerprint(&cert_pem).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to fingerprint certificate: {:?}", e),
        )
    })?;
    let enrollment_used_at = Utc::now().to_rfc3339();

    let saved = state
        .db
        .save_tunnel_cert(
            &db_tunnel.id,
            &cert_pem,
            &cert_fingerprint,
            &enrollment_used_at,
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save cert: {:?}", e),
            )
        })?;

    if !saved {
        return Err((
            StatusCode::CONFLICT,
            "Tunnel was enrolled by another request. Generate a new enrollment key if needed."
                .to_string(),
        ));
    }

    if let Ok(tunnels) = tunnel_snapshot(&state).await {
        let _ = state.events.send(crate::event::Event::TunnelEnrolled {
            id: db_tunnel.id.clone(),
            tunnels,
            status: status_snapshot(&state),
        });
    }

    Ok(Json(EnrollResponse {
        certificate: cert_pem,
        ca_certificate: state.ca.cert_pem.clone(),
    }))
}

#[derive(Debug, Serialize, Clone)]
pub struct ListenerInfo {
    pub name: String,
    pub address: String,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct StatusResponse {
    pub listeners: Vec<ListenerInfo>,
}

pub fn status_snapshot(state: &AppState) -> StatusResponse {
    let listeners = vec![
        ListenerInfo {
            name: "HTTP Web Proxy".to_string(),
            address: state.config.http_addr.clone(),
            status: "Active".to_string(),
        },
        ListenerInfo {
            name: "HTTPS Web Proxy".to_string(),
            address: state.config.https_addr.clone(),
            status: "Active".to_string(),
        },
        ListenerInfo {
            name: "Admin Control Portal".to_string(),
            address: state.config.admin_addr.clone(),
            status: "Active".to_string(),
        },
        ListenerInfo {
            name: "QUIC Tunnel Server".to_string(),
            address: state.config.tunnel_addr.clone(),
            status: "Active".to_string(),
        },
    ];
    StatusResponse { listeners }
}

pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    Json(status_snapshot(&state))
}

pub fn hash_enrollment_token(token: &str) -> String {
    hex_encode(&sha256(token.as_bytes()))
}

fn certificate_fingerprint(cert_pem: &str) -> anyhow::Result<String> {
    let cert = openssl::x509::X509::from_pem(cert_pem.as_bytes())?;
    Ok(hex_encode(&sha256(&cert.to_der()?)))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}
