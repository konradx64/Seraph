use rusqlite::params;
use crate::route::{Route, TlsMode};
use super::Database;

impl Database {
    pub fn load_routes(&self) -> anyhow::Result<Vec<Route>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT hostname, path_prefix, upstream, tunnel, tls, upstream_tls, hsts, cors_origins, forward_ip FROM routes")?;
        let route_iter = stmt.query_map([], |row| {
            let hostname: String = row.get(0)?;
            let path_prefix: Option<String> = row.get(1)?;
            let upstream: String = row.get(2)?;
            let tunnel: Option<String> = row.get(3)?;
            let tls_json: String = row.get(4)?;
            let tls: TlsMode = serde_json::from_str(&tls_json).unwrap_or_default();
            let upstream_tls: bool = row.get::<_, i32>(5)? != 0;
            let hsts: bool = row.get::<_, i32>(6)? != 0;
            let cors_origins: Option<String> = row.get(7)?;
            let forward_ip: bool = row.get::<_, i32>(8)? != 0;
            Ok(Route {
                hostname,
                path_prefix,
                upstream,
                tunnel,
                tls,
                upstream_tls,
                hsts,
                cors_origins,
                forward_ip,
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

        // Enforce absolute uniqueness on hostname and path_prefix
        if route.path_prefix.is_none() {
            let _ = conn.execute("DELETE FROM routes WHERE hostname = ?1 AND path_prefix IS NULL", params![route.hostname]);
        } else {
            let _ = conn.execute("DELETE FROM routes WHERE hostname = ?1 AND path_prefix = ?2", params![route.hostname, route.path_prefix]);
        }

        let tls_json = serde_json::to_string(&route.tls)?;
        conn.execute(
            "INSERT INTO routes (hostname, path_prefix, upstream, tunnel, tls, upstream_tls, hsts, cors_origins, forward_ip)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                route.hostname,
                route.path_prefix,
                route.upstream,
                route.tunnel,
                tls_json,
                if route.upstream_tls { 1 } else { 0 },
                if route.hsts { 1 } else { 0 },
                route.cors_origins,
                if route.forward_ip { 1 } else { 0 }
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
