use pingora::upstreams::peer::HttpPeer;
use pingora_proxy::{ProxyHttp, Session};

use crate::{state::AppState, tunnel::peer::TunnelPeer};
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

pub struct ReqContext {
    pub start_time: std::time::Instant,
    pub matched_host: Option<String>,
}

#[async_trait]
impl ProxyHttp for WebProxyHandler {
    type CTX = ReqContext;
    fn new_ctx(&self) -> Self::CTX {
        ReqContext {
            start_time: std::time::Instant::now(),
            matched_host: None,
        }
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let path = session.req_header().uri.path().to_string();

        // 1. Serve ACME challenge first
        if path.starts_with("/.well-known/acme-challenge/") {
            let token = path.trim_start_matches("/.well-known/acme-challenge/");
            let key_auth = {
                let challenges = self.state.acme_challenges.read().unwrap();
                challenges.get(token).cloned()
            };
            if let Some(auth) = key_auth {
                tracing::info!("Serving ACME challenge for token: {}", token);
                let mut response =
                    pingora::http::ResponseHeader::build(pingora::http::StatusCode::OK, Some(4))
                        .unwrap();
                response
                    .insert_header("Content-Type", "text/plain")
                    .unwrap();
                response
                    .insert_header("Content-Length", auth.len().to_string())
                    .unwrap();

                session
                    .write_response_header(Box::new(response), false)
                    .await?;
                session
                    .write_response_body(Some(bytes::Bytes::from(auth.into_bytes())), true)
                    .await?;
                return Ok(true);
            }
        }

        // 2. Fetch Host header to verify redirection requirements
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
            .unwrap_or("")
            .to_string();

        let routes = self.state.routes.load();
        let matched = routes.match_route(&host, &path);

        if let Some(route) = matched {
            ctx.matched_host = Some(route.hostname.clone());

            // 3. Force HTTP -> HTTPS redirect if TlsMode::Enforced and client connects over plain HTTP
            let is_tls = session.req_header().uri.scheme_str() == Some("https");
            if route.tls == crate::route::TlsMode::Enforced && !is_tls {
                tracing::info!("Redirecting HTTP request to HTTPS for host: {}", host);

                let mut response = pingora::http::ResponseHeader::build(
                    pingora::http::StatusCode::MOVED_PERMANENTLY,
                    Some(0),
                )
                .unwrap();

                let query = session
                    .req_header()
                    .uri
                    .query()
                    .map(|q| format!("?{}", q))
                    .unwrap_or_default();
                let location = format!("https://{}:8443{}{}", host, path, query);

                response.insert_header("Location", location).unwrap();
                response.insert_header("Content-Length", "0").unwrap();

                session
                    .write_response_header(Box::new(response), true)
                    .await?;

                let _ = self.state.events.send(crate::event::Event::RequestHit {
                    host: host.to_string(),
                    method: session.req_header().method.to_string(),
                    path: path.to_string(),
                    upstream: "HTTP-to-HTTPS-Redirect".to_string(),
                });

                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        let path = session.req_header().uri.path();
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

        if let Some(route) = &matched {
            // Block HTTPS connections if route is configured as HTTP only
            let is_tls = session.req_header().uri.scheme_str() == Some("https");
            if route.tls == crate::route::TlsMode::Disabled && is_tls {
                return Err(pingora::Error::explain(
                    pingora::ErrorType::HTTPStatus(400),
                    "HTTPS not supported for this host",
                ));
            }
        }

        let mut peer = None;
        if let Some(route) = &matched {
            ctx.matched_host = Some(route.hostname.clone());
            if let Some(tunnel_id) = &route.tunnel {
                let tunnel_peer = TunnelPeer::new(
                    self.state.clone(),
                    tunnel_id,
                    &route.upstream,
                    route.upstream_tls,
                    &route.hostname,
                );
                peer = Some(Box::new(tunnel_peer.into_http_peer()));
            } else {
                peer = Some(Box::new(HttpPeer::new(
                    &route.upstream,
                    route.upstream_tls,
                    route.hostname.clone(),
                )));
            }
        }

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

    async fn logging(
        &self,
        session: &mut Session,
        e: Option<&pingora::Error>,
        ctx: &mut Self::CTX,
    ) {
        let status = session
            .response_written()
            .map(|resp| resp.status.as_u16())
            .unwrap_or(502);

        self.state.stats.record_request(status);

        if let Some(host) = &ctx.matched_host {
            let latency_ms = ctx.start_time.elapsed().as_millis() as u64;
            let is_connection_failure = e.is_some() || status == 502 || status == 504;
            self.state
                .stats
                .record_route_request(host, latency_ms, is_connection_failure);
        }
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        if let Some(host) = &ctx.matched_host {
            let routes = self.state.routes.load();
            if let Some(route) = routes.match_route(host, "") {
                if route.forward_ip {
                    if let Some(client_addr) = session.client_addr() {
                        let client_ip = match client_addr.as_inet() {
                            Some(inet) => inet.ip().to_string(),
                            None => client_addr.to_string(),
                        };
                        let _ = upstream_request.insert_header("X-Real-IP", &client_ip);
                        let _ = upstream_request.insert_header("X-Forwarded-For", &client_ip);
                    }

                    let is_tls = session.req_header().uri.scheme_str() == Some("https");
                    let proto = if is_tls { "https" } else { "http" };
                    let _ = upstream_request.insert_header("X-Forwarded-Proto", proto);
                }
            }
        }
        Ok(())
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut pingora::http::ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        if let Some(host) = &ctx.matched_host {
            let routes = self.state.routes.load();
            if let Some(route) = routes.match_route(host, "") {
                if route.hsts {
                    let _ = upstream_response.insert_header(
                        "Strict-Transport-Security",
                        "max-age=63072000; includeSubDomains; preload",
                    );
                }

                if let Some(origins) = &route.cors_origins {
                    if !origins.is_empty() {
                        let _ =
                            upstream_response.insert_header("Access-Control-Allow-Origin", origins);
                        let _ = upstream_response.insert_header(
                            "Access-Control-Allow-Methods",
                            "GET, POST, PUT, DELETE, OPTIONS, HEAD, PATCH",
                        );
                        let _ = upstream_response.insert_header(
                            "Access-Control-Allow-Headers",
                            "Content-Type, Authorization, X-Requested-With",
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
