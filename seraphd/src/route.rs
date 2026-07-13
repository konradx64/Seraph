use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub hostname: String,
    pub path_prefix: Option<String>,
    pub upstream: String,
    pub tunnel: Option<String>,
    pub tls: TlsMode,
    pub upstream_tls: bool,
    pub hsts: bool,
    pub cors_origins: Option<String>,
    pub forward_ip: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum TlsMode {
    Disabled,
    #[default]
    Enabled,
    Enforced,
}

pub fn clean_upstream(upstream: &str) -> String {
    upstream
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or(upstream)
        .to_string()
}

impl Route {
    pub fn new(hostname: impl Into<String>, upstream: impl Into<String>) -> Self {
        Self {
            hostname: hostname.into(),
            path_prefix: None,
            upstream: clean_upstream(&upstream.into()),
            tunnel: None,
            tls: TlsMode::Enabled,
            upstream_tls: false,
            hsts: false,
            cors_origins: None,
            forward_ip: true,
        }
    }

    pub fn matches(&self, host: &str, path: &str) -> bool {
        if self.hostname != host {
            return false;
        }

        match &self.path_prefix {
            Some(prefix) => path.starts_with(prefix),
            None => true,
        }
    }
}
