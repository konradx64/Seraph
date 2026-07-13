#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum Event {
    DashboardSnapshot {
        routes: Vec<crate::route::Route>,
        certs: Vec<String>,
        tunnels: Vec<crate::control::tunnels::TunnelListItem>,
        status: crate::control::tunnels::StatusResponse,
        stats: crate::stats::StatsResponse,
    },
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
        routes: Vec<crate::route::Route>,
    },
    RouteDeleted {
        key: String,
        routes: Vec<crate::route::Route>,
    },
    CertRegistered {
        sni: String,
        certs: Vec<String>,
    },
    TunnelCreated {
        id: String,
        tunnels: Vec<crate::control::tunnels::TunnelListItem>,
        status: crate::control::tunnels::StatusResponse,
    },
    TunnelDeleted {
        id: String,
        tunnels: Vec<crate::control::tunnels::TunnelListItem>,
        status: crate::control::tunnels::StatusResponse,
    },
    TunnelEnrolled {
        id: String,
        tunnels: Vec<crate::control::tunnels::TunnelListItem>,
        status: crate::control::tunnels::StatusResponse,
    },
    TunnelConnected {
        id: String,
        tunnels: Vec<crate::control::tunnels::TunnelListItem>,
        status: crate::control::tunnels::StatusResponse,
    },
    TunnelDisconnected {
        id: String,
        tunnels: Vec<crate::control::tunnels::TunnelListItem>,
        status: crate::control::tunnels::StatusResponse,
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
