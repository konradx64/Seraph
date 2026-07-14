use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use tokio::sync::mpsc;

const STATS_EVENT_BUFFER: usize = 16_384;
const MAX_FLUSH_EVENTS: usize = 65_536;

#[derive(Debug, Default)]
pub struct PersistedStats {
    pub total_requests: u64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub dropped_events: u64,
    pub routes: HashMap<String, PersistedRouteStats>,
    pub tunnels: HashMap<String, PersistedTunnelStats>,
}

#[derive(Debug)]
pub struct PersistedRouteStats {
    pub total_requests: u64,
    pub total_latency_ms: u64,
}

#[derive(Debug)]
pub struct PersistedTunnelStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Debug)]
enum StatsEvent {
    Request {
        status: u16,
    },
    RouteRequest {
        host: String,
        latency_ms: u64,
        is_connection_failure: bool,
    },
    TunnelTraffic {
        id: String,
        sent: u64,
        received: u64,
    },
}

#[derive(Default)]
struct PendingRouteStats {
    total_requests: u64,
    total_latency_ms: u64,
    online: bool,
}

#[derive(Default)]
struct PendingTunnelStats {
    bytes_sent: u64,
    bytes_received: u64,
}

#[derive(Default)]
struct PendingStats {
    total_requests: u64,
    status_2xx: u64,
    status_3xx: u64,
    status_4xx: u64,
    status_5xx: u64,
    routes: HashMap<String, PendingRouteStats>,
    tunnels: HashMap<String, PendingTunnelStats>,
}

impl PendingStats {
    fn record(&mut self, event: StatsEvent) {
        match event {
            StatsEvent::Request { status } => {
                self.total_requests += 1;

                if (200..300).contains(&status) {
                    self.status_2xx += 1;
                } else if (300..400).contains(&status) {
                    self.status_3xx += 1;
                } else if (400..500).contains(&status) {
                    self.status_4xx += 1;
                } else if (500..600).contains(&status) {
                    self.status_5xx += 1;
                }
            }
            StatsEvent::RouteRequest {
                host,
                latency_ms,
                is_connection_failure,
            } => {
                let stats = self.routes.entry(host).or_default();
                stats.total_requests += 1;
                stats.total_latency_ms += latency_ms;
                stats.online = !is_connection_failure;
            }
            StatsEvent::TunnelTraffic { id, sent, received } => {
                let stats = self.tunnels.entry(id).or_default();
                stats.bytes_sent += sent;
                stats.bytes_received += received;
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.total_requests == 0 && self.routes.is_empty() && self.tunnels.is_empty()
    }
}

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
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);
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
pub struct TunnelStats {
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
}

#[derive(Serialize, Clone, Debug)]
pub struct TunnelStatsSnapshot {
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Debug)]
pub struct Stats {
    pub total_requests: AtomicU64,
    pub status_2xx: AtomicU64,
    pub status_3xx: AtomicU64,
    pub status_4xx: AtomicU64,
    pub status_5xx: AtomicU64,
    pub route_stats: RwLock<HashMap<String, RouteStats>>,
    pub tunnel_stats: RwLock<HashMap<String, TunnelStats>>,
    event_tx: mpsc::Sender<StatsEvent>,
    event_rx: Mutex<mpsc::Receiver<StatsEvent>>,
    dropped_events: AtomicU64,
}

impl Default for Stats {
    fn default() -> Self {
        let (event_tx, event_rx) = mpsc::channel(STATS_EVENT_BUFFER);

        Self {
            total_requests: AtomicU64::new(0),
            status_2xx: AtomicU64::new(0),
            status_3xx: AtomicU64::new(0),
            status_4xx: AtomicU64::new(0),
            status_5xx: AtomicU64::new(0),
            route_stats: RwLock::new(HashMap::new()),
            tunnel_stats: RwLock::new(HashMap::new()),
            event_tx,
            event_rx: Mutex::new(event_rx),
            dropped_events: AtomicU64::new(0),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct StatsResponse {
    pub total_requests: u64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub dropped_events: u64,
    pub routes: HashMap<String, RouteStatsSnapshot>,
    pub tunnels: HashMap<String, TunnelStatsSnapshot>,
}

impl Stats {
    pub fn from_persisted(persisted: PersistedStats) -> Self {
        let (event_tx, event_rx) = mpsc::channel(STATS_EVENT_BUFFER);
        Self {
            total_requests: AtomicU64::new(persisted.total_requests),
            status_2xx: AtomicU64::new(persisted.status_2xx),
            status_3xx: AtomicU64::new(persisted.status_3xx),
            status_4xx: AtomicU64::new(persisted.status_4xx),
            status_5xx: AtomicU64::new(persisted.status_5xx),
            route_stats: RwLock::new(
                persisted
                    .routes
                    .into_iter()
                    .map(|(host, route)| {
                        (
                            host,
                            RouteStats {
                                total_requests: AtomicU64::new(route.total_requests),
                                total_latency_ms: AtomicU64::new(route.total_latency_ms),
                                online: AtomicBool::new(false),
                            },
                        )
                    })
                    .collect(),
            ),
            tunnel_stats: RwLock::new(
                persisted
                    .tunnels
                    .into_iter()
                    .map(|(id, tunnel)| {
                        (
                            id,
                            TunnelStats {
                                bytes_sent: AtomicU64::new(tunnel.bytes_sent),
                                bytes_received: AtomicU64::new(tunnel.bytes_received),
                            },
                        )
                    })
                    .collect(),
            ),
            event_tx,
            event_rx: Mutex::new(event_rx),
            dropped_events: AtomicU64::new(persisted.dropped_events),
        }
    }

    pub fn persisted_snapshot(&self) -> PersistedStats {
        let routes = self
            .route_stats
            .read()
            .unwrap()
            .iter()
            .map(|(host, route)| {
                (
                    host.clone(),
                    PersistedRouteStats {
                        total_requests: route.total_requests.load(Ordering::Relaxed),
                        total_latency_ms: route.total_latency_ms.load(Ordering::Relaxed),
                    },
                )
            })
            .collect();
        let tunnels = self
            .tunnel_stats
            .read()
            .unwrap()
            .iter()
            .map(|(id, tunnel)| {
                (
                    id.clone(),
                    PersistedTunnelStats {
                        bytes_sent: tunnel.bytes_sent.load(Ordering::Relaxed),
                        bytes_received: tunnel.bytes_received.load(Ordering::Relaxed),
                    },
                )
            })
            .collect();

        PersistedStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            status_2xx: self.status_2xx.load(Ordering::Relaxed),
            status_3xx: self.status_3xx.load(Ordering::Relaxed),
            status_4xx: self.status_4xx.load(Ordering::Relaxed),
            status_5xx: self.status_5xx.load(Ordering::Relaxed),
            dropped_events: self.dropped_events.load(Ordering::Relaxed),
            routes,
            tunnels,
        }
    }

    fn emit(&self, event: StatsEvent) {
        if self.event_tx.try_send(event).is_err() {
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn flush_events(&self) {
        let mut pending = PendingStats::default();

        {
            let mut event_rx = self.event_rx.lock().unwrap();
            for _ in 0..MAX_FLUSH_EVENTS {
                match event_rx.try_recv() {
                    Ok(event) => pending.record(event),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                }
            }
        }

        if pending.is_empty() {
            return;
        }

        self.total_requests
            .fetch_add(pending.total_requests, Ordering::Relaxed);
        self.status_2xx
            .fetch_add(pending.status_2xx, Ordering::Relaxed);
        self.status_3xx
            .fetch_add(pending.status_3xx, Ordering::Relaxed);
        self.status_4xx
            .fetch_add(pending.status_4xx, Ordering::Relaxed);
        self.status_5xx
            .fetch_add(pending.status_5xx, Ordering::Relaxed);

        if !pending.routes.is_empty() {
            let mut guard = self.route_stats.write().unwrap();
            for (host, pending_stats) in pending.routes {
                let stats = guard
                    .entry(host)
                    .or_insert_with(|| RouteStats::new(pending_stats.online));
                stats
                    .total_requests
                    .fetch_add(pending_stats.total_requests, Ordering::Relaxed);
                stats
                    .total_latency_ms
                    .fetch_add(pending_stats.total_latency_ms, Ordering::Relaxed);
                stats.online.store(pending_stats.online, Ordering::Relaxed);
            }
        }

        if !pending.tunnels.is_empty() {
            let mut guard = self.tunnel_stats.write().unwrap();
            for (id, pending_stats) in pending.tunnels {
                let stats = guard.entry(id).or_insert_with(|| TunnelStats {
                    bytes_sent: AtomicU64::new(0),
                    bytes_received: AtomicU64::new(0),
                });
                stats
                    .bytes_sent
                    .fetch_add(pending_stats.bytes_sent, Ordering::Relaxed);
                stats
                    .bytes_received
                    .fetch_add(pending_stats.bytes_received, Ordering::Relaxed);
            }
        }
    }

    pub fn get_snapshot(&self) -> StatsResponse {
        let mut routes_map = HashMap::new();
        {
            let guard = self.route_stats.read().unwrap();
            for (host, rstats) in guard.iter() {
                routes_map.insert(host.clone(), rstats.get_snapshot());
            }
        }
        let mut tunnels_map = HashMap::new();
        {
            let guard = self.tunnel_stats.read().unwrap();
            for (id, tstats) in guard.iter() {
                tunnels_map.insert(
                    id.clone(),
                    TunnelStatsSnapshot {
                        bytes_sent: tstats.bytes_sent.load(Ordering::Relaxed),
                        bytes_received: tstats.bytes_received.load(Ordering::Relaxed),
                    },
                );
            }
        }

        StatsResponse {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            status_2xx: self.status_2xx.load(Ordering::Relaxed),
            status_3xx: self.status_3xx.load(Ordering::Relaxed),
            status_4xx: self.status_4xx.load(Ordering::Relaxed),
            status_5xx: self.status_5xx.load(Ordering::Relaxed),
            dropped_events: self.dropped_events.load(Ordering::Relaxed),
            routes: routes_map,
            tunnels: tunnels_map,
        }
    }

    pub fn record_request(&self, status: u16) {
        self.emit(StatsEvent::Request { status });
    }

    pub fn record_route_request(&self, host: &str, latency_ms: u64, is_connection_failure: bool) {
        self.emit(StatsEvent::RouteRequest {
            host: host.to_string(),
            latency_ms,
            is_connection_failure,
        });
    }

    pub fn record_tunnel_traffic(&self, id: &str, sent: u64, received: u64) {
        self.emit(StatsEvent::TunnelTraffic {
            id: id.to_string(),
            sent,
            received,
        });
    }
}
