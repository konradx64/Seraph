use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct StoredCert {
    pub sni: String,
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    pub acme_email: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Metadata {
    sni: String,
    acme_email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CertificateStore {
    root: PathBuf,
}

impl CertificateStore {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let root = data_dir.join("certs");
        crate::secure_fs::create_private_dir(&root)
            .context("failed to create TLS certificate directory")?;
        Ok(Self { root })
    }

    fn entry_dir(&self, sni: &str) -> PathBuf {
        let encoded = sni
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        self.root.join(encoded)
    }

    pub fn save(
        &self,
        sni: &str,
        cert_pem: &[u8],
        key_pem: &[u8],
        acme_email: Option<&str>,
    ) -> Result<()> {
        let dir = self.entry_dir(sni);
        crate::secure_fs::create_private_dir(&dir)?;
        std::fs::write(dir.join("cert.pem"), cert_pem)?;
        crate::secure_fs::write_private_file(&dir.join("key.pem"), key_pem)?;
        let metadata = Metadata {
            sni: sni.to_string(),
            acme_email: acme_email.map(str::to_string),
        };
        std::fs::write(
            dir.join("metadata.json"),
            serde_json::to_vec_pretty(&metadata)?,
        )?;
        Ok(())
    }

    pub fn load_all(&self) -> Result<Vec<StoredCert>> {
        let mut certs = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let dir = entry?.path();
            if !dir.is_dir() {
                continue;
            }
            let metadata: Metadata =
                serde_json::from_slice(&std::fs::read(dir.join("metadata.json"))?)?;
            crate::secure_fs::restrict_private_file(&dir.join("key.pem"))?;
            certs.push(StoredCert {
                sni: metadata.sni,
                cert_pem: std::fs::read(dir.join("cert.pem"))?,
                key_pem: std::fs::read(dir.join("key.pem"))?,
                acme_email: metadata.acme_email,
            });
        }
        Ok(certs)
    }
}
