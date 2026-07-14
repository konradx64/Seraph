use anyhow::{Context, Result};
use clap::Parser;
use rcgen::KeyPair;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about = "Seraph Reverse Tunnel Agent")]
struct Args {
    /// Address of the Seraph gateway control API (e.g. http://127.0.0.1:9090)
    #[arg(short, long, default_value = "http://127.0.0.1:9090")]
    server: String,

    /// UDP address of the Seraph QUIC tunnel server (e.g. 127.0.0.1:7700)
    /// If not specified, we will attempt to parse the host from the server URL on port 7700.
    #[arg(short, long)]
    tunnel_addr: Option<String>,

    /// Enrollment key generated in the dashboard (required on first run)
    #[arg(short, long)]
    key: Option<String>,

    /// Directory to store certificates and keys
    #[arg(short, long, default_value = "./agent-data")]
    data_dir: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let mut server_url = args.server.trim().to_string();
    if !server_url.starts_with("http://") && !server_url.starts_with("https://") {
        server_url = format!("http://{}", server_url);
    }

    let data_dir_path = Path::new(&args.data_dir);
    let key_path = data_dir_path.join("identity.key");
    let cert_path = data_dir_path.join("identity.crt");
    let ca_path = data_dir_path.join("ca.crt");
    create_private_dir(data_dir_path).context("Failed to secure agent data directory")?;

    // 1. Perform enrollment if identity files do not exist
    if !key_path.exists() || !cert_path.exists() || !ca_path.exists() {
        tracing::info!("No existing identity files found. Initiating enrollment...");
        let enrollment_key = args.key.context(
            "Enrollment key (--key) is required for first-time registration of the agent",
        )?;

        enroll(enrollment_key, &server_url, &key_path, &cert_path, &ca_path).await?;
    } else {
        restrict_private_file(&key_path).context("Failed to secure identity.key")?;
        tracing::info!("Identity loaded from data directory: {}", args.data_dir);
    }

    // 2. Load mTLS credentials
    let client_config = build_client_config(&key_path, &cert_path, &ca_path)?;

    // 3. Resolve tunnel destination address
    let tunnel_destination = match args.tunnel_addr {
        Some(addr) => addr,
        None => {
            let url = reqwest::Url::parse(&server_url).context("Invalid server URL")?;
            let host = url.host_str().context("No host found in server URL")?;
            format!("{}:7700", host)
        }
    };

    let tunnel_sock_addr = tokio::net::lookup_host(&tunnel_destination)
        .await
        .context("Failed to resolve tunnel destination address")?
        .next()
        .context("No socket addresses found for tunnel destination")?;

    tracing::info!("Connecting to Seraph QUIC Tunnel at {}", tunnel_destination);

    // Build Quinn client endpoint matching IP version of the target destination
    let bind_addr = if tunnel_sock_addr.is_ipv6() {
        SocketAddr::from(([0u8; 16], 0))
    } else {
        SocketAddr::from(([0u8; 4], 0))
    };
    let mut endpoint = quinn::Endpoint::client(bind_addr)?;
    let mut transport = quinn::TransportConfig::default();
    transport.max_concurrent_bidi_streams(quinn::VarInt::from_u32(2048));
    transport.max_idle_timeout(Some(quinn::VarInt::from_u32(30_000).into()));
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(10)));

    let mut quinn_client_config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(client_config)
            .context("Failed to build Quinn client config")?,
    ));
    quinn_client_config.transport_config(Arc::new(transport));
    endpoint.set_default_client_config(quinn_client_config);

    // Connect to the server
    let connection = endpoint
        .connect(tunnel_sock_addr, "seraph-tunnel")?
        .await
        .context("Failed to establish QUIC connection to Seraph gateway")?;

    tracing::info!("Tunnel successfully established with Seraph server!");

    // 4. Accept loop
    while let Ok((send_stream, recv_stream)) = connection.accept_bi().await {
        tokio::spawn(async move {
            if let Err(e) = handle_stream(send_stream, recv_stream).await {
                tracing::warn!("Stream bridge handling exited with error: {:?}", e);
            }
        });
    }

    tracing::warn!("QUIC connection closed");
    Ok(())
}

// ---------------------------------------------------------------------------
// Enrollment - sign keypair and fetch certificate
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct EnrollPayload {
    token: String,
    csr: String,
}

#[derive(serde::Deserialize)]
struct EnrollResponse {
    certificate: String,
    ca_certificate: String,
}

async fn enroll(
    token: String,
    server_url: &str,
    key_path: &Path,
    cert_path: &Path,
    ca_path: &Path,
) -> Result<()> {
    tracing::info!("Generating local key pair...");
    let key_pair = KeyPair::generate().context("Failed to generate private key")?;
    let key_pem = key_pair.serialize_pem();

    // Create a temporary CSR
    tracing::info!("Creating Certificate Signing Request (CSR)...");
    let mut params = rcgen::CertificateParams::new(vec![])?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "seraph-agent-enrollment");
    let csr_pem = params.serialize_request(&key_pair)?.pem()?;

    // Submit the enrollment request
    let enroll_url = format!("{}/api/tunnels/enroll", server_url.trim_end_matches('/'));
    tracing::info!("Submitting CSR to {}...", enroll_url);

    let client = reqwest::Client::new();
    let response = client
        .post(&enroll_url)
        .json(&EnrollPayload {
            token,
            csr: csr_pem,
        })
        .send()
        .await
        .context("Enrollment network request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let err_body = response.text().await.unwrap_or_default();
        anyhow::bail!("Enrollment failed ({}): {}", status, err_body);
    }

    let enroll_res: EnrollResponse = response
        .json()
        .await
        .context("Failed to parse enrollment response")?;

    // Persist keys and certs to data_dir
    write_private_file(key_path, &key_pem).context("Failed to save identity.key")?;
    std::fs::write(cert_path, &enroll_res.certificate).context("Failed to save identity.crt")?;
    std::fs::write(ca_path, &enroll_res.ca_certificate).context("Failed to save ca.crt")?;

    tracing::info!("Enrollment complete! Credentials persisted to disk.");
    Ok(())
}

fn create_private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private_file(path: &Path, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents.as_ref())?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn restrict_private_file(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

// ---------------------------------------------------------------------------
// Stream Bridge - read target header, connect locally, copy bytes
// ---------------------------------------------------------------------------

async fn handle_stream(
    mut send_stream: quinn::SendStream,
    mut recv_stream: quinn::RecvStream,
) -> Result<()> {
    // Read target destination byte-by-byte until '\n' to avoid buffering request bytes
    let mut dest_bytes = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        recv_stream
            .read_exact(&mut byte)
            .await
            .context("Stream ended before receiving destination header")?;
        if byte[0] == b'\n' {
            break;
        }
        dest_bytes.push(byte[0]);
    }

    let destination = String::from_utf8(dest_bytes)
        .context("Invalid UTF-8 destination address")?
        .trim()
        .to_string();

    tracing::debug!("Connecting local bridge to: {}", destination);

    // Connect to the local application socket
    let mut local_socket = tokio::net::TcpStream::connect(&destination)
        .await
        .with_context(|| format!("Failed to connect to local target address: {}", destination))?;

    // Bridge the bytes bi-directionally
    let (mut local_read, mut local_write) = local_socket.split();
    let bridge_read = tokio::io::copy(&mut recv_stream, &mut local_write);
    let bridge_write = tokio::io::copy(&mut local_read, &mut send_stream);

    tokio::try_join!(bridge_read, bridge_write)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// TLS Config Helper - mTLS setup
// ---------------------------------------------------------------------------

fn build_client_config(
    key_path: &Path,
    cert_path: &Path,
    ca_path: &Path,
) -> Result<rustls::ClientConfig> {
    let key_pem = std::fs::read_to_string(key_path).context("Failed to read private key file")?;
    let cert_pem = std::fs::read_to_string(cert_path).context("Failed to read certificate file")?;
    let ca_pem = std::fs::read_to_string(ca_path).context("Failed to read CA certificate file")?;

    // 1. Build trust store (trust server signed by CA)
    let mut root_store = rustls::RootCertStore::empty();
    let mut ca_reader = std::io::Cursor::new(ca_pem);
    for cert in rustls_pemfile::certs(&mut ca_reader) {
        root_store
            .add(cert?)
            .context("Failed to add CA cert to client trust store")?;
    }

    // 2. Load client certificate chain
    let mut cert_reader = std::io::Cursor::new(cert_pem);
    let mut certs = Vec::new();
    for cert in rustls_pemfile::certs(&mut cert_reader) {
        certs.push(cert?);
    }

    // 3. Load private key
    let mut key_reader = std::io::Cursor::new(key_pem);
    let private_key = rustls_pemfile::private_key(&mut key_reader)?
        .context("No private key found in client key file")?;

    // 4. Build TLS config
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(certs, private_key)
        .context("Failed to build client TLS config")?;

    Ok(client_config)
}
