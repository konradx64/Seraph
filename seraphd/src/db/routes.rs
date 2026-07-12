use rusqlite::params;
use crate::route::{Route, TlsMode};
use super::Database;

impl Database {
    pub fn load_routes(&self) -> anyhow::Result<Vec<Route>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT hostname, path_prefix, upstream, tunnel, tls FROM routes")?;
        let route_iter = stmt.query_map([], |row| {
            let hostname: String = row.get(0)?;
            let path_prefix: Option<String> = row.get(1)?;
            let upstream: String = row.get(2)?;
            let tunnel: Option<String> = row.get(3)?;
            let tls_json: String = row.get(4)?;
            let tls: TlsMode = serde_json::from_str(&tls_json).unwrap_or(TlsMode::Auto);
            Ok(Route {
                hostname,
                path_prefix,
                upstream,
                tunnel,
                tls,
            })
        })?;

        let mut routes = Vec::new();
        for route in route_iter {
            routes.push(route?);
        }
        Ok(routes)
    }

    pub fn save_route(&self, route: &Route) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let tls_json = serde_json::to_string(&route.tls)?;
        conn.execute(
            "INSERT OR REPLACE INTO routes (hostname, path_prefix, upstream, tunnel, tls)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                route.hostname,
                route.path_prefix,
                route.upstream,
                route.tunnel,
                tls_json
            ],
        )?;
        Ok(())
    }

    pub fn delete_route(&self, hostname: &str, path_prefix: Option<&str>) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = match path_prefix {
            Some(prefix) => conn.execute(
                "DELETE FROM routes WHERE hostname = ?1 AND path_prefix = ?2",
                params![hostname, prefix],
            )?,
            None => conn.execute(
                "DELETE FROM routes WHERE hostname = ?1 AND path_prefix IS NULL",
                params![hostname],
            )?,
        };
        Ok(rows_affected > 0)
    }
}
