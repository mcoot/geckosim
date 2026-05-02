//! Integration test: same seed + same calls → byte-equal Snapshot.
//!
//! Bakes in the determinism discipline from ADR 0008 so any future
//! source of nondeterminism (`HashMap` iteration order, `Instant::now`,
//! unsorted query results, …) is caught immediately.

mod common;

use gecko_sim_core::agent::{MemoryEntry, Needs};
use gecko_sim_core::{ContentBundle, Sim, Snapshot, Vec2};

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

fn run_hungry_agent_memories(seed: u64, ticks: u64) -> Vec<MemoryEntry> {
    let mut sim = Sim::new(seed, common::seed_content_bundle());
    let agent = sim.spawn_test_agent_with_needs(
        "Hungry",
        Needs {
            hunger: 0.3,
            ..Needs::full()
        },
    );
    let leaf = sim.world_graph().default_spawn_leaf;
    sim.spawn_one_of_each_object_type(leaf, Vec2::ZERO);
    for _ in 0..ticks {
        sim.tick();
    }
    sim.agent_memory(agent)
        .expect("agent has memory")
        .to_vec()
}

#[test]
fn same_seed_same_snapshot_after_100_ticks() {
    assert_eq!(run(42, 100), run(42, 100));
}

#[test]
fn same_seed_same_snapshot_at_tick_zero() {
    assert_eq!(run(42, 0), run(42, 0));
}

#[test]
fn same_seed_same_action_memories_after_fridge_action() {
    assert_eq!(
        run_hungry_agent_memories(42, 15),
        run_hungry_agent_memories(42, 15)
    );
}
