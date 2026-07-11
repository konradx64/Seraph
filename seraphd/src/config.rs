use super::route::{Route, TlsMode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub path_prefix: Option<String>,
    pub upstream: String,
    pub tunnel: Option<String>,
    #[serde(default)]
    pub tls: TlsMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub http_addr: String,
    pub https_addr: String,
    pub admin_addr: String,
    #[serde(flatten)]
    pub hostnames: HashMap<String, RouteConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            http_addr: "0.0.0.0:8080".to_string(),
            https_addr: "0.0.0.0:8443".to_string(),
            admin_addr: "127.0.0.1:9090".to_string(),
            hostnames: HashMap::new(),
        }
    }
}

impl AppConfig {
    pub fn routes(&self) -> Vec<Route> {
        self.hostnames
            .iter()
            .map(|(hostname, config)| Route {
                hostname: hostname.clone(),
                path_prefix: config.path_prefix.clone(),
                upstream: crate::route::clean_upstream(&config.upstream),
                tunnel: config.tunnel.clone(),
                tls: config.tls.clone(),
            })
            .collect()
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let toml_str = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&toml_str)?;
        Ok(config)
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let toml_str = toml::to_string_pretty(self)?;
        fs::write(path, toml_str)?;
        Ok(())
    }
}
