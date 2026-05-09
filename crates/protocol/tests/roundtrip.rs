//! Serde roundtrip every wire type. Locks the JSON-on-the-wire format
//! against accidental drift.

use gecko_sim_core::agent::{Mood, Needs, Personality};
use gecko_sim_core::ids::{AgentId, LeafAreaId};
use gecko_sim_core::{AgentSnapshot, Snapshot, Vec2, WorldGraph, WorldLayout};
use gecko_sim_protocol::{
    ClientMessage, PlayerInput, ServerMessage, WireFormat, PROTOCOL_VERSION,
};

fn sample_snapshot() -> Snapshot {
    sample_snapshot_with_agents(2)
}

#[allow(
    clippy::cast_precision_loss,
    reason = "fixture indices stay below the f32 precision threshold"
)]
fn sample_snapshot_with_agents(count: usize) -> Snapshot {
    let names = ["Alice", "Bob", "Carol", "Dave", "Eve"];
    let agents = (0..count)
        .map(|i| AgentSnapshot {
            id: AgentId::new(i as u64),
            name: names.get(i).copied().unwrap_or("Agent").to_string(),
            needs: Needs::full(),
            mood: Mood::neutral(),
            personality: Personality::default(),
            leaf: LeafAreaId::new(1),
            pos: Vec2::new(i as f32, 0.0),
            facing: Vec2::new(1.0, 0.0),
            action_phase: None,
            current_action: None,
        })
        .collect();
    Snapshot {
        tick: 7,
        agents,
        objects: vec![],
    }
}

fn sample_world_layout() -> WorldLayout {
    WorldLayout::from(&WorldGraph::seed_v0())
}

fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let encoded = serde_json::to_string(value).expect("serialize");
    let decoded: T = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(value, &decoded, "roundtrip changed value: {encoded}");
}

#[test]
fn server_hello_roundtrips() {
    roundtrip(&ServerMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        format: WireFormat::Json,
    });
}

#[test]
fn server_init_roundtrips() {
    roundtrip(&ServerMessage::Init {
        current_tick: 0,
        world: sample_world_layout(),
        snapshot: sample_snapshot(),
    });
}

#[test]
fn server_snapshot_roundtrips() {
    roundtrip(&ServerMessage::Snapshot {
        snapshot: sample_snapshot(),
    });
}

#[test]
fn client_hello_roundtrips_with_known_tick() {
    roundtrip(&ClientMessage::ClientHello {
        last_known_tick: Some(42),
    });
}

#[test]
fn client_hello_roundtrips_without_known_tick() {
    roundtrip(&ClientMessage::ClientHello {
        last_known_tick: None,
    });
}

#[test]
fn client_player_input_set_speed_roundtrips() {
    roundtrip(&ClientMessage::PlayerInput(PlayerInput::SetSpeed {
        multiplier: 8.0,
    }));
}

#[test]
fn client_player_input_toggle_pause_roundtrips() {
    roundtrip(&ClientMessage::PlayerInput(PlayerInput::TogglePause));
}

#[test]
fn server_messages_use_tagged_enum_layout() {
    let json = serde_json::to_string(&ServerMessage::Hello {
        protocol_version: 1,
        format: WireFormat::Json,
    })
    .unwrap();
    assert!(json.contains("\"type\":\"hello\""), "got {json}");
    assert!(json.contains("\"format\":\"json\""), "got {json}");
}

#[test]
fn client_messages_use_tagged_enum_layout() {
    let json = serde_json::to_string(&ClientMessage::PlayerInput(PlayerInput::TogglePause))
        .unwrap();
    assert!(json.contains("\"type\":\"player_input\""), "got {json}");
    assert!(json.contains("\"kind\":\"toggle_pause\""), "got {json}");
}

#[test]
fn protocol_version_is_one() {
    assert_eq!(PROTOCOL_VERSION, 1);
}

#[test]
fn empty_snapshot_roundtrips_with_explicit_empty_agents_array() {
    let snapshot = sample_snapshot_with_agents(0);
    let encoded = serde_json::to_string(&snapshot).expect("serialize");
    // Empty Vec must serialise as `"agents":[]`, not be omitted.
    assert!(
        encoded.contains("\"agents\":[]"),
        "expected explicit empty array, got {encoded}"
    );
    roundtrip(&snapshot);
}

#[test]
fn single_agent_snapshot_roundtrips() {
    roundtrip(&sample_snapshot_with_agents(1));
}

#[test]
fn three_agent_snapshot_roundtrips() {
    roundtrip(&sample_snapshot_with_agents(3));
}

#[test]
fn wire_format_json_variant_roundtrips() {
    roundtrip(&WireFormat::Json);
}

#[test]
fn agent_snapshot_with_current_action_roundtrips() {
    use gecko_sim_core::CurrentActionView;
    let snap = Snapshot {
        tick: 10,
        agents: vec![AgentSnapshot {
            id: AgentId::new(0),
            name: "Alice".to_string(),
            needs: Needs::full(),
            mood: Mood::neutral(),
            personality: Personality::default(),
            leaf: LeafAreaId::new(1),
            pos: Vec2::ZERO,
            facing: Vec2::new(1.0, 0.0),
            action_phase: Some(gecko_sim_core::decision::Phase::Performing),
            current_action: Some(CurrentActionView {
                display_name: "Eat snack".to_string(),
                fraction_complete: 0.5,
                phase: gecko_sim_core::decision::Phase::Performing,
                target_object_id: Some(gecko_sim_core::ids::ObjectId::new(2)),
                target_position: Some(Vec2::new(96.0, 87.0)),
                target_label: Some("Fridge".to_string()),
            }),
        }],
        objects: vec![],
    };
    roundtrip(&snap);
}

#[test]
fn agent_snapshot_with_personality_roundtrips() {
    let snap = Snapshot {
        tick: 11,
        agents: vec![AgentSnapshot {
            id: AgentId::new(0),
            name: "Alice".to_string(),
            needs: Needs::full(),
            mood: Mood::neutral(),
            personality: Personality {
                openness: 0.4,
                conscientiousness: -0.3,
                extraversion: 0.7,
                agreeableness: -0.5,
                neuroticism: 0.1,
            },
            leaf: LeafAreaId::new(1),
            pos: Vec2::ZERO,
            facing: Vec2::new(1.0, 0.0),
            action_phase: None,
            current_action: None,
        }],
        objects: vec![],
    };
    roundtrip(&snap);
}

#[test]
fn agent_snapshot_with_spatial_fields_roundtrips() {
    let snap = Snapshot {
        tick: 12,
        agents: vec![AgentSnapshot {
            id: AgentId::new(0),
            name: "Alice".to_string(),
            needs: Needs::full(),
            mood: Mood::neutral(),
            personality: Personality::default(),
            leaf: LeafAreaId::new(3),
            pos: Vec2::new(12.5, -3.25),
            facing: Vec2::new(0.0, 1.0),
            action_phase: Some(gecko_sim_core::decision::Phase::Walking),
            current_action: None,
        }],
        objects: vec![gecko_sim_core::ObjectSnapshot {
            id: gecko_sim_core::ids::ObjectId::new(0),
            type_id: gecko_sim_core::ids::ObjectTypeId::new(1),
            leaf: LeafAreaId::new(3),
            pos: Vec2::new(15.0, 0.0),
        }],
    };
    roundtrip(&snap);
}

#[test]
fn init_with_world_layout_roundtrips() {
    roundtrip(&ServerMessage::Init {
        current_tick: 3,
        world: sample_world_layout(),
        snapshot: sample_snapshot(),
    });
}
