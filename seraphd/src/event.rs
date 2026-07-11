use tokio::sync::broadcast;

#[derive(Debug, Clone, serde::Serialize)]
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
}

impl Event {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::RequestHit { .. } => "request_hit",
            Self::RequestMiss { .. } => "request_miss",
            Self::RouteAdded { .. } => "route_added",
            Self::RouteDeleted { .. } => "route_deleted",
            Self::CertRegistered { .. } => "cert_registered",
        }
    }
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestHit { host, method, path, upstream } => {
                write!(f, "Proxy: {} {} {} -> {}", host, method, path, upstream)
            }
            Self::RequestMiss { host, method, path } => {
                write!(f, "Proxy 404: {} {} {} (No route)", host, method, path)
            }
            Self::RouteAdded { key } => {
                write!(f, "Route for {} was added", key)
            }
            Self::RouteDeleted { key } => {
                write!(f, "Route for {} was deleted", key)
            }
            Self::CertRegistered { sni } => {
                write!(f, "Certificate registered successfully for {}", sni)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx }
    }

    pub fn publish(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
