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

    async fn request_filter(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let path = session.req_header().uri.path();
        if path.starts_with("/.well-known/acme-challenge/") {
            let token = path.trim_start_matches("/.well-known/acme-challenge/");
            let key_auth = {
                let challenges = self.state.acme_challenges.read().unwrap();
                challenges.get(token).cloned()
            };
            if let Some(auth) = key_auth {
                tracing::info!("Serving ACME challenge for token: {}", token);
                let mut response = pingora::http::ResponseHeader::build(
                    pingora::http::StatusCode::OK,
                    Some(4),
                ).unwrap();
                response.insert_header("Content-Type", "text/plain").unwrap();
                response.insert_header("Content-Length", auth.len().to_string()).unwrap();
                
                session.write_response_header(Box::new(response), false).await?;
                session.write_response_body(Some(bytes::Bytes::from(auth.into_bytes())), true).await?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        let path = session.req_header().uri.path();

        // For HTTP/1.1 the host is in the Host header.
        // For HTTP/2 it's in the :authority pseudo-header, which Pingora
        // maps into the URI's authority component.
        let host = session
            .req_header()
            .uri
            .host()
            .or_else(|| {
                session
                    .req_header()
                    .headers
                    .get("Host")
                    .and_then(|h| h.to_str().ok())
                    .map(|h| h.split(':').next().unwrap_or(""))
            })
            .unwrap_or("");

        tracing::info!("Path: {} Host: {}", path, host);

        let routes = self.state.routes.load();
        let matched = routes.match_route(host, path);

        let peer = matched.as_ref().map(|route| {
            Box::new(HttpPeer::new(
                &route.upstream,
                false,
                route.hostname.clone(),
            ))
        });

        match &matched {
            Some(route) => {
                let _ = self.state.events.send(crate::event::Event::RequestHit {
                    host: host.to_string(),
                    method: session.req_header().method.to_string(),
                    path: path.to_string(),
                    upstream: route.upstream.clone(),
                });
            }
            None => {
                let _ = self.state.events.send(crate::event::Event::RequestMiss {
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
