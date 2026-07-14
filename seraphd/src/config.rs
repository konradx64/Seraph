use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(author, version, about = "Seraph reverse proxy and tunnel server")]
pub struct AppConfig {
    /// Address for the HTTP proxy listener
    #[arg(long, env = "SERAPHD_HTTP_ADDR", default_value = "0.0.0.0:8080")]
    pub http_addr: String,

    /// Address for the HTTPS proxy listener
    #[arg(long, env = "SERAPHD_HTTPS_ADDR", default_value = "0.0.0.0:8443")]
    pub https_addr: String,

    /// Public HTTPS port used in HTTP-to-HTTPS redirects
    #[arg(long, env = "SERAPHD_HTTPS_REDIRECT_PORT", default_value_t = 443)]
    pub https_redirect_port: u16,

    /// Address for the admin API and dashboard listener
    #[arg(long, env = "SERAPHD_ADMIN_ADDR", default_value = "127.0.0.1:9090")]
    pub admin_addr: String,

    /// Password for the admin dashboard and control API
    #[arg(long, env = "SERAPHD_ADMIN_KEY", hide_env_values = true)]
    pub admin_key: String,

    /// Directory for the database, TLS certificates, and tunnel CA
    #[arg(long, env = "SERAPHD_DATA_DIR", default_value = "data")]
    pub data_dir: String,

    /// Address for the QUIC tunnel listener
    #[arg(long, env = "SERAPHD_TUNNEL_ADDR", default_value = "0.0.0.0:7700")]
    pub tunnel_addr: String,
}
