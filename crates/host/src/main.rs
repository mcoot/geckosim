use gecko_sim_core::{ContentBundle, Sim};
use tracing_subscriber::EnvFilter;

const DEMO_SEED: u64 = 0xDEAD_BEEF;
const DEMO_TICKS: u64 = 100;

#[expect(
    clippy::unnecessary_wraps,
    reason = "main returns Result so future ? chains land cleanly"
)]
#[expect(
    clippy::default_constructed_unit_structs,
    reason = "ContentBundle is a unit struct placeholder in this pass"
)]
fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));

    let mut sim = Sim::new(DEMO_SEED, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");

    let initial = sim.snapshot();
    tracing::info!(?initial, "initial snapshot");

    for _ in 0..DEMO_TICKS {
        sim.tick();
    }

    let after = sim.snapshot();
    tracing::info!(?after, ticks = DEMO_TICKS, "snapshot after demo run");

    Ok(())
}
