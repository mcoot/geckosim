// allow: idiomatic extendable-main pattern; body will use `?` as host grows
#[allow(clippy::unnecessary_wraps)]
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
