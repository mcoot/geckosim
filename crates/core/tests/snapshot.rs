//! Integration test: spawning agents and producing a deterministic snapshot.

use gecko_sim_core::ids::AgentId;
use gecko_sim_core::{ContentBundle, Sim};

#[test]
#[expect(
    clippy::float_cmp,
    reason = "literal 1.0 set in Needs::full() vs literal 1.0 in assertion is bit-exact"
)]
fn snapshot_contains_spawned_agents_sorted_by_id() {
    let mut sim = Sim::new(0, ContentBundle::default());
    let alice = sim.spawn_test_agent("Alice");
    let bob = sim.spawn_test_agent("Bob");
    let charlie = sim.spawn_test_agent("Charlie");

    assert_eq!(alice, AgentId::new(0));
    assert_eq!(bob, AgentId::new(1));
    assert_eq!(charlie, AgentId::new(2));

    let snap = sim.snapshot();
    assert_eq!(snap.tick, 0);
    assert_eq!(snap.agents.len(), 3);

    // Sorted by AgentId ascending.
    assert_eq!(snap.agents[0].id, AgentId::new(0));
    assert_eq!(snap.agents[0].name, "Alice");
    assert_eq!(snap.agents[0].needs.hunger, 1.0);

    assert_eq!(snap.agents[1].id, AgentId::new(1));
    assert_eq!(snap.agents[1].name, "Bob");

    assert_eq!(snap.agents[2].id, AgentId::new(2));
    assert_eq!(snap.agents[2].name, "Charlie");
}
