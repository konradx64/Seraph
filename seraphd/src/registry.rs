use crate::route::Route;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct CertPair {
    pub cert: X509,
    pub chain: Vec<X509>,
    pub key: PKey<Private>,
}

impl fmt::Debug for CertPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CertPair")
            .field("cert", &"X509 Certificate")
            .field("chain_length", &self.chain.len())
            .field("key", &"Private Key")
            .finish()
    }
}

#[derive(Default, Clone)]
pub struct CertificateRegistry {
    certs: HashMap<String, Arc<CertPair>>,
}

impl fmt::Debug for CertificateRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CertificateRegistry")
            .field("domains", &self.certs.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl CertificateRegistry {
    pub fn new() -> Self {
        Self {
            certs: HashMap::new(),
        }
    }

    pub fn register(&mut self, sni: &str, cert_pem: &[u8], key_pem: &[u8]) -> anyhow::Result<()> {
        let mut certificates = X509::stack_from_pem(cert_pem)?;
        if certificates.is_empty() {
            anyhow::bail!("certificate PEM does not contain a certificate");
        }
        let cert = certificates.remove(0);
        let chain = certificates;
        let key = PKey::private_key_from_pem(key_pem)?;
        if !cert.public_key()?.public_eq(&key) {
            anyhow::bail!("certificate and private key do not match");
        }
        self.certs
            .insert(sni.to_string(), Arc::new(CertPair { cert, chain, key }));
        Ok(())
    }

    pub fn get(&self, sni: &str) -> Option<Arc<CertPair>> {
        self.certs.get(sni).cloned()
    }
}

#[derive(Debug, Clone)]
pub struct RouteRegistry {
    routes: Vec<Route>,
}

impl RouteRegistry {
    pub fn new(routes: Vec<Route>) -> Self {
        Self { routes }
    }

    pub fn match_route(&self, host: &str, path: &str) -> Option<Route> {
        self.routes
            .iter()
            .filter(|route| route.matches(host, path))
            .max_by_key(|route| route.path_prefix.as_ref().map(|p| p.len()).unwrap_or(0))
            .cloned()
    }

    pub fn all(&self) -> &[Route] {
        &self.routes
    }
}
