use rusqlite::params;
use super::Database;

pub struct TunnelInfo {
    pub id: String,
    pub token: String,
    pub client_cert: Option<String>,
    pub created_at: String,
}

impl Database {
    pub fn load_tunnels(&self) -> anyhow::Result<Vec<TunnelInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, token, client_cert, created_at FROM tunnels")?;
        let tunnel_iter = stmt.query_map([], |row| {
            Ok(TunnelInfo {
                id: row.get(0)?,
                token: row.get(1)?,
                client_cert: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;

        let mut tunnels = Vec::new();
        for tunnel in tunnel_iter {
            tunnels.push(tunnel?);
        }
        Ok(tunnels)
    }

    pub fn save_tunnel(&self, id: &str, token: &str, created_at: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO tunnels (id, token, client_cert, created_at)
             VALUES (?1, ?2, NULL, ?3)",
            params![id, token, created_at],
        )?;
        Ok(())
    }

    pub fn save_tunnel_cert(&self, id: &str, client_cert: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tunnels SET client_cert = ?1 WHERE id = ?2",
            params![client_cert, id],
        )?;
        Ok(())
    }

    pub fn delete_tunnel(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM tunnels WHERE id = ?1",
            params![id],
        )?;
        Ok(rows_affected > 0)
    }

    pub fn get_tunnel_by_token(&self, token: &str) -> anyhow::Result<Option<TunnelInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, token, client_cert, created_at FROM tunnels WHERE token = ?1")?;
        let mut rows = stmt.query(params![token])?;
        if let Some(row) = rows.next()? {
            Ok(Some(TunnelInfo {
                id: row.get(0)?,
                token: row.get(1)?,
                client_cert: row.get(2)?,
                created_at: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }
}
