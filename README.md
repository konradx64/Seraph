# Seraph

<p align="center">
  <img src="dashboard/public/favicon.png" alt="Seraph Logo" width="120" />
</p>

<p align="center">
  A self-hosted reverse proxy with QUIC tunneling, TLS automation, and a real-time dashboard.
</p>


## Components

- **seraphd**: The core proxy server daemon.
- **dashboard**: A Svelte-based frontend management panel.
- **seraph-agent**: A skeleton client agent.

## Running the Daemon

```bash
cargo run -p seraphd
```
