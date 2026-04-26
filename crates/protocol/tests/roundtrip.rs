//! Serde roundtrip every wire type. Locks the JSON-on-the-wire format
//! against accidental drift.

use gecko_sim_core::agent::Needs;
use gecko_sim_core::ids::AgentId;
use gecko_sim_core::{AgentSnapshot, Snapshot};
use gecko_sim_protocol::{
    ClientMessage, PlayerInput, ServerMessage, WireFormat, PROTOCOL_VERSION,
};

fn sample_snapshot() -> Snapshot {
    Snapshot {
        tick: 7,
        agents: vec![
            AgentSnapshot {
                id: AgentId::new(0),
                name: "Alice".to_string(),
                needs: Needs::full(),
            },
            AgentSnapshot {
                id: AgentId::new(1),
                name: "Bob".to_string(),
                needs: Needs::full(),
            },
        ],
    }
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
