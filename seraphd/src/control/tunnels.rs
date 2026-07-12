use crate::state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize)]
pub struct TunnelListItem {
    pub id: String,
    pub token: Option<String>,
    pub client_cert: Option<String>,
    pub created_at: String,
    pub status: String, // "Online" or "Offline"
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

// GET /api/tunnels
pub async fn get_tunnels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TunnelListItem>>, (StatusCode, String)> {
    let tunnels = state.db.load_tunnels().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to load tunnels: {:?}", e),
        )
    })?;

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
                token: Some(t.token),
                client_cert: t.client_cert,
                created_at: t.created_at,
                status,
                bytes_sent,
                bytes_received,
            }
        })
        .collect();

    Ok(Json(items))
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

// POST /api/tunnels
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

    // Generate a secure 32-character random key
    use rand::Rng;
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let created_at = chrono::Utc::now().to_rfc3339();

    state.db.save_tunnel(id, &token, &created_at).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save tunnel: {:?}", e),
        )
    })?;

    Ok(Json(CreateTunnelResponse {
        id: id.to_string(),
        token,
    }))
}

#[derive(Deserialize)]
pub struct DeleteTunnelParams {
    pub id: String,
}

// DELETE /api/tunnels
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

    // If the tunnel is currently online, disconnect it immediately
    {
        let mut active = state.active_tunnels.write().await;
        if let Some(conn) = active.remove(&params.id) {
            conn.close(0u32.into(), b"Tunnel deleted");
        }
    }

    Ok(Json(deleted))
}

#[derive(Deserialize)]
pub struct EnrollPayload {
    pub token: String,
    pub csr: String, // PEM-encoded CSR
}

#[derive(Serialize)]
pub struct EnrollResponse {
    pub certificate: String,    // PEM-encoded client certificate
    pub ca_certificate: String, // PEM-encoded CA certificate
}

// POST /api/tunnels/enroll
pub async fn enroll_tunnel(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EnrollPayload>,
) -> Result<Json<EnrollResponse>, (StatusCode, String)> {
    // 1. Verify token in Database
    let db_tunnel = state
        .db
        .get_tunnel_by_token(&payload.token)
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

    // 2. Parse CSR PEM using rcgen
    let csr_params = rcgen::CertificateSigningRequestParams::from_pem(&payload.csr)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid CSR PEM: {:?}", e)))?;

    // 3. Sign the certificate using our CA
    // Under rcgen 0.13, we build the client certificate from the CSR public key and subject, signed by our CA
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

    // Set certificate validity
    params.not_before = time::OffsetDateTime::now_utc();
    params.not_after = time::OffsetDateTime::now_utc()
        .checked_add(time::Duration::days(365))
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Date overflow".to_string(),
            )
        })?;

    // Sign the client public key with our CA cert and key
    let cert = params
        .signed_by(&csr_params.public_key, &state.ca.cert, &state.ca.key_pair)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Certificate signing failed: {:?}", e),
            )
        })?;

    let cert_pem = cert.pem();

    // 4. Save the issued certificate to the database
    state
        .db
        .save_tunnel_cert(&db_tunnel.id, &cert_pem)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save cert: {:?}", e),
            )
        })?;

    Ok(Json(EnrollResponse {
        certificate: cert_pem,
        ca_certificate: state.ca.cert_pem.clone(),
    }))
}

#[derive(Serialize)]
pub struct ListenerInfo {
    pub name: String,
    pub address: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub listeners: Vec<ListenerInfo>,
}

// GET /api/status
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let mut listeners = vec![
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
            address: "0.0.0.0:7700".to_string(),
            status: "Active".to_string(),
        },
    ];

    // Add active dynamic UDS route listeners
    {
        let active_listeners = state.active_route_listeners.lock().unwrap();
        for host in active_listeners.iter() {
            listeners.push(ListenerInfo {
                name: format!("UDS Route Bridge ({})", host),
                address: format!("tunnels/route-{}.sock", host),
                status: "Active".to_string(),
            });
        }
    }

    Json(StatusResponse { listeners })
}
