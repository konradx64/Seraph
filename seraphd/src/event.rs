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
    TunnelConnected {
        id: String,
    },
    TunnelDisconnected {
        id: String,
    },
    Log {
        time: String,
        text: String,
    },
    StatsUpdate {
        total_requests: u64,
        status_2xx: u64,
        status_3xx: u64,
        status_4xx: u64,
        status_5xx: u64,
        rps: u64,
        routes: std::collections::HashMap<String, crate::stats::RouteStatsSnapshot>,
        tunnels: std::collections::HashMap<String, crate::stats::TunnelStatsSnapshot>,
    },
}

