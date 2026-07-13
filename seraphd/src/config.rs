use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

fn default_database_path() -> String {
    "seraph.db".to_string()
}

fn default_tunnel_addr() -> String {
    "0.0.0.0:7700".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub http_addr: String,
    pub https_addr: String,
    pub admin_addr: String,
    #[serde(default = "default_database_path")]
    pub database_path: String,
    #[serde(default = "default_tunnel_addr")]
    pub tunnel_addr: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            http_addr: "0.0.0.0:8080".to_string(),
            https_addr: "0.0.0.0:8443".to_string(),
            admin_addr: "127.0.0.1:9090".to_string(),
            database_path: "seraph.db".to_string(),
            tunnel_addr: "0.0.0.0:7700".to_string(),
        }
    }
}

impl AppConfig {
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

