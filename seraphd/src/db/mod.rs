use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

mod routes;
mod stats;
mod tunnels;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS routes (
                hostname TEXT NOT NULL,
                path_prefix TEXT,
                upstream TEXT NOT NULL,
                tunnel TEXT,
                tls TEXT NOT NULL,
                PRIMARY KEY (hostname, path_prefix)
            )",
            [],
        )?;
        // Migration: Add upstream_tls if it doesn't exist
        let _ = conn.execute(
            "ALTER TABLE routes ADD COLUMN upstream_tls INTEGER DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE routes ADD COLUMN hsts INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE routes ADD COLUMN cors_origins TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE routes ADD COLUMN forward_ip INTEGER DEFAULT 1",
            [],
        );

        conn.execute(
            "CREATE TABLE IF NOT EXISTS tunnels (
                id TEXT PRIMARY KEY,
                token_hash TEXT NOT NULL,
                client_cert TEXT,
                client_cert_fingerprint TEXT,
                enrollment_expires_at TEXT NOT NULL,
                enrollment_used_at TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS stats_global (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                total_requests INTEGER NOT NULL,
                status_2xx INTEGER NOT NULL,
                status_3xx INTEGER NOT NULL,
                status_4xx INTEGER NOT NULL,
                status_5xx INTEGER NOT NULL,
                dropped_events INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS stats_routes (
                hostname TEXT PRIMARY KEY,
                total_requests INTEGER NOT NULL,
                total_latency_ms INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS stats_tunnels (
                tunnel_id TEXT PRIMARY KEY,
                bytes_sent INTEGER NOT NULL,
                bytes_received INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(())
    }
}
