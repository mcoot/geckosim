//! Integration test: same seed + same calls → byte-equal Snapshot.
//!
//! Bakes in the determinism discipline from ADR 0008 so any future
//! source of nondeterminism (`HashMap` iteration order, `Instant::now`,
//! unsorted query results, …) is caught immediately.

use gecko_sim_core::{ContentBundle, Sim, Snapshot};

fn run(seed: u64, ticks: u64) -> Snapshot {
    let mut sim = Sim::new(seed, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");
    for _ in 0..ticks {
        sim.tick();
    }
    sim.snapshot()
}

#[test]
fn same_seed_same_snapshot_after_100_ticks() {
    assert_eq!(run(42, 100), run(42, 100));
}

#[test]
fn same_seed_same_snapshot_at_tick_zero() {
    assert_eq!(run(42, 0), run(42, 0));
}
