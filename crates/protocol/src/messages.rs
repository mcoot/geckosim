//! Wire-type envelope for the host ↔ frontend WebSocket channel (ADR 0013).
//!
//! Two enum families:
//!   - `ServerMessage`: `Hello` (handshake), `Init` (full snapshot on connect),
//!     `Snapshot` (per-sample stream payload).
//!   - `ClientMessage`: `ClientHello` (handshake reply, `last_known_tick`
//!     parsed-but-ignored at v0) and `PlayerInput` (driver-bound controls).
//!
//! `Snapshot` and `AgentSnapshot` are re-used from `gecko_sim_core` directly
//! — no wire/sim decoupling at v0 (per the spec's "wire-type strategy A").
//!
//! `WireFormat` reserves the format-negotiation slot for later
//! `MessagePack` / postcard expansion without a protocol version bump.

use gecko_sim_core::Snapshot;
use serde::{Deserialize, Serialize};

/// Wire protocol version. Bump on incompatible changes; additive changes
/// (new variants, new optional fields) do not require a bump.
pub const PROTOCOL_VERSION: u32 = 1;

/// Wire encoding negotiated in `Hello.format`. Json-only for v0; the
/// extra variant slot anchors future `MessagePack` / postcard support.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum WireFormat {
    Json,
}

/// Server-originated frame. Tagged with `"type"` field (`snake_case`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum ServerMessage {
    /// Handshake greeting, sent immediately after the WS upgrade.
    Hello {
        protocol_version: u32,
        format: WireFormat,
    },
    /// Full state on connect (or reconnect — fresh `Init` always for v0).
    Init {
        current_tick: u64,
        snapshot: Snapshot,
    },
    /// Periodic sample stream payload.
    Snapshot {
        snapshot: Snapshot,
    },
}

/// Client-originated frame. Tagged with `"type"` field (`snake_case`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum ClientMessage {
    /// Handshake reply; `last_known_tick` is parsed but ignored at v0.
    ClientHello {
        last_known_tick: Option<u64>,
    },
    /// Player input. v0 only carries driver-bound variants; sim-bound
    /// variants (`SetPolicy`, `NudgeAgent`, save/load, …) land with
    /// their consumer systems.
    PlayerInput(PlayerInput),
}

/// Driver-bound player inputs. Tagged with `"kind"` so the outer
/// `ClientMessage` "type" tag stays unique.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum PlayerInput {
    /// Set wall-clock tick rate. `multiplier == 0.0` means paused.
    /// Driver clamps to `[0.0, 64.0]`; NaN is treated as 0.0.
    SetSpeed { multiplier: f32 },
    /// Toggle between current speed and 0.0; restores last non-zero
    /// speed (or 1.0 if none) when un-pausing.
    TogglePause,
}
