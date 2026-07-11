mod app;
mod cert_registry;
mod config;
mod control;
mod event;
mod route;
mod route_registry;
mod state;
mod web_proxy;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    app::run()
}
