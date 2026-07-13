<p align="left">
  <img src="assets/logo.png" alt="Seraph Logo" width="100" />
</p>

# Seraph

**A self-hosted reverse proxy with QUIC tunneling, automated TLS, and a real-time dashboard — built with Rust & Pingora.**

![Preview](assets/preview.png)

---

## Features

- 🔀 **Reverse Proxy** – HTTP & HTTPS routing with virtual host support
- 🔒 **Automated TLS** – ACME/Let's Encrypt certificate provisioning and renewal
- 🚇 **QUIC Tunneling** – Expose local services through encrypted QUIC tunnels (mTLS)
- 📊 **Real-time Dashboard** – Svelte-based admin UI with live traffic stats
- 🔑 **Agent Enrollment** – Agents self-enroll via one-time keys, no manual cert handling

## Components

| Component | Description |
|-----------|-------------|
| `seraphd` | Core proxy daemon (Pingora-based, embeds the dashboard) |
| `seraph-agent` | Lightweight tunnel agent, runs on the client side |
| `dashboard` | Svelte frontend, embedded into `seraphd` at compile time |

## Getting Started

### Run locally

```bash
cargo run -p seraphd
```

The admin dashboard is available at `http://127.0.0.1:9090`.

### Docker

```bash
# Daemon
docker run -d \
  -p 8080:8080 -p 8443:8443 -p 9090:9090 -p 7700:7700/udp \
  -v seraph-data:/var/lib/seraph \
  ghcr.io/konradx64/seraph-seraphd:latest

# Agent (first run — enrollment)
docker run -d \
  -v agent-data:/var/lib/seraph-agent \
  ghcr.io/konradx64/seraph-seraph-agent:latest \
  --server http://your-server:9090 \
  --key YOUR_ENROLLMENT_KEY
```

> After the first enrollment, the agent stores its identity in the volume and reconnects automatically on restart.

## Configuration

`seraphd` looks for a `config.toml` in the working directory. If not found, a default one is generated automatically.

```toml
http_addr    = "0.0.0.0:8080"
https_addr   = "0.0.0.0:8443"
admin_addr   = "127.0.0.1:9090"
tunnel_addr  = "0.0.0.0:7700"
database_path = "data/seraph.db"
```

## Architecture

```
  Internet
     │
     ▼
  seraphd  ──── HTTPS/HTTP ────▶  upstream services
     │
     │  QUIC (mTLS, UDP 7700)
     │
  seraph-agent  ──── TCP ────▶  local service
```

## License

MIT
