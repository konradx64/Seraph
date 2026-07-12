use rusqlite::Connection;
use std::sync::Mutex;

mod routes;
mod certs;

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
        conn.execute(
            "CREATE TABLE IF NOT EXISTS certificates (
                sni TEXT PRIMARY KEY,
                cert_pem TEXT NOT NULL,
                key_pem TEXT NOT NULL
            )",
            [],
        )?;
        Ok(())
    }
}
