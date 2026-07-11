use pingora::upstreams::peer::HttpPeer;
use pingora_proxy::{ProxyHttp, Session};

use crate::state::AppState;
use async_trait::async_trait;
use std::sync::Arc;

pub struct WebProxyHandler {
    state: Arc<AppState>,
}

impl WebProxyHandler {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl ProxyHttp for WebProxyHandler {
    type CTX = bool;
    fn new_ctx(&self) -> Self::CTX {
        false
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        let path = session.req_header().uri.path();
        let host_raw = session
            .req_header()
            .headers
            .get("Host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let host = host_raw.split(':').next().unwrap_or("");

        tracing::info!("Path: {} Host: {} (raw: {})", path, host, host_raw);

        let routes = self.state.routes.load();
        let matched = routes.match_route(host, path);

        let peer = matched.as_ref().map(|route| {
            Box::new(HttpPeer::new(&route.upstream, false, route.hostname.clone()))
        });

        match &matched {
            Some(route) => {
                self.state.events.publish(crate::event::Event::RequestHit {
                    host: host.to_string(),
                    method: session.req_header().method.to_string(),
                    path: path.to_string(),
                    upstream: route.upstream.clone(),
                });
            }
            None => {
                self.state.events.publish(crate::event::Event::RequestMiss {
                    host: host.to_string(),
                    method: session.req_header().method.to_string(),
                    path: path.to_string(),
                });
            }
        }

        match peer {
            Some(peer) => Ok(peer),
            None => Err(pingora::Error::explain(
                pingora::ErrorType::HTTPStatus(404),
                "No route configured",
            )),
        }
    }
}
