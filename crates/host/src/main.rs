use tracing_subscriber::EnvFilter;

// allow: idiomatic extendable-main pattern; body will use `?` as host grows
#[allow(clippy::unnecessary_wraps)]
fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
