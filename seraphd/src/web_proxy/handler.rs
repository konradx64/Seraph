use pingora::upstreams::peer::HttpPeer;
use pingora_proxy::{ProxyHttp, Session};

use crate::{state::AppState, tunnel::peer::TunnelPeer};
use async_trait::async_trait;
use std::{net::IpAddr, sync::Arc};

pub struct WebProxyHandler {
    state: Arc<AppState>,
}

impl WebProxyHandler {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

fn downstream_is_tls(session: &Session) -> bool {
    session
        .as_downstream()
        .digest()
        .and_then(|digest| digest.ssl_digest.as_ref())
        .is_some()
}

fn parse_forwarded_for(value: &str) -> Option<IpAddr> {
    value
        .split(',')
        .map(str::trim)
        .find_map(|candidate| candidate.parse().ok())
}

fn forwarded_client_ip(session: &Session) -> Option<IpAddr> {
    session
        .req_header()
        .headers
        .get("X-Forwarded-For")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_forwarded_for)
        .or_else(|| {
            session
                .req_header()
                .headers
                .get("X-Real-IP")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse().ok())
        })
}

pub struct ReqContext {
    pub start_time: std::time::Instant,
    pub matched_host: Option<String>,
    pub upstream: Option<String>,
    pub request_host: Option<String>,
    pub request_method: Option<String>,
    pub request_path: Option<String>,
    pub forward_ip: bool,
    pub client_ip: Option<std::net::IpAddr>,
    pub geo_lookup: Option<crate::geoip::GeoLookupResult>,
}

#[async_trait]
impl ProxyHttp for WebProxyHandler {
    type CTX = ReqContext;
    fn new_ctx(&self) -> Self::CTX {
        ReqContext {
            start_time: std::time::Instant::now(),
            matched_host: None,
            upstream: None,
            request_host: None,
            request_method: None,
            request_path: None,
            forward_ip: false,
            client_ip: None,
            geo_lookup: None,
        }
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let peer_ip = session
            .client_addr()
            .and_then(|addr| addr.as_inet())
            .map(|inet| inet.ip());
        let client_ip = if self.state.config.trust_proxy_headers {
            forwarded_client_ip(session).or(peer_ip)
        } else {
            peer_ip
        };
        let geo_lookup = client_ip.and_then(|ip| self.state.geoip.lookup(ip));

        ctx.client_ip = client_ip;
        ctx.geo_lookup = geo_lookup;

        let path = session.req_header().uri.path().to_string();

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

        ctx.request_host = Some(host.clone());
        ctx.request_method = Some(session.req_header().method.to_string());
        ctx.request_path = Some(path.clone());

        let routes = self.state.routes.load();
        let matched = routes.match_route(&host, &path);

        if let Some(route) = matched {
            ctx.matched_host = Some(route.hostname.clone());
            ctx.upstream = Some(route.upstream.clone());
            ctx.forward_ip = route.forward_ip;

            let is_tls = downstream_is_tls(session);
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
                let authority = if self.state.config.https_redirect_port == 443 {
                    host.clone()
                } else {
                    format!("{}:{}", host, self.state.config.https_redirect_port)
                };
                let location = format!("https://{}{}{}", authority, path, query);

                response.insert_header("Location", location).unwrap();
                response.insert_header("Content-Length", "0").unwrap();

                session
                    .write_response_header(Box::new(response), true)
                    .await?;

                ctx.upstream = Some("HTTP-to-HTTPS-Redirect".to_string());

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

        let is_tls = downstream_is_tls(session);
        if let Some(route) = &matched
            && route.tls == crate::route::TlsMode::Disabled
            && is_tls
        {
            return Err(pingora::Error::explain(
                pingora::ErrorType::HTTPStatus(400),
                "HTTPS not supported for this host",
            ));
        }

        let mut peer = None;
        if let Some(route) = &matched {
            ctx.matched_host = Some(route.hostname.clone());
            ctx.upstream = Some(route.upstream.clone());
            ctx.forward_ip = route.forward_ip;
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
        let latency_ms = ctx.start_time.elapsed().as_millis() as u64;

        self.state.stats.record_request(status);

        if let Some(host) = &ctx.matched_host {
            let is_connection_failure = e.is_some() || status == 502 || status == 504;
            self.state
                .stats
                .record_route_request(host, latency_ms, is_connection_failure);
        }

        if let (Some(host), Some(method), Some(path)) = (
            ctx.request_host.clone(),
            ctx.request_method.clone(),
            ctx.request_path.clone(),
        ) {
            let client_ip = ctx.client_ip.map(|ip| ip.to_string());
            let client_country = ctx
                .geo_lookup
                .as_ref()
                .and_then(|geo| geo.country_code.clone());
            let client_city = ctx.geo_lookup.as_ref().and_then(|geo| geo.city.clone());
            let client_latitude = ctx.geo_lookup.as_ref().and_then(|geo| geo.latitude);
            let client_longitude = ctx.geo_lookup.as_ref().and_then(|geo| geo.longitude);

            let event = if ctx.matched_host.is_some() {
                crate::event::Event::RequestHit {
                    host,
                    method,
                    path,
                    upstream: ctx
                        .upstream
                        .clone()
                        .unwrap_or_else(|| "Rejected by proxy".to_string()),
                    status_code: status,
                    latency_ms,
                    client_ip,
                    client_country,
                    client_city,
                    client_latitude,
                    client_longitude,
                }
            } else {
                crate::event::Event::RequestMiss {
                    host,
                    method,
                    path,
                    status_code: status,
                    latency_ms,
                    client_ip,
                    client_country,
                    client_city,
                    client_latitude,
                    client_longitude,
                }
            };
            let _ = self.state.events.send(event);
        }
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        if let Some(host) = &ctx.matched_host {
            if ctx.forward_ip
                && let Some(client_addr) = session.client_addr()
            {
                let client_ip = client_addr
                    .as_inet()
                    .map(|inet| inet.ip().to_string())
                    .unwrap_or_else(|| client_addr.to_string());
                let _ = upstream_request.insert_header("X-Real-IP", &client_ip);
                let _ = upstream_request.insert_header("X-Forwarded-For", &client_ip);
            }

            let is_tls = downstream_is_tls(session);
            let proto = if is_tls { "https" } else { "http" };
            let _ = upstream_request.insert_header("X-Forwarded-Proto", proto);
            let _ = upstream_request.insert_header("X-Forwarded-Host", host);
            let forwarded_port = if is_tls {
                self.state.config.https_redirect_port
            } else {
                80
            };
            let _ = upstream_request.insert_header("X-Forwarded-Port", forwarded_port.to_string());
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

                if let Some(origins) = &route.cors_origins
                    && !origins.is_empty()
                {
                    let _ = upstream_response.insert_header("Access-Control-Allow-Origin", origins);
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
        Ok(())
    }
}
