//! Integration test for the decision-runtime v0: agents pick + execute
//! advertisements end-to-end through `Sim::tick`.

mod common;

use gecko_sim_core::agent::{MemoryKind, Needs};
use gecko_sim_core::ids::ObjectTypeId;
use gecko_sim_core::{Sim, Vec2};

#[test]
fn agent_eats_from_fridge_when_hungry() {
    let mut sim = Sim::new(0, common::seed_content_bundle());
    let agent_id = sim.spawn_test_agent_with_needs(
        "Hungry",
        Needs {
            hunger: 0.3,
            ..Needs::full()
        },
    );
    let leaf = sim.world_graph().default_spawn_leaf;
    sim.spawn_test_object(ObjectTypeId::new(1), leaf, Vec2::ZERO);

    // The fridge ad takes 10 ticks. The first tick decides; ticks 2-11
    // execute; tick 11 completes (since started_tick=1 and duration=10
    // means expected_end_tick=11). Run an extra few ticks for slack.
    for _ in 0..15 {
        sim.tick();
    }

    let snap = sim.snapshot();
    let agent = &snap.agents[0];
    assert!(
        agent.needs.hunger > 0.6,
        "hunger restored from 0.3 to {}",
        agent.needs.hunger
    );
    assert_eq!(agent.pos, Vec2::new(0.0, -1.0));
    assert_eq!(agent.facing, Vec2::new(0.0, 1.0));
    let memories = sim.agent_memory(agent_id).expect("agent has memory");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].kind, MemoryKind::Routine);
    assert_eq!(memories[0].participants, vec![agent_id]);
}

#[test]
fn two_agents_do_not_choose_same_single_occupancy_chair_spot() {
    let mut sim = Sim::new(0, common::seed_content_bundle());
    sim.spawn_test_agent_with_needs(
        "Comfy 1",
        Needs {
            comfort: 0.3,
            ..Needs::full()
        },
    );
    sim.spawn_test_agent_with_needs(
        "Comfy 2",
        Needs {
            comfort: 0.3,
            ..Needs::full()
        },
    );
    let leaf = sim.world_graph().default_spawn_leaf;
    sim.spawn_test_object(ObjectTypeId::new(2), leaf, Vec2::ZERO);

    sim.tick();

    let snap = sim.snapshot();
    let sit_actions = snap
        .agents
        .iter()
        .filter(|agent| {
            agent
                .current_action
                .as_ref()
                .is_some_and(|action| action.display_name == "Sit")
        })
        .count();
    assert_eq!(sit_actions, 1);
}
