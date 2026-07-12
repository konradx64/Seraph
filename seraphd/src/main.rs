mod app;
mod config;
mod control;
mod db;
mod event;
mod registry;
mod route;
mod state;
mod web_proxy;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    app::run()
}
