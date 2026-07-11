use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub hostname: String,
    pub path_prefix: Option<String>,
    pub upstream: String,
    pub tunnel: Option<String>,
    pub tls: TlsMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum TlsMode {
    Off,
    #[default]
    Auto,
    Manual {
        certificate_id: String,
    },
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
            tls: TlsMode::Auto,
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
