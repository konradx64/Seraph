use rusqlite::params;
use super::Database;

impl Database {
    pub fn load_certs(&self) -> anyhow::Result<Vec<(String, Vec<u8>, Vec<u8>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT sni, cert_pem, key_pem FROM certificates")?;
        let cert_iter = stmt.query_map([], |row| {
            let sni: String = row.get(0)?;
            let cert_pem: String = row.get(1)?;
            let key_pem: String = row.get(2)?;
            Ok((sni, cert_pem.into_bytes(), key_pem.into_bytes()))
        })?;

        let mut certs = Vec::new();
        for cert in cert_iter {
            certs.push(cert?);
        }
        Ok(certs)
    }

    pub fn save_cert(&self, sni: &str, cert_pem: &[u8], key_pem: &[u8]) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let cert_str = std::str::from_utf8(cert_pem)?;
        let key_str = std::str::from_utf8(key_pem)?;
        conn.execute(
            "INSERT OR REPLACE INTO certificates (sni, cert_pem, key_pem) VALUES (?1, ?2, ?3)",
            params![sni, cert_str, key_str],
        )?;
        Ok(())
    }
}
