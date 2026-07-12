use rusqlite::Connection;
use std::sync::Mutex;

mod routes;
mod certs;
mod tunnels;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let db = Self { conn: Mutex::new(conn) };
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
        let _ = conn.execute("ALTER TABLE routes ADD COLUMN upstream_tls INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE routes ADD COLUMN hsts INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE routes ADD COLUMN cors_origins TEXT", []);
        let _ = conn.execute("ALTER TABLE routes ADD COLUMN forward_ip INTEGER DEFAULT 1", []);

        conn.execute(
            "CREATE TABLE IF NOT EXISTS certificates (
                sni TEXT PRIMARY KEY,
                cert_pem TEXT NOT NULL,
                key_pem TEXT NOT NULL
            )",
            [],
        )?;
        // Migration: Add acme_email if it doesn't exist
        let _ = conn.execute("ALTER TABLE certificates ADD COLUMN acme_email TEXT", []);

        conn.execute(
            "CREATE TABLE IF NOT EXISTS tunnels (
                id TEXT PRIMARY KEY,
                token TEXT NOT NULL,
                client_cert TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }
}
