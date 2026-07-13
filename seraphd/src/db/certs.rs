use super::Database;
use rusqlite::params;

pub struct DbCert {
    pub sni: String,
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    pub acme_email: Option<String>,
}

impl Database {
    pub fn load_certs(&self) -> anyhow::Result<Vec<DbCert>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT sni, cert_pem, key_pem, acme_email FROM certificates")?;
        let cert_iter = stmt.query_map([], |row| {
            let sni: String = row.get(0)?;
            let cert_pem: String = row.get(1)?;
            let key_pem: String = row.get(2)?;
            let acme_email: Option<String> = row.get(3)?;
            Ok(DbCert {
                sni,
                cert_pem: cert_pem.into_bytes(),
                key_pem: key_pem.into_bytes(),
                acme_email,
            })
        })?;

        let mut certs = Vec::new();
        for cert in cert_iter {
            certs.push(cert?);
        }
        Ok(certs)
    }

    pub fn save_cert(
        &self,
        sni: &str,
        cert_pem: &[u8],
        key_pem: &[u8],
        acme_email: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let cert_str = std::str::from_utf8(cert_pem)?;
        let key_str = std::str::from_utf8(key_pem)?;
        conn.execute(
            "INSERT OR REPLACE INTO certificates (sni, cert_pem, key_pem, acme_email) VALUES (?1, ?2, ?3, ?4)",
            params![sni, cert_str, key_str, acme_email],
        )?;
        Ok(())
    }

    pub fn load_acme_certs(&self) -> anyhow::Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT sni, acme_email FROM certificates WHERE acme_email IS NOT NULL")?;
        let rows = stmt.query_map([], |row| {
            let sni: String = row.get(0)?;
            let email: String = row.get(1)?;
            Ok((sni, email))
        })?;
        let mut certs = Vec::new();
        for r in rows {
            certs.push(r?);
        }
        Ok(certs)
    }
}
