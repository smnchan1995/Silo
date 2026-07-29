fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    silo_ui::run()
}
