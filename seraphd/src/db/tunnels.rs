use super::Database;
use rusqlite::{OptionalExtension, params};

pub struct TunnelInfo {
    pub id: String,
    pub client_cert: Option<String>,
    pub created_at: String,
    pub enrollment_expires_at: Option<String>,
    pub enrollment_used_at: Option<String>,
}

impl Database {
    pub fn load_tunnels(&self) -> anyhow::Result<Vec<TunnelInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, client_cert, created_at, enrollment_expires_at, enrollment_used_at
             FROM tunnels",
        )?;
        let tunnel_iter = stmt.query_map([], |row| {
            Ok(TunnelInfo {
                id: row.get(0)?,
                client_cert: row.get(1)?,
                created_at: row.get(2)?,
                enrollment_expires_at: row.get(3)?,
                enrollment_used_at: row.get(4)?,
            })
        })?;

        let mut tunnels = Vec::new();
        for tunnel in tunnel_iter {
            tunnels.push(tunnel?);
        }
        Ok(tunnels)
    }

    pub fn save_tunnel(
        &self,
        id: &str,
        token_hash: &str,
        created_at: &str,
        enrollment_expires_at: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO tunnels (
                id, token_hash, client_cert, client_cert_fingerprint, enrollment_expires_at,
                enrollment_used_at, created_at
             )
             VALUES (?1, ?2, NULL, NULL, ?3, NULL, ?4)",
            params![id, token_hash, enrollment_expires_at, created_at],
        )?;
        Ok(())
    }

    pub fn save_tunnel_cert(
        &self,
        id: &str,
        client_cert: &str,
        client_cert_fingerprint: &str,
        enrollment_used_at: &str,
    ) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE tunnels
             SET client_cert = ?1,
                 client_cert_fingerprint = ?2,
                 enrollment_used_at = ?3
             WHERE id = ?4
               AND client_cert IS NULL
               AND enrollment_used_at IS NULL",
            params![client_cert, client_cert_fingerprint, enrollment_used_at, id],
        )?;
        Ok(rows == 1)
    }

    pub fn delete_tunnel(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM tunnels WHERE id = ?1",
            params![id],
        )?;
        Ok(rows_affected > 0)
    }

    pub fn get_tunnel_by_token_hash(&self, token_hash: &str) -> anyhow::Result<Option<TunnelInfo>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, client_cert, created_at, enrollment_expires_at, enrollment_used_at
             FROM tunnels
             WHERE token_hash = ?1",
            params![token_hash],
            |row| {
                Ok(TunnelInfo {
                    id: row.get(0)?,
                    client_cert: row.get(1)?,
                    created_at: row.get(2)?,
                    enrollment_expires_at: row.get(3)?,
                    enrollment_used_at: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }
}
