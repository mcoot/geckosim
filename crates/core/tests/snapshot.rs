//! Integration test: spawning agents and producing a deterministic snapshot.

use std::collections::HashMap;

use gecko_sim_core::agent::{Need, Needs, Personality};
use gecko_sim_core::ids::AgentId;
use gecko_sim_core::ids::{
    AdvertisementId, InteractionSpotId, LeafAreaId, ObjectTypeId,
};
use gecko_sim_core::object::{
    Advertisement, Effect, InteractionSpot, InterruptClass, MeshId, ObjectType, Op, Predicate,
    ScoreTemplate, StateValue,
};
use gecko_sim_core::{ContentBundle, Sim, Vec2};

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
    assert!(snap.objects.is_empty());

    // Sorted by AgentId ascending.
    assert_eq!(snap.agents[0].id, AgentId::new(0));
    assert_eq!(snap.agents[0].name, "Alice");
    assert_eq!(snap.agents[0].needs.hunger, 1.0);

    assert_eq!(snap.agents[1].id, AgentId::new(1));
    assert_eq!(snap.agents[1].name, "Bob");

    assert_eq!(snap.agents[2].id, AgentId::new(2));
    assert_eq!(snap.agents[2].name, "Charlie");

    // Spatial fields: every agent lives in default_spawn_leaf with a
    // deterministic +x offset; no action committed yet at tick 0.
    let spawn_leaf = sim.world_graph().default_spawn_leaf;
    assert_eq!(snap.agents[0].leaf, spawn_leaf);
    assert_eq!(snap.agents[0].pos, Vec2::ZERO);
    assert_eq!(snap.agents[1].pos, Vec2::new(1.0, 0.0));
    assert_eq!(snap.agents[2].pos, Vec2::new(2.0, 0.0));
    assert!(snap.agents[0].action_phase.is_none());

    let alice_memory = sim.agent_memory(alice).expect("Alice has memory component");
    assert!(alice_memory.is_empty());
}

fn fridge_object_type() -> ObjectType {
    ObjectType {
        id: ObjectTypeId::new(1),
        display_name: "Fridge".to_string(),
        mesh_id: MeshId(1),
        default_state: [("stocked".to_string(), StateValue::Bool(true))]
            .into_iter()
            .collect(),
        interaction_spots: vec![InteractionSpot {
            id: InteractionSpotId::new(1),
            offset: Vec2::new(0.0, -1.0),
            facing: Vec2::new(0.0, 1.0),
            label: Some("door".to_string()),
        }],
        advertisements: vec![Advertisement {
            id: AdvertisementId::new(1),
            display_name: "Eat snack".to_string(),
            preconditions: vec![
                Predicate::ObjectState(
                    "stocked".to_string(),
                    Op::Eq,
                    StateValue::Bool(true),
                ),
                Predicate::AgentNeed(Need::Hunger, Op::Lt, 0.6),
            ],
            effects: vec![Effect::AgentNeedDelta(Need::Hunger, 0.4)],
            duration_ticks: 10,
            interrupt_class: InterruptClass::NeedsThresholdOnly,
            score_template: ScoreTemplate {
                need_weights: vec![(Need::Hunger, 1.0)],
                personality_weights: Personality::default(),
                situational_modifiers: vec![],
            },
        }],
    }
}

#[test]
fn snapshot_current_action_exposes_object_intent() {
    let fridge = fridge_object_type();
    let mut object_types = HashMap::new();
    object_types.insert(fridge.id, fridge);
    let mut sim = Sim::new(
        0,
        ContentBundle {
            object_types,
            accessories: HashMap::new(),
        },
    );
    let spawn_leaf: LeafAreaId = sim.world_graph().default_spawn_leaf;
    let object_id = sim.spawn_test_object(
        ObjectTypeId::new(1),
        spawn_leaf,
        Vec2::new(96.0, 88.0),
    );
    sim.spawn_test_agent_with_needs(
        "Alice",
        Needs {
            hunger: 0.2,
            ..Needs::full()
        },
    );

    sim.tick();
    let snap = sim.snapshot();
    let action = snap.agents[0]
        .current_action
        .as_ref()
        .expect("Alice chose an action");

    assert_eq!(action.display_name, "Eat snack");
    assert_eq!(action.phase, gecko_sim_core::decision::Phase::Walking);
    assert_eq!(action.target_object_id, Some(object_id));
    assert_eq!(action.target_position, Some(Vec2::new(96.0, 87.0)));
    assert_eq!(action.target_label.as_deref(), Some("Fridge"));
    assert_eq!(action.fraction_complete, 0.0);
}
