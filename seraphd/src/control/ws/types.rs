use crate::route::{Route, TlsMode};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "action", content = "payload")]
pub enum ApiRequest {
    GetRoutes,
    AddRoute {
        key: String,
        upstream: String,
        tls: Option<TlsMode>,
        tunnel: Option<String>,
    },
    DeleteRoute {
        key: String,
    },
    RegisterCert {
        sni: String,
        cert_pem: String,
        key_pem: String,
    },
}

#[derive(Serialize, Clone)]
#[serde(tag = "event", content = "payload")]
pub enum ApiResponse {
    RoutesList(Vec<Route>),
    CommandResult { success: bool, message: String },
    SystemEvent { event_type: String, message: String },
}
