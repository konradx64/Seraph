use clap::Parser;
use seraphd::config::AppConfig;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    seraphd::run(AppConfig::parse())
}
