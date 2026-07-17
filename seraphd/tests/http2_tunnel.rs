use axum::{Router, extract::Request, http::HeaderValue, response::Response, routing::get};
use quinn::{Endpoint, RecvStream, SendStream};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use seraphd::{
    cert_store::CertificateStore,
    config::AppConfig,
    db::Database,
    registry::{CertificateRegistry, RouteRegistry},
    route::{Route, TlsMode},
    state::AppState,
    stats::{PersistedStats, Stats},
    tunnel::{ca::TunnelCa, listener::QuicTunnelService},
    web_proxy::create_proxy_service,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};

const HOST: &str = "session.test";
const AGENT_ID: &str = "integration-agent";
const REQUESTS: usize = 64;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_http2_requests_keep_tunnel_responses_and_cookies_isolated() {
    let temp = tempfile::tempdir().unwrap();
    let upstream_addr = free_tcp_addr();
    let https_addr = free_tcp_addr();
    let tunnel_addr = free_udp_addr();

    tokio::spawn(run_echo_upstream(upstream_addr));

    let ca = TunnelCa::load_or_create(temp.path()).unwrap();
    let (agent_cert, agent_key) = ca.issue_agent_cert(AGENT_ID).unwrap();
    let ca_pem = ca.cert_pem.clone();

    let (server_cert, server_key) = server_certificate();
    let mut certificates = CertificateRegistry::new();
    certificates
        .register(HOST, server_cert.as_bytes(), server_key.as_bytes())
        .unwrap();

    let route = Route {
        hostname: HOST.to_string(),
        path_prefix: None,
        upstream: upstream_addr.to_string(),
        tunnel: Some(AGENT_ID.to_string()),
        tls: TlsMode::Enabled,
        upstream_tls: false,
        hsts: false,
        cors_origins: None,
        forward_ip: true,
    };
    let config = AppConfig {
        http_addr: free_tcp_addr().to_string(),
        https_addr: https_addr.to_string(),
        https_redirect_port: https_addr.port(),
        http2: true,
        admin_addr: free_tcp_addr().to_string(),
        admin_key: "test".to_string(),
        data_dir: temp.path().display().to_string(),
        tunnel_addr: tunnel_addr.to_string(),
        geoip_db: None,
        trust_proxy_headers: false,
    };
    let database_path = temp.path().join("test.db");
    let state = Arc::new(AppState::new(
        config,
        Database::open(&database_path.to_string_lossy()).unwrap(),
        CertificateStore::new(temp.path()).unwrap(),
        RouteRegistry::new(vec![route]),
        certificates,
        ca,
        Stats::from_persisted(PersistedStats::default()),
        seraphd::geoip::GeoIpService::new(None),
    ));

    start_seraph(state.clone());
    let agent = tokio::spawn(run_agent(
        tunnel_addr,
        agent_cert,
        agent_key,
        ca_pem,
    ));

    wait_for_tunnel(&state).await;

    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .danger_accept_invalid_certs(true)
        .resolve(HOST, SocketAddr::new(https_addr.ip(), https_addr.port()))
        .build()
        .unwrap();

    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..REQUESTS {
        let client = client.clone();
        tasks.spawn(async move {
            let request_id = format!("request-{index}");
            let cookie = format!("session=session-{index}; passphrase=passphrase-{index}");
            let response = client
                .get(format!("https://{HOST}/echo"))
                .header("x-request-id", &request_id)
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap();

            assert_eq!(response.version(), reqwest::Version::HTTP_2);
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            assert_eq!(response.headers()["x-seen-request-id"], request_id);
            assert_eq!(response.headers()["x-seen-cookie"], cookie);
            assert_eq!(response.headers()["x-seen-forwarded-proto"], "https");
            assert_eq!(response.headers()["x-seen-forwarded-host"], HOST);

            let set_cookies: Vec<_> = response
                .headers()
                .get_all("set-cookie")
                .iter()
                .map(|value| value.to_str().unwrap().to_string())
                .collect();
            assert_eq!(
                set_cookies,
                vec![
                    format!("session=response-session-{index}; Path=/; Secure; HttpOnly"),
                    format!("passphrase=response-passphrase-{index}; Path=/; Secure; HttpOnly"),
                ]
            );
        });
    }

    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    agent.abort();
}

async fn run_echo_upstream(addr: SocketAddr) {
    let app = Router::new().route(
        "/echo",
        get(|request: Request| async move {
            let request_id = request.headers()["x-request-id"].to_str().unwrap();
            let cookie = request.headers()["cookie"].to_str().unwrap();
            let forwarded_proto = request.headers()["x-forwarded-proto"].clone();
            let forwarded_host = request.headers()["x-forwarded-host"].clone();
            let index = request_id.strip_prefix("request-").unwrap();
            let mut response = Response::new(axum::body::Body::empty());
            response
                .headers_mut()
                .insert("x-seen-request-id", HeaderValue::from_str(request_id).unwrap());
            response
                .headers_mut()
                .insert("x-seen-cookie", HeaderValue::from_str(cookie).unwrap());
            response
                .headers_mut()
                .insert("x-seen-forwarded-proto", forwarded_proto);
            response
                .headers_mut()
                .insert("x-seen-forwarded-host", forwarded_host);
            response.headers_mut().append(
                "set-cookie",
                HeaderValue::from_str(&format!(
                    "session=response-session-{index}; Path=/; Secure; HttpOnly"
                ))
                .unwrap(),
            );
            response.headers_mut().append(
                "set-cookie",
                HeaderValue::from_str(&format!(
                    "passphrase=response-passphrase-{index}; Path=/; Secure; HttpOnly"
                ))
                .unwrap(),
            );
            response
        }),
    );
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn start_seraph(state: Arc<AppState>) {
    std::thread::spawn(move || {
        use pingora::{server::Server, services::background::background_service};

        let mut server = Server::new(None).unwrap();
        server.bootstrap();
        server.add_service(background_service(
            "test_quic_tunnel",
            QuicTunnelService::new(state.clone()),
        ));
        server.add_service(create_proxy_service(&server.configuration, state).unwrap());
        server.run_forever();
    });
}

async fn run_agent(
    tunnel_addr: SocketAddr,
    cert_pem: String,
    key_pem: String,
    ca_pem: String,
) {
    let client_config = agent_client_config(&cert_pem, &key_pem, &ca_pem);
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut endpoint = Endpoint::client(bind_addr).unwrap();
    endpoint.set_default_client_config(client_config);

    let connection = loop {
        match endpoint.connect(tunnel_addr, "seraph-tunnel") {
            Ok(connecting) => match connecting.await {
                Ok(connection) => break connection,
                Err(_) => tokio::time::sleep(Duration::from_millis(25)).await,
            },
            Err(_) => tokio::time::sleep(Duration::from_millis(25)).await,
        }
    };

    while let Ok((send, recv)) = connection.accept_bi().await {
        tokio::spawn(async move {
            bridge_stream(send, recv).await.unwrap();
        });
    }
}

async fn bridge_stream(mut send: SendStream, mut recv: RecvStream) -> anyhow::Result<()> {
    let mut destination = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        recv.read_exact(&mut byte).await?;
        if byte[0] == b'\n' {
            break;
        }
        destination.push(byte[0]);
    }
    let destination = String::from_utf8(destination)?;
    let mut upstream = tokio::net::TcpStream::connect(destination).await?;
    let (mut upstream_read, mut upstream_write) = upstream.split();
    tokio::try_join!(
        tokio::io::copy(&mut recv, &mut upstream_write),
        tokio::io::copy(&mut upstream_read, &mut send)
    )?;
    Ok(())
}

fn agent_client_config(cert_pem: &str, key_pem: &str, ca_pem: &str) -> quinn::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    let mut ca_reader = std::io::Cursor::new(ca_pem);
    for cert in rustls_pemfile::certs(&mut ca_reader) {
        roots.add(cert.unwrap()).unwrap();
    }
    let mut cert_reader = std::io::Cursor::new(cert_pem);
    let certs = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut key_reader = std::io::Cursor::new(key_pem);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .unwrap()
        .unwrap();
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_client_auth_cert(certs, key)
        .unwrap();
    quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls).unwrap(),
    ))
}

async fn wait_for_tunnel(state: &AppState) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if state.active_tunnels.read().await.contains_key(AGENT_ID) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("agent did not connect");
}

fn server_certificate() -> (String, String) {
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(vec![HOST.to_string()]).unwrap();
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, HOST);
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

fn free_tcp_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap()
}

fn free_udp_addr() -> SocketAddr {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.local_addr().unwrap()
}
