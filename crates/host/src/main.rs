use anyhow::Context;
use gecko_sim_host::{config, demo, sim_driver, ws_server};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));

    let content_path = config::content_dir();
    tracing::info!(path = %content_path.display(), "loading content");
    let content = gecko_sim_content::load_from_dir(&content_path)
        .with_context(|| format!("loading content from {}", content_path.display()))?;
    tracing::info!(
        object_types = content.object_types.len(),
        accessories = content.accessories.len(),
        "content loaded"
    );

    let sim = demo::build_demo_sim(content);

    let initial = sim.snapshot();
    tracing::info!(
        agents = initial.agents.len(),
        objects = initial.objects.len(),
        "sim primed"
    );

    let world_layout = gecko_sim_core::WorldLayout::from(sim.world_graph());

    let addr = config::listen_addr()?;
    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!(%local_addr, "ws transport listening");

    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel(initial);

    let driver = tokio::spawn(sim_driver::run(sim, input_rx, snapshot_tx));
    let server = tokio::spawn(ws_server::run(
        listener,
        input_tx,
        snapshot_rx,
        world_layout,
    ));

    tokio::signal::ctrl_c().await?;
    tracing::info!("ctrl-c received, shutting down");
    driver.abort();
    server.abort();
    Ok(())
}
