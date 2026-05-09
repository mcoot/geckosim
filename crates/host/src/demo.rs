//! Seeded demo scene used by the host binary.

use gecko_sim_core::agent::Needs;
use gecko_sim_core::ids::{LeafAreaId, ObjectTypeId};
use gecko_sim_core::{ContentBundle, Sim, Vec2};

pub const DEMO_SEED: u64 = 0xDEAD_BEEF;

const FRIDGE_TYPE: ObjectTypeId = ObjectTypeId(1);
const CHAIR_TYPE: ObjectTypeId = ObjectTypeId(2);

/// Build the local demo sim with agents and objects spread around the
/// seed living room. This is deliberately tiny: it keeps the current
/// seed content, but gives the renderer useful spatial variety.
#[must_use]
pub fn build_demo_sim(content: ContentBundle) -> Sim {
    let mut sim = Sim::new(DEMO_SEED, content);
    let living_room = sim.world_graph().default_spawn_leaf;

    spawn_demo_agents(&mut sim, living_room);
    spawn_known_object(&mut sim, FRIDGE_TYPE, living_room, Vec2::new(116.0, 116.0));
    spawn_known_object(&mut sim, CHAIR_TYPE, living_room, Vec2::new(88.0, 112.0));
    spawn_known_object(&mut sim, CHAIR_TYPE, living_room, Vec2::new(112.0, 96.0));

    sim
}

fn spawn_demo_agents(sim: &mut Sim, living_room: LeafAreaId) {
    sim.spawn_test_agent_with_needs_at(
        "Alice",
        Needs {
            hunger: 0.2,
            comfort: 0.75,
            ..Needs::full()
        },
        living_room,
        Vec2::new(84.0, 84.0),
    );
    sim.spawn_test_agent_with_needs_at(
        "Bob",
        Needs {
            comfort: 0.2,
            ..Needs::full()
        },
        living_room,
        Vec2::new(116.0, 84.0),
    );
    sim.spawn_test_agent_with_needs_at(
        "Charlie",
        Needs {
            comfort: 0.15,
            ..Needs::full()
        },
        living_room,
        Vec2::new(100.0, 116.0),
    );
}

fn spawn_known_object(
    sim: &mut Sim,
    type_id: ObjectTypeId,
    living_room: LeafAreaId,
    position: Vec2,
) -> bool {
    if !sim.object_catalog().by_id.contains_key(&type_id) {
        tracing::warn!(?type_id, "demo object type missing from content; skipping");
        return false;
    }

    sim.spawn_test_object(type_id, living_room, position);
    true
}
