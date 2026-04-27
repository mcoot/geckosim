use gecko_sim_core::{ContentBundle, Sim};
use gecko_sim_host::{config, sim_driver, ws_server};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEMO_SEED: u64 = 0xDEAD_BEEF;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));

    let mut sim = Sim::new(DEMO_SEED, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");

    let initial = sim.snapshot();
    tracing::info!(agents = initial.agents.len(), "sim primed");

    let addr = config::listen_addr()?;
    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!(%local_addr, "ws transport listening");

    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel(initial);

    let driver = tokio::spawn(sim_driver::run(sim, input_rx, snapshot_tx));
    let server = tokio::spawn(ws_server::run(listener, input_tx, snapshot_rx));

    tokio::signal::ctrl_c().await?;
    tracing::info!("ctrl-c received, shutting down");
    driver.abort();
    server.abort();
    Ok(())
}
