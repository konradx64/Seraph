use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::RwLock;
use std::collections::HashMap;
use serde::Serialize;

#[derive(Debug)]
pub struct RouteStats {
    pub total_requests: AtomicU64,
    pub total_latency_ms: AtomicU64,
    pub online: AtomicBool,
}

impl RouteStats {
    pub fn new(online: bool) -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
            online: AtomicBool::new(online),
        }
    }

    pub fn record(&self, latency_ms: u64, is_connection_failure: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ms.fetch_add(latency_ms, Ordering::Relaxed);
        if is_connection_failure {
            self.online.store(false, Ordering::Relaxed);
        } else {
            self.online.store(true, Ordering::Relaxed);
        }
    }

    pub fn get_snapshot(&self) -> RouteStatsSnapshot {
        let reqs = self.total_requests.load(Ordering::Relaxed);
        let total_lat = self.total_latency_ms.load(Ordering::Relaxed);
        let avg = if reqs == 0 { 0 } else { total_lat / reqs };
        RouteStatsSnapshot {
            total_requests: reqs,
            avg_latency_ms: avg,
            online: self.online.load(Ordering::Relaxed),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct RouteStatsSnapshot {
    pub total_requests: u64,
    pub avg_latency_ms: u64,
    pub online: bool,
}

#[derive(Debug)]
pub struct Stats {
    pub total_requests: AtomicU64,
    pub status_2xx: AtomicU64,
    pub status_3xx: AtomicU64,
    pub status_4xx: AtomicU64,
    pub status_5xx: AtomicU64,
    pub route_stats: RwLock<HashMap<String, RouteStats>>,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            status_2xx: AtomicU64::new(0),
            status_3xx: AtomicU64::new(0),
            status_4xx: AtomicU64::new(0),
            status_5xx: AtomicU64::new(0),
            route_stats: RwLock::new(HashMap::new()),
        }
    }
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_requests: u64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub routes: HashMap<String, RouteStatsSnapshot>,
}

impl Stats {
    pub fn get_snapshot(&self) -> StatsResponse {
        let mut routes_map = HashMap::new();
        {
            let guard = self.route_stats.read().unwrap();
            for (host, rstats) in guard.iter() {
                routes_map.insert(host.clone(), rstats.get_snapshot());
            }
        }

        StatsResponse {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            status_2xx: self.status_2xx.load(Ordering::Relaxed),
            status_3xx: self.status_3xx.load(Ordering::Relaxed),
            status_4xx: self.status_4xx.load(Ordering::Relaxed),
            status_5xx: self.status_5xx.load(Ordering::Relaxed),
            routes: routes_map,
        }
    }

    pub fn record_request(&self, status: u16) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        if (200..300).contains(&status) {
            self.status_2xx.fetch_add(1, Ordering::Relaxed);
        } else if (300..400).contains(&status) {
            self.status_3xx.fetch_add(1, Ordering::Relaxed);
        } else if (400..500).contains(&status) {
            self.status_4xx.fetch_add(1, Ordering::Relaxed);
        } else if (500..600).contains(&status) {
            self.status_5xx.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_route_request(&self, host: &str, latency_ms: u64, is_connection_failure: bool) {
        {
            let read_guard = self.route_stats.read().unwrap();
            if let Some(rstats) = read_guard.get(host) {
                rstats.record(latency_ms, is_connection_failure);
                return;
            }
        }
        {
            let mut write_guard = self.route_stats.write().unwrap();
            let rstats = RouteStats::new(!is_connection_failure);
            rstats.record(latency_ms, is_connection_failure);
            write_guard.insert(host.to_string(), rstats);
        }
    }
}
