#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum Event {
    RequestHit {
        host: String,
        method: String,
        path: String,
        upstream: String,
    },
    RequestMiss {
        host: String,
        method: String,
        path: String,
    },
    RouteAdded {
        key: String,
    },
    RouteDeleted {
        key: String,
    },
    CertRegistered {
        sni: String,
    },
    Log {
        time: String,
        text: String,
    },
}

