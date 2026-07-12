fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    seraphd::run()
}
