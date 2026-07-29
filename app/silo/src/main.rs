use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let vault_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./scratch-vault"));
    silo_ui::run(vault_path)
}
