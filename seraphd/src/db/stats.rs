use super::Database;
use crate::stats::{PersistedRouteStats, PersistedStats, PersistedTunnelStats};
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;

impl Database {
    pub fn load_stats(&self) -> anyhow::Result<PersistedStats> {
        let conn = self.conn.lock().unwrap();
        let mut stats = conn
            .query_row(
                "SELECT total_requests, status_2xx, status_3xx, status_4xx, status_5xx, dropped_events
                 FROM stats_global WHERE id = 1",
                [],
                |row| {
                    Ok(PersistedStats {
                        total_requests: row.get(0)?,
                        status_2xx: row.get(1)?,
                        status_3xx: row.get(2)?,
                        status_4xx: row.get(3)?,
                        status_5xx: row.get(4)?,
                        dropped_events: row.get(5)?,
                        routes: HashMap::new(),
                        tunnels: HashMap::new(),
                    })
                },
            )
            .optional()?
            .unwrap_or_default();

        let mut route_stmt =
            conn.prepare("SELECT hostname, total_requests, total_latency_ms FROM stats_routes")?;
        let routes = route_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PersistedRouteStats {
                    total_requests: row.get(1)?,
                    total_latency_ms: row.get(2)?,
                },
            ))
        })?;
        for route in routes {
            let (hostname, route_stats) = route?;
            stats.routes.insert(hostname, route_stats);
        }

        let mut tunnel_stmt =
            conn.prepare("SELECT tunnel_id, bytes_sent, bytes_received FROM stats_tunnels")?;
        let tunnels = tunnel_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PersistedTunnelStats {
                    bytes_sent: row.get(1)?,
                    bytes_received: row.get(2)?,
                },
            ))
        })?;
        for tunnel in tunnels {
            let (id, tunnel_stats) = tunnel?;
            stats.tunnels.insert(id, tunnel_stats);
        }

        Ok(stats)
    }

    pub fn save_stats(&self, stats: &PersistedStats) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO stats_global
                (id, total_requests, status_2xx, status_3xx, status_4xx, status_5xx, dropped_events)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                total_requests = excluded.total_requests,
                status_2xx = excluded.status_2xx,
                status_3xx = excluded.status_3xx,
                status_4xx = excluded.status_4xx,
                status_5xx = excluded.status_5xx,
                dropped_events = excluded.dropped_events",
            params![
                stats.total_requests,
                stats.status_2xx,
                stats.status_3xx,
                stats.status_4xx,
                stats.status_5xx,
                stats.dropped_events,
            ],
        )?;

        for (hostname, route) in &stats.routes {
            tx.execute(
                "INSERT INTO stats_routes (hostname, total_requests, total_latency_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(hostname) DO UPDATE SET
                    total_requests = excluded.total_requests,
                    total_latency_ms = excluded.total_latency_ms",
                params![hostname, route.total_requests, route.total_latency_ms],
            )?;
        }
        for (id, tunnel) in &stats.tunnels {
            tx.execute(
                "INSERT INTO stats_tunnels (tunnel_id, bytes_sent, bytes_received)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(tunnel_id) DO UPDATE SET
                    bytes_sent = excluded.bytes_sent,
                    bytes_received = excluded.bytes_received",
                params![id, tunnel.bytes_sent, tunnel.bytes_received],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_round_trip_through_database() {
        let db = Database::open(":memory:").unwrap();
        let mut stats = PersistedStats {
            total_requests: 42,
            status_2xx: 30,
            status_3xx: 2,
            status_4xx: 8,
            status_5xx: 2,
            dropped_events: 1,
            ..PersistedStats::default()
        };
        stats.routes.insert(
            "example.com".to_string(),
            PersistedRouteStats {
                total_requests: 20,
                total_latency_ms: 1_500,
            },
        );
        stats.tunnels.insert(
            "office".to_string(),
            PersistedTunnelStats {
                bytes_sent: 10_000,
                bytes_received: 20_000,
            },
        );

        db.save_stats(&stats).unwrap();
        let restored = db.load_stats().unwrap();

        assert_eq!(restored.total_requests, 42);
        assert_eq!(restored.status_2xx, 30);
        assert_eq!(restored.dropped_events, 1);
        assert_eq!(restored.routes["example.com"].total_requests, 20);
        assert_eq!(restored.routes["example.com"].total_latency_ms, 1_500);
        assert_eq!(restored.tunnels["office"].bytes_sent, 10_000);
        assert_eq!(restored.tunnels["office"].bytes_received, 20_000);
    }
}
