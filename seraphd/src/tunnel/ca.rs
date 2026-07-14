//! Tunnel Certificate Authority
//!
//! Generates a self-signed CA and issues client certificates for tunnel agents.

use anyhow::{Context, Result};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose,
};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

const CA_CERT_FILE: &str = "tunnel_ca.crt";
const CA_KEY_FILE: &str = "tunnel_ca.key";

pub struct CaPaths {
    pub cert: PathBuf,
    pub key: PathBuf,
}

impl CaPaths {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            cert: data_dir.join(CA_CERT_FILE),
            key: data_dir.join(CA_KEY_FILE),
        }
    }

    pub fn exist(&self) -> bool {
        self.cert.exists() && self.key.exists()
    }
}

pub struct TunnelCa {
    pub cert_pem: String,
    pub cert: Certificate,
    pub key_pair: KeyPair,
}

impl TunnelCa {
    pub fn load_or_create(data_dir: &Path) -> Result<Self> {
        let paths = CaPaths::new(data_dir);

        if paths.exist() {
            tracing::info!("Loading existing tunnel CA from disk");
            crate::secure_fs::restrict_private_file(&paths.key)
                .context("Failed to secure tunnel CA key")?;
            let cert_pem =
                std::fs::read_to_string(&paths.cert).context("Failed to read tunnel CA cert")?;
            let key_pem =
                std::fs::read_to_string(&paths.key).context("Failed to read tunnel CA key")?;

            let key_pair = KeyPair::from_pem(&key_pem).context("Failed to parse tunnel CA key")?;

            let cert = Self::build_ca_params()?.self_signed(&key_pair)?;

            Ok(Self {
                cert_pem,
                cert,
                key_pair,
            })
        } else {
            tracing::info!("Generating new tunnel CA keypair");
            let (cert, key_pair) = Self::generate_ca()?;
            let cert_pem = cert.pem();
            let key_pem = key_pair.serialize_pem();

            std::fs::create_dir_all(data_dir).context("Failed to create data directory")?;
            std::fs::write(&paths.cert, &cert_pem).context("Failed to write tunnel CA cert")?;
            crate::secure_fs::write_private_file(&paths.key, &key_pem)
                .context("Failed to write tunnel CA key")?;

            Ok(Self {
                cert_pem,
                cert,
                key_pair,
            })
        }
    }

    fn build_ca_params() -> Result<CertificateParams> {
        let mut params = CertificateParams::new(vec![])?;
        params
            .distinguished_name
            .push(DnType::CommonName, "Seraph Tunnel CA");
        params
            .distinguished_name
            .push(DnType::OrganizationName, "Seraph");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc()
            .checked_add(time::Duration::days(365 * 10))
            .context("Date overflow")?;
        Ok(params)
    }

    fn generate_ca() -> Result<(Certificate, KeyPair)> {
        let key_pair = KeyPair::generate()?;
        let cert = Self::build_ca_params()?.self_signed(&key_pair)?;
        Ok((cert, key_pair))
    }

    /// Issue a signed client certificate for a named agent.
    pub fn issue_agent_cert(&self, agent_id: &str) -> Result<(String, String)> {
        let agent_key = KeyPair::generate()?;

        let mut params = CertificateParams::new(vec![])?;
        params.distinguished_name.push(DnType::CommonName, agent_id);
        params
            .distinguished_name
            .push(DnType::OrganizationName, "Seraph Agent");
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc()
            .checked_add(time::Duration::days(365))
            .context("Date overflow")?;

        let cert = params.signed_by(&agent_key, &self.cert, &self.key_pair)?;

        Ok((cert.pem(), agent_key.serialize_pem()))
    }
}
