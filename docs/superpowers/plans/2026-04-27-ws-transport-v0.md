# WS transport v0 implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **VCS:** This repo is jj-tracked (colocated with git). Use `jj`, never raw `git`. Each task ends with `jj desc` (current commit) and a verify step. The next task starts with `jj new` only if `jj st` shows `@` is non-empty.

**Goal:** Stand up a real WebSocket server in `crates/host` so a client can `wscat -c ws://127.0.0.1:9001/`, receive a `Hello` followed by an `Init` snapshot, then a `Snapshot` payload at ~30 Hz, and control sim pacing with `SetSpeed` / `TogglePause`.

**Architecture:** Single tokio multi-threaded runtime hosting three task families. A `sim_driver` task owns `Sim` and a driver-side `speed` state; one `tokio::select!` arm paces ticks via recomputed `sleep_until` and `block_in_place(|| sim.tick())`, another samples at fixed 33 ms wall-clock and writes to `tokio::sync::watch<Snapshot>`, the third handles inputs inline (pacing inputs are driver-only — they never enter a queue or hit `Sim::apply_input`). A `ws_server` task accepts on a `TcpListener`; each per-connection task does `accept_async`, sends `Hello`/`Init`, then `select!`s between `snapshot_rx.changed()` and incoming WS frames. Wire types live in `protocol::messages` as serde-tagged enums; `core::Snapshot` and `core::AgentSnapshot` gain `Serialize`/`Deserialize` derives and ride directly on the wire (decoupling deferred until needed).

**Tech Stack:** Rust 2021, `tokio` (multi-thread, macros, net, sync, time, signal), `tokio-tungstenite` for the WS upgrade and framing, `futures-util` for `SinkExt`/`StreamExt`, `serde_json` for JSON, existing `serde` derives in `core`. No frontend; client-facing testing is `wscat` (manual) or the integration test (automated).

**Spec:** [`docs/superpowers/specs/2026-04-27-ws-transport-v0-design.md`](../specs/2026-04-27-ws-transport-v0-design.md).

**Pre-flight:** Before Task 1, confirm the workspace is clean:

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

All three must succeed. If they don't, fix that before starting.

---

## Task 1: serde derives on `Snapshot` and `AgentSnapshot`

**Files:**
- Modify: `crates/core/src/snapshot.rs`

`AgentId` and `Needs` already derive `Serialize`/`Deserialize` from the schema pass. `Snapshot` and `AgentSnapshot` are the only types that need them added — they were left off in the live runtime v0 pass because nothing serialised them yet.

- [ ] **Step 1: Start the commit**

```bash
jj st
```

If `@` is non-empty (something staged from prior work), run `jj new`. If empty, reuse it. Then describe:

```bash
jj desc -m "WS transport v0: serde derives on Snapshot and AgentSnapshot"
```

- [ ] **Step 2: Write the failing compile-time check**

Append a `#[cfg(test)]` module to `crates/core/src/snapshot.rs`:

```rust
#[cfg(test)]
mod serde_derive_tests {
    use super::{AgentSnapshot, Snapshot};

    fn assert_serialize<T: serde::Serialize>() {}
    fn assert_deserialize<T: serde::de::DeserializeOwned>() {}

    #[test]
    fn snapshot_types_implement_serde() {
        assert_serialize::<Snapshot>();
        assert_deserialize::<Snapshot>();
        assert_serialize::<AgentSnapshot>();
        assert_deserialize::<AgentSnapshot>();
    }
}
```

This compiles iff the derives land. No new dev-deps needed — `serde` is already in core's dependencies.

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test -p gecko-sim-core --lib serde_derive_tests
```

Expected: compile error — `Snapshot` does not implement `Serialize` (and `AgentSnapshot` likewise).

- [ ] **Step 4: Add the serde derives**

Modify `crates/core/src/snapshot.rs`. Replace its current contents with:

```rust
//! Snapshot type: a deterministic, by-value view of sim state at a tick.
//!
//! Agents are sorted by `AgentId` ascending so two `Sim` instances built
//! from the same seed and same calls produce byte-equal `Snapshot`s.
//! Serde derives let `Snapshot` ride directly on the wire (per ADR 0013
//! and the WS transport v0 spec — wire types live in `protocol`, but the
//! `Snapshot` shape itself is the schema-of-record from `core`).

use serde::{Deserialize, Serialize};

use crate::agent::Needs;
use crate::ids::AgentId;

/// Full sim state at a tick boundary. `PartialEq` is required by the
/// determinism test in the test suite; serde derives let `protocol`
/// envelope this type without a parallel wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
}

/// Per-agent snapshot row. Holds the slice of state this pass exposes;
/// other groupings (Personality, Mood, Spatial, …) extend this type as
/// their first consumer system lands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
}

#[cfg(test)]
mod serde_derive_tests {
    use super::{AgentSnapshot, Snapshot};

    fn assert_serialize<T: serde::Serialize>() {}
    fn assert_deserialize<T: serde::de::DeserializeOwned>() {}

    #[test]
    fn snapshot_types_implement_serde() {
        assert_serialize::<Snapshot>();
        assert_deserialize::<Snapshot>();
        assert_serialize::<AgentSnapshot>();
        assert_deserialize::<AgentSnapshot>();
    }
}
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-core --lib serde_derive_tests
```

Expected: 1 passed.

- [ ] **Step 6: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. The existing `core/tests/{smoke,snapshot,needs_decay,determinism}.rs` are unaffected.

- [ ] **Step 7: Verify the commit**

```bash
jj st
```

Description: `WS transport v0: serde derives on Snapshot and AgentSnapshot`.

---

## Task 2: protocol crate wire types + roundtrip tests

**Files:**
- Modify: `crates/protocol/Cargo.toml` (add `serde_json` dev-dep)
- Modify: `crates/protocol/src/lib.rs` (declare `mod messages;` + re-exports)
- Create: `crates/protocol/src/messages.rs`
- Create: `crates/protocol/tests/roundtrip.rs`

Wire-types pass: define `ServerMessage`, `ClientMessage`, `PlayerInput`, `WireFormat`, `PROTOCOL_VERSION`. JSON roundtrip-tested for every variant.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "WS transport v0: protocol wire types and JSON roundtrip tests"
```

- [ ] **Step 2: Write the failing test**

Create `crates/protocol/tests/roundtrip.rs`:

```rust
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
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test -p gecko-sim-protocol --test roundtrip
```

Expected: compile error — none of `ServerMessage`/`ClientMessage`/`PlayerInput`/`WireFormat`/`PROTOCOL_VERSION` exist yet, and the integration test crate hasn't declared `serde_json`.

- [ ] **Step 4: Add `serde_json` as a dev-dependency**

Modify `crates/protocol/Cargo.toml`. Replace its contents with:

```toml
[package]
name = "gecko-sim-protocol"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Gecko-sim protocol: wire types shared with the frontend (later)."

[dependencies]
gecko-sim-core.workspace = true
serde.workspace = true

[dev-dependencies]
serde_json.workspace = true

[lints]
workspace = true
```

`serde_json` is added to the workspace deps in Task 3 (host Cargo.toml task). Since cargo resolves workspace-deferred deps lazily, declaring `serde_json.workspace = true` here before Task 3 will fail at this step. **Therefore Task 3's first edit (workspace `Cargo.toml`) belongs at the start of this task too.** Do that next:

- [ ] **Step 5: Add `serde_json` (and the host deps) to workspace `Cargo.toml`**

Modify the root `Cargo.toml`. Find the `[workspace.dependencies]` block:

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
bevy_ecs = "0.16"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
glam = { version = "0.30", features = ["serde"] }
rand = "0.9"
rand_pcg = { version = "0.9", features = ["serde"] }
ron = "0.10"
anyhow = "1"
bitflags = { version = "2", features = ["serde"] }

gecko-sim-core = { path = "crates/core" }
gecko-sim-content = { path = "crates/content" }
gecko-sim-protocol = { path = "crates/protocol" }
```

Insert the new external deps just above the path-deps block. The full updated block:

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
bevy_ecs = "0.16"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
glam = { version = "0.30", features = ["serde"] }
rand = "0.9"
rand_pcg = { version = "0.9", features = ["serde"] }
ron = "0.10"
anyhow = "1"
bitflags = { version = "2", features = ["serde"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "sync", "time", "signal"] }
tokio-tungstenite = "0.24"
futures-util = "0.3"

gecko-sim-core = { path = "crates/core" }
gecko-sim-content = { path = "crates/content" }
gecko-sim-protocol = { path = "crates/protocol" }
```

These workspace declarations only affect crates that opt in via `<dep>.workspace = true`; nothing else changes for `core`/`content` from this edit.

- [ ] **Step 6: Create `crates/protocol/src/messages.rs`**

```rust
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
//! MessagePack / postcard expansion without a protocol version bump.

use gecko_sim_core::Snapshot;
use serde::{Deserialize, Serialize};

/// Wire protocol version. Bump on incompatible changes; additive changes
/// (new variants, new optional fields) do not require a bump.
pub const PROTOCOL_VERSION: u32 = 1;

/// Wire encoding negotiated in `Hello.format`. Json-only for v0; the
/// extra variant slot anchors future MessagePack / postcard support.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WireFormat {
    Json,
}

/// Server-originated frame. Tagged with `"type"` field (snake_case).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
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

/// Client-originated frame. Tagged with `"type"` field (snake_case).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
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
pub enum PlayerInput {
    /// Set wall-clock tick rate. `multiplier == 0.0` means paused.
    /// Driver clamps to `[0.0, 64.0]`; NaN is treated as 0.0.
    SetSpeed { multiplier: f32 },
    /// Toggle between current speed and 0.0; restores last non-zero
    /// speed (or 1.0 if none) when un-pausing.
    TogglePause,
}
```

- [ ] **Step 7: Wire up `protocol/src/lib.rs`**

Replace the contents of `crates/protocol/src/lib.rs` with:

```rust
//! Gecko-sim protocol: wire types for the host ↔ frontend WebSocket channel.
//! See `messages` for the envelope enums and ADR 0013 for the design.

pub mod messages;

pub use messages::{ClientMessage, PlayerInput, ServerMessage, WireFormat, PROTOCOL_VERSION};
```

- [ ] **Step 8: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-protocol --test roundtrip
```

Expected: 10 passed.

- [ ] **Step 9: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 10: Verify the commit**

```bash
jj st
```

Description: `WS transport v0: protocol wire types and JSON roundtrip tests`.

---

## Task 3: host crate Cargo.toml + lib.rs scaffold

**Files:**
- Modify: `crates/host/Cargo.toml`
- Create: `crates/host/src/lib.rs`

`host` becomes a dual lib + bin crate so the integration test can drive its modules in-process. This task lands the Cargo manifest and an empty `lib.rs` shell; subsequent tasks fill in the modules. Workspace-level dep declarations are already in place from Task 2.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "WS transport v0: host lib target with tokio + tungstenite deps"
```

- [ ] **Step 2: Replace `crates/host/Cargo.toml`**

Current:

```toml
[package]
name = "gecko-sim-host"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Gecko-sim host: native binary that runs the sim and (later) serves WebSocket clients."

[dependencies]
gecko-sim-core.workspace = true
gecko-sim-content.workspace = true
gecko-sim-protocol.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true

[lints]
workspace = true
```

Replace with:

```toml
[package]
name = "gecko-sim-host"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Gecko-sim host: native binary that runs the sim and serves the WebSocket transport."

[lib]
name = "gecko_sim_host"
path = "src/lib.rs"

[[bin]]
name = "gecko-sim-host"
path = "src/main.rs"

[dependencies]
gecko-sim-core.workspace = true
gecko-sim-content.workspace = true
gecko-sim-protocol.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
tokio.workspace = true
tokio-tungstenite.workspace = true
futures-util.workspace = true
serde_json.workspace = true

[dev-dependencies]
tokio-tungstenite.workspace = true
futures-util.workspace = true
serde_json.workspace = true

[lints]
workspace = true
```

(The `dev-dependencies` block re-declares deps already in `[dependencies]` so the integration test under `tests/` picks them up identically; this is the standard cargo pattern when a runtime dep is also test-needed.)

- [ ] **Step 3: Create `crates/host/src/lib.rs`**

```rust
//! Gecko-sim host library: the WebSocket transport surface.
//!
//! The `gecko-sim-host` binary (`src/main.rs`) is a thin wrapper that
//! constructs a `Sim`, channels, and a `TcpListener`, then spawns the
//! `sim_driver` and `ws_server` tasks defined here. Exposing them as a
//! library lets `tests/ws_smoke.rs` drive the same code paths in-process
//! against an ephemeral listener.

pub mod config;
pub mod sim_driver;
pub mod ws_server;
```

- [ ] **Step 4: Add empty placeholder modules so `lib.rs` compiles**

These get fleshed out in Tasks 4–6. For now, give each a one-line stub so the build passes:

`crates/host/src/config.rs`:

```rust
//! Listen-address resolution for the host binary.
//! Filled in by Task 4.
```

`crates/host/src/sim_driver.rs`:

```rust
//! Sim-driver task: ticks `Sim`, samples snapshots, handles pacing inputs.
//! Filled in by Task 5.
```

`crates/host/src/ws_server.rs`:

```rust
//! WS server task: accepts connections and runs per-connection handlers.
//! Filled in by Task 6.
```

- [ ] **Step 5: Build to verify the manifest is valid**

```bash
cargo build --workspace
```

Expected: clean build. The new deps (`tokio`, `tokio-tungstenite`, `futures-util`, `serde_json`) resolve and compile but are unused by this commit (host's `main.rs` doesn't import them yet, and the lib stubs are empty). Cargo emits no warnings for unused deps at the manifest level; if rustc warns about unused imports inside `main.rs`, that's a separate lint (none expected at this step).

- [ ] **Step 6: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 7: Verify the commit**

```bash
jj st
```

Description: `WS transport v0: host lib target with tokio + tungstenite deps`.

---

## Task 4: host config module

**Files:**
- Modify: `crates/host/src/config.rs`

Tiny module: parse `GECKOSIM_HOST_ADDR` env var with default `127.0.0.1:9001`. Use a pure helper that takes the env value as an argument so we can unit-test without mutating process state.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "WS transport v0: host config module (GECKOSIM_HOST_ADDR)"
```

- [ ] **Step 2: Write the failing test (inline `#[cfg(test)]`)**

Replace the contents of `crates/host/src/config.rs` with the production code + tests in one edit. First, the production code:

```rust
//! Listen-address resolution for the host binary.
//!
//! Default: `127.0.0.1:9001`. Override with `GECKOSIM_HOST_ADDR=…`.
//! Loopback-only at v0 (no auth, no TLS) — see ADR 0013.

use std::env;
use std::net::SocketAddr;

const DEFAULT_ADDR: &str = "127.0.0.1:9001";

/// Environment variable consulted by [`listen_addr`].
pub const ENV_VAR: &str = "GECKOSIM_HOST_ADDR";

/// Pure helper: parse a `SocketAddr` from `Some(env_value)` or fall back
/// to the v0 default. Exposed for tests; production calls use [`listen_addr`].
pub fn parse_addr(raw: Option<&str>) -> anyhow::Result<SocketAddr> {
    let s = raw.unwrap_or(DEFAULT_ADDR);
    s.parse::<SocketAddr>()
        .map_err(|e| anyhow::anyhow!("invalid {ENV_VAR}={s:?}: {e}"))
}

/// Resolve the listen address from `GECKOSIM_HOST_ADDR` or fall back to
/// `127.0.0.1:9001`. Reads process env at call time.
pub fn listen_addr() -> anyhow::Result<SocketAddr> {
    parse_addr(env::var(ENV_VAR).ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_loopback_9001() {
        let addr = parse_addr(None).expect("parse default");
        assert_eq!(addr.to_string(), "127.0.0.1:9001");
    }

    #[test]
    fn override_with_ephemeral_port_parses() {
        let addr = parse_addr(Some("127.0.0.1:0")).expect("parse override");
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 0);
    }

    #[test]
    fn invalid_addr_returns_err() {
        let err = parse_addr(Some("not a socket addr")).expect_err("should fail");
        let msg = format!("{err}");
        assert!(msg.contains(ENV_VAR), "msg = {msg}");
    }
}
```

- [ ] **Step 3: Run the tests**

```bash
cargo test -p gecko-sim-host --lib config::tests
```

Expected: 3 passed.

- [ ] **Step 4: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 5: Verify the commit**

```bash
jj st
```

Description: `WS transport v0: host config module (GECKOSIM_HOST_ADDR)`.

---

## Task 5: host sim_driver module

**Files:**
- Modify: `crates/host/src/sim_driver.rs`

The `select!` loop. Three arms: tick deadline, sample interval, input channel. `block_in_place` around the sync `Sim` calls. Pacing inputs handled inline.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "WS transport v0: sim_driver task with tick/sample/input select loop"
```

- [ ] **Step 2: Write the failing tests for the input-handler helper**

The body of `sim_driver::run` is hard to unit-test because it blocks on real time. The cleanest factoring is to extract a pure `apply_pacing_input` helper that the loop calls — pure code = unit-testable. Write the tests first in `crates/host/src/sim_driver.rs`:

```rust
//! Sim-driver task: ticks `Sim`, samples snapshots, handles pacing inputs.
//!
//! Topology and rationale: see `docs/superpowers/specs/2026-04-27-ws-transport-v0-design.md`.
//! Single-task `tokio::select!` loop with three arms:
//!   1. tick deadline (paced at `1.0 / speed` sec; disabled when `speed == 0.0`)
//!   2. sample interval (33 ms wall-clock, always)
//!   3. input channel (handled inline; pacing inputs never enter `Sim`)
//!
//! `block_in_place` wraps `sim.tick()` and `sim.snapshot()` so a slow
//! tick doesn't starve other tasks on the same tokio worker.

use std::time::Duration;

use gecko_sim_core::{Sim, Snapshot};
use gecko_sim_protocol::PlayerInput;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, sleep_until};

const SAMPLE_PERIOD: Duration = Duration::from_millis(33);
const MAX_SPEED: f32 = 64.0;
const DEFAULT_SPEED: f32 = 1.0;

/// Driver-side pacing state. Lives on the sim_driver task; never seen by
/// the sim or by clients beyond its observable effect on tick rate.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PacingState {
    speed: f32,
    last_nonzero_speed: f32,
}

impl Default for PacingState {
    fn default() -> Self {
        Self {
            speed: DEFAULT_SPEED,
            last_nonzero_speed: DEFAULT_SPEED,
        }
    }
}

impl PacingState {
    fn apply(&mut self, input: PlayerInput) {
        match input {
            PlayerInput::SetSpeed { multiplier } => {
                let m = if multiplier.is_nan() {
                    0.0
                } else {
                    multiplier.clamp(0.0, MAX_SPEED)
                };
                self.speed = m;
                if m > 0.0 {
                    self.last_nonzero_speed = m;
                }
            }
            PlayerInput::TogglePause => {
                self.speed = if self.speed == 0.0 {
                    self.last_nonzero_speed
                } else {
                    0.0
                };
            }
        }
    }

    fn tick_period(self) -> Option<Duration> {
        if self.speed > 0.0 {
            Some(Duration::from_secs_f32(1.0 / self.speed))
        } else {
            None
        }
    }
}

/// Drive the sim. Owns `sim`, drains `input_rx`, publishes snapshots
/// to `snapshot_tx` at 33 ms cadence.
pub async fn run(
    mut sim: Sim,
    mut input_rx: mpsc::UnboundedReceiver<PlayerInput>,
    snapshot_tx: watch::Sender<Snapshot>,
) {
    let mut pacing = PacingState::default();
    let mut last_tick_at = Instant::now();

    let mut sample = tokio::time::interval(SAMPLE_PERIOD);
    sample.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        let next_tick_deadline = pacing.tick_period().map(|p| last_tick_at + p);

        tokio::select! {
            biased;

            _ = sample.tick() => {
                let snap = tokio::task::block_in_place(|| sim.snapshot());
                let _ = snapshot_tx.send_replace(snap);
            }

            _ = wait_for_optional_deadline(next_tick_deadline) => {
                tokio::task::block_in_place(|| { sim.tick(); });
                last_tick_at = Instant::now();
            }

            maybe_input = input_rx.recv() => {
                match maybe_input {
                    Some(input) => pacing.apply(input),
                    None => break, // sender dropped — host is shutting down
                }
            }
        }
    }
}

/// `select!`-friendly wrapper: when `Some(deadline)`, sleep until it;
/// when `None`, never resolve.
async fn wait_for_optional_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(d) => sleep_until(d).await,
        None => std::future::pending::<()>().await,
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact comparisons against literal speeds we set
mod tests {
    use super::*;

    #[test]
    fn default_speed_is_one() {
        let p = PacingState::default();
        assert_eq!(p.speed, 1.0);
        assert_eq!(p.last_nonzero_speed, 1.0);
        assert_eq!(p.tick_period(), Some(Duration::from_secs_f32(1.0)));
    }

    #[test]
    fn set_speed_clamps_above_64() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 1000.0 });
        assert_eq!(p.speed, 64.0);
        assert_eq!(p.last_nonzero_speed, 64.0);
    }

    #[test]
    fn set_speed_clamps_below_zero() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: -5.0 });
        assert_eq!(p.speed, 0.0);
        // Negative input does not update last_nonzero_speed.
        assert_eq!(p.last_nonzero_speed, 1.0);
    }

    #[test]
    fn set_speed_nan_treated_as_zero() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: f32::NAN });
        assert_eq!(p.speed, 0.0);
    }

    #[test]
    fn set_speed_zero_pauses_without_losing_resume_value() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 4.0 });
        assert_eq!(p.last_nonzero_speed, 4.0);
        p.apply(PlayerInput::SetSpeed { multiplier: 0.0 });
        assert_eq!(p.speed, 0.0);
        assert_eq!(p.last_nonzero_speed, 4.0); // remembered for resume
    }

    #[test]
    fn toggle_pause_from_running_pauses() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 8.0 });
        p.apply(PlayerInput::TogglePause);
        assert_eq!(p.speed, 0.0);
        assert_eq!(p.last_nonzero_speed, 8.0);
    }

    #[test]
    fn toggle_pause_from_paused_restores_last_nonzero() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 8.0 });
        p.apply(PlayerInput::TogglePause); // pause
        p.apply(PlayerInput::TogglePause); // resume
        assert_eq!(p.speed, 8.0);
    }

    #[test]
    fn paused_has_no_tick_period() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 0.0 });
        assert_eq!(p.tick_period(), None);
    }

    #[test]
    fn tick_period_inverts_speed() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 4.0 });
        assert_eq!(p.tick_period(), Some(Duration::from_secs_f32(0.25)));
    }
}
```

- [ ] **Step 3: Run the tests**

```bash
cargo test -p gecko-sim-host --lib sim_driver::tests
```

Expected: 9 passed.

- [ ] **Step 4: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. Clippy will check `sim_driver::run` even though it's not exercised yet — verify it lints clean.

- [ ] **Step 5: Verify the commit**

```bash
jj st
```

Description: `WS transport v0: sim_driver task with tick/sample/input select loop`.

---

## Task 6: host ws_server module

**Files:**
- Modify: `crates/host/src/ws_server.rs`

Accept loop + per-connection handler. Send `Hello`, await `ClientHello`, send `Init`, then `select!` between snapshot updates and incoming frames. Tested end-to-end in Task 8.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "WS transport v0: ws_server task and per-connection handler"
```

- [ ] **Step 2: Replace `crates/host/src/ws_server.rs`**

```rust
//! WS server task: accepts connections and runs per-connection handlers.
//!
//! Per-connection flow (ADR 0013, "Connection lifecycle"):
//!   1. `tokio_tungstenite::accept_async` → upgrade to WebSocket.
//!   2. Send `ServerMessage::Hello`.
//!   3. Read first frame, parse as `ClientMessage::ClientHello`
//!      (`last_known_tick` is parsed-but-ignored at v0).
//!   4. Read latest snapshot from `watch`, send `ServerMessage::Init`.
//!   5. Loop: forward `Snapshot` messages on every `watch::changed()`;
//!      forward inbound `PlayerInput` frames to `input_tx`.
//!
//! Multi-client falls out of `tokio::sync::watch` for free: every
//! per-connection task subscribes its own `Receiver`. Inputs from any
//! client apply, with no sender attribution (v0).

use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use gecko_sim_core::Snapshot;
use gecko_sim_protocol::{
    ClientMessage, PlayerInput, ServerMessage, WireFormat, PROTOCOL_VERSION,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Run the accept loop on `listener`. Each accepted connection spawns a
/// per-connection task. This function returns only when the listener
/// errors fatally; `host::main` aborts the join handle on shutdown.
pub async fn run(
    listener: TcpListener,
    input_tx: mpsc::UnboundedSender<PlayerInput>,
    snapshot_rx: watch::Receiver<Snapshot>,
) {
    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(?e, "accept failed");
                continue;
            }
        };
        let input_tx = input_tx.clone();
        let snapshot_rx = snapshot_rx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, peer_addr, input_tx, snapshot_rx).await {
                tracing::info!(%peer_addr, error = %e, "connection ended");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    input_tx: mpsc::UnboundedSender<PlayerInput>,
    mut snapshot_rx: watch::Receiver<Snapshot>,
) -> anyhow::Result<()> {
    tracing::info!(%peer_addr, "ws connection accepted");
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut tx, mut rx) = ws.split();

    // 1. Hello.
    let hello = ServerMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        format: WireFormat::Json,
    };
    tx.send(WsMessage::Text(serde_json::to_string(&hello)?)).await?;

    // 2. Wait for ClientHello (parsed but field ignored at v0).
    let first = rx
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("client closed before ClientHello"))??;
    match first {
        WsMessage::Text(text) => {
            let _client_hello: ClientMessage = serde_json::from_str(&text)?;
        }
        other => {
            return Err(anyhow::anyhow!(
                "expected text ClientHello, got {other:?}"
            ));
        }
    }

    // 3. Init.
    let snap = snapshot_rx.borrow_and_update().clone();
    let init = ServerMessage::Init {
        current_tick: snap.tick,
        snapshot: snap,
    };
    tx.send(WsMessage::Text(serde_json::to_string(&init)?)).await?;

    // 4. Stream loop.
    loop {
        tokio::select! {
            changed = snapshot_rx.changed() => {
                if changed.is_err() {
                    // sim_driver dropped — host is shutting down.
                    break;
                }
                let snap = snapshot_rx.borrow_and_update().clone();
                let msg = ServerMessage::Snapshot { snapshot: snap };
                tx.send(WsMessage::Text(serde_json::to_string(&msg)?)).await?;
            }
            frame = rx.next() => {
                match frame {
                    Some(Ok(WsMessage::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(ClientMessage::PlayerInput(input)) => {
                                let _ = input_tx.send(input);
                            }
                            Ok(ClientMessage::ClientHello { .. }) => {
                                // No reconnect handshake mid-stream at v0; ignore.
                            }
                            Err(e) => {
                                tracing::debug!(%peer_addr, error = %e, "bad client frame, ignoring");
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {
                        // Binary frames aren't part of v0; tungstenite handles
                        // ping/pong frames internally before we see them.
                    }
                    Some(Err(e)) => return Err(e.into()),
                }
            }
        }
    }

    tracing::info!(%peer_addr, "ws connection closed");
    Ok(())
}
```

- [ ] **Step 3: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. No new tests yet — Task 8 is the integration test that exercises this code.

If clippy flags `unused_variables` for `peer_addr` in some path, the `tracing::info!` at the top should keep it live. If a clippy `pedantic` lint flags the long signatures or the `match` arm on `Some(Ok(_))`, allow the lint locally with a brief reason rather than restructuring.

- [ ] **Step 4: Verify the commit**

```bash
jj st
```

Description: `WS transport v0: ws_server task and per-connection handler`.

---

## Task 7: rewire `host/src/main.rs`

**Files:**
- Modify: `crates/host/src/main.rs`

Replace the synchronous demo loop with an async entrypoint that constructs the channels, spawns the driver and server, and awaits ctrl-c.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "WS transport v0: main.rs spawns driver + server, awaits ctrl-c"
```

- [ ] **Step 2: Replace `crates/host/src/main.rs`**

Current:

```rust
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
```

Replace with:

```rust
use gecko_sim_core::{ContentBundle, Sim};
use gecko_sim_host::{config, sim_driver, ws_server};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEMO_SEED: u64 = 0xDEAD_BEEF;

#[expect(
    clippy::default_constructed_unit_structs,
    reason = "ContentBundle is a unit struct placeholder until content loading lands"
)]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));

    let mut sim = Sim::new(DEMO_SEED, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");

    let initial = sim.snapshot();
    tracing::info!(agents = initial.agents.len(), "sim primed");

    let addr = config::listen_addr()?;
    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!(%local_addr, "ws transport listening");

    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel(initial);

    let driver = tokio::spawn(sim_driver::run(sim, input_rx, snapshot_tx));
    let server = tokio::spawn(ws_server::run(listener, input_tx, snapshot_rx));

    tokio::signal::ctrl_c().await?;
    tracing::info!("ctrl-c received, shutting down");
    driver.abort();
    server.abort();
    Ok(())
}
```

- [ ] **Step 3: Smoke-test the binary manually**

```bash
cargo run -p gecko-sim-host
```

Expected output (within ~1 second):
- `gecko-sim host v0.1.0`
- `agents = 3 sim primed`
- `local_addr = 127.0.0.1:9001 ws transport listening`
- (process stays running; press Ctrl-C)

After Ctrl-C, expected:
- `ctrl-c received, shutting down`
- process exits 0.

If port 9001 is already in use, run with an override:

```bash
GECKOSIM_HOST_ADDR=127.0.0.1:9101 cargo run -p gecko-sim-host
```

(Optional manual verification with `wscat`:)

```bash
wscat -c ws://127.0.0.1:9001/
> {"type":"client_hello","last_known_tick":null}
```

Expected to see one `{"type":"hello",…}` then one `{"type":"init",…}` then a stream of `{"type":"snapshot",…}` messages. If `wscat` isn't installed, skip — Task 8's integration test covers this automatically.

- [ ] **Step 4: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 5: Verify the commit**

```bash
jj st
```

Description: `WS transport v0: main.rs spawns driver + server, awaits ctrl-c`.

---

## Task 8: ws_smoke integration test

**Files:**
- Create: `crates/host/tests/ws_smoke.rs`

End-to-end: spawn the driver + server in-process on an ephemeral port, drive the protocol with a `tokio-tungstenite` client.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "WS transport v0: end-to-end ws_smoke integration test"
```

- [ ] **Step 2: Create `crates/host/tests/ws_smoke.rs`**

```rust
//! End-to-end smoke test: spawn the host's driver + server tasks in
//! the same process on an ephemeral port; drive the protocol with a
//! tokio-tungstenite client.
//!
//! Exercises: Hello/Init handshake, periodic Snapshot streaming with
//! monotonic tick, TogglePause stops ticks, second TogglePause resumes.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use gecko_sim_core::{ContentBundle, Sim, Snapshot};
use gecko_sim_host::{sim_driver, ws_server};
use gecko_sim_protocol::{
    ClientMessage, PlayerInput, ServerMessage, WireFormat, PROTOCOL_VERSION,
};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message as WsMessage;

async fn next_text<S>(stream: &mut S) -> String
where
    S: StreamExt<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let msg = stream
            .next()
            .await
            .expect("stream ended early")
            .expect("ws error");
        if let WsMessage::Text(s) = msg {
            return s;
        }
    }
}

async fn next_server_msg<S>(stream: &mut S) -> ServerMessage
where
    S: StreamExt<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let text = next_text(stream).await;
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("bad server msg {text}: {e}"))
}

async fn next_snapshot_tick<S>(stream: &mut S) -> u64
where
    S: StreamExt<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match next_server_msg(stream).await {
            ServerMessage::Snapshot { snapshot } => return snapshot.tick,
            ServerMessage::Init { current_tick, .. } => return current_tick,
            ServerMessage::Hello { .. } => continue,
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ws_handshake_and_pause_resume() {
    // 1. Build a sim with three agents.
    let mut sim = Sim::new(0xC0FFEE, ContentBundle);
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");
    let initial = sim.snapshot();

    // 2. Wire channels and bind a listener on an ephemeral port.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral");
    let local_addr = listener.local_addr().expect("local_addr");

    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel::<Snapshot>(initial);

    let driver = tokio::spawn(sim_driver::run(sim, input_rx, snapshot_tx));
    let server = tokio::spawn(ws_server::run(listener, input_tx, snapshot_rx));

    // Give the server a moment to start the accept loop.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 3. Connect a WS client.
    let url = format!("ws://{local_addr}/");
    let (ws, _resp) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("connect");
    let (mut tx, mut rx) = ws.split();

    // 4. Server sends Hello first.
    match next_server_msg(&mut rx).await {
        ServerMessage::Hello {
            protocol_version,
            format,
        } => {
            assert_eq!(protocol_version, PROTOCOL_VERSION);
            assert_eq!(format, WireFormat::Json);
        }
        other => panic!("expected Hello, got {other:?}"),
    }

    // 5. Client sends ClientHello.
    let hello = ClientMessage::ClientHello {
        last_known_tick: None,
    };
    tx.send(WsMessage::Text(serde_json::to_string(&hello).unwrap()))
        .await
        .expect("send ClientHello");

    // 6. Server sends Init.
    match next_server_msg(&mut rx).await {
        ServerMessage::Init {
            current_tick,
            snapshot,
        } => {
            assert_eq!(current_tick, snapshot.tick);
            assert_eq!(snapshot.agents.len(), 3);
        }
        other => panic!("expected Init, got {other:?}"),
    }

    // 7. Observe a few Snapshot messages with monotonic non-decreasing tick.
    let mut last_tick = 0u64;
    let mut saw_advance = false;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(2_000);
    while tokio::time::Instant::now() < deadline {
        let t = tokio::time::timeout(Duration::from_millis(500), next_snapshot_tick(&mut rx))
            .await
            .expect("snapshot timeout");
        assert!(t >= last_tick, "tick went backward: {t} < {last_tick}");
        if t > last_tick {
            saw_advance = true;
        }
        last_tick = t;
        if saw_advance && last_tick >= 1 {
            break;
        }
    }
    assert!(saw_advance, "expected at least one tick advance within 2s");

    // 8. TogglePause; ticks should freeze.
    let pause = ClientMessage::PlayerInput(PlayerInput::TogglePause);
    tx.send(WsMessage::Text(serde_json::to_string(&pause).unwrap()))
        .await
        .expect("send TogglePause (pause)");

    // Allow a few sample cycles to flush, then assert no advance over 500 ms.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let frozen_tick =
        tokio::time::timeout(Duration::from_millis(200), next_snapshot_tick(&mut rx))
            .await
            .expect("snapshot timeout while paused");
    let later_tick =
        tokio::time::timeout(Duration::from_millis(500), async {
            let mut latest = frozen_tick;
            for _ in 0..10 {
                latest = next_snapshot_tick(&mut rx).await;
            }
            latest
        })
        .await
        .expect("paused snapshots timeout");
    assert_eq!(
        later_tick, frozen_tick,
        "tick advanced while paused: frozen={frozen_tick}, later={later_tick}"
    );

    // 9. TogglePause again; ticks should resume.
    let resume = ClientMessage::PlayerInput(PlayerInput::TogglePause);
    tx.send(WsMessage::Text(serde_json::to_string(&resume).unwrap()))
        .await
        .expect("send TogglePause (resume)");

    let mut resumed_advance = false;
    let resume_deadline = tokio::time::Instant::now() + Duration::from_millis(2_500);
    while tokio::time::Instant::now() < resume_deadline {
        let t = tokio::time::timeout(Duration::from_millis(500), next_snapshot_tick(&mut rx))
            .await
            .expect("post-resume snapshot timeout");
        if t > later_tick {
            resumed_advance = true;
            break;
        }
    }
    assert!(resumed_advance, "tick did not resume after second TogglePause");

    // 10. Tear down.
    driver.abort();
    server.abort();
}
```

- [ ] **Step 3: Run the new test**

```bash
cargo test -p gecko-sim-host --test ws_smoke -- --nocapture
```

Expected: 1 passed. The test runs against real wall-clock and takes a couple of seconds.

If the test fails on a CI environment with very slow scheduling, the per-step timeouts (`500ms`, `200ms`, `2_000ms`, `2_500ms`) may need to be widened. Do not silently extend them — diagnose first. The test budgets are sized so 1× speed (1 tick/sec) demonstrably advances within the windows.

- [ ] **Step 4: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. The full test suite now runs `core` + `protocol` + `host` tests in parallel.

- [ ] **Step 5: Verify the commit**

```bash
jj st
```

Description: `WS transport v0: end-to-end ws_smoke integration test`.

---

## Task 9: Final verification

**Files:** none modified — verification only.

- [ ] **Step 1: Run the full DoD check suite**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

All three must succeed with zero warnings.

- [ ] **Step 2: Manual binary smoke**

```bash
cargo run -p gecko-sim-host
```

Expected within ~1 second:
- `gecko-sim host v0.1.0`
- `sim primed (agents = 3)`
- `ws transport listening (local_addr = 127.0.0.1:9001)`

Press Ctrl-C; expected: `ctrl-c received, shutting down`; process exits 0.

If you have `wscat` installed (`npm i -g wscat`), open a second terminal and run:

```bash
wscat -c ws://127.0.0.1:9001/
```

Then in the wscat prompt:

```
> {"type":"client_hello","last_known_tick":null}
```

Expected (each on its own line): a `hello` frame, then an `init` frame containing three agents, then a stream of `snapshot` frames each with a slightly higher `tick`. Send `{"type":"player_input","kind":"toggle_pause"}` to see the `tick` freeze; send it again to resume.

- [ ] **Step 3: Inspect the commit chain**

```bash
jj log -r 'trunk()..@' --no-graph -T 'change_id.shortest() ++ "  " ++ description.first_line() ++ "\n"'
```

Expected (top to bottom):

```
<id>  WS transport v0: end-to-end ws_smoke integration test
<id>  WS transport v0: main.rs spawns driver + server, awaits ctrl-c
<id>  WS transport v0: ws_server task and per-connection handler
<id>  WS transport v0: sim_driver task with tick/sample/input select loop
<id>  WS transport v0: host config module (GECKOSIM_HOST_ADDR)
<id>  WS transport v0: host lib target with tokio + tungstenite deps
<id>  WS transport v0: protocol wire types and JSON roundtrip tests
<id>  WS transport v0: serde derives on Snapshot and AgentSnapshot
<id>  WS transport v0 spec: tokio-tungstenite + watch-broadcast snapshot stream + SetSpeed/TogglePause
…
```

Default recommendation: **keep the per-task chain.** The commits are atomic per-task and tell a clean implementation story. Squashing into one is optional and non-load-bearing — same convention as the live runtime v0 pass.

- [ ] **Step 4: Pass complete**

The WS transport v0 pass is done. Frontend, RON content loading, and system #2 are all unblocked as independent next passes (see the spec's "What this pass enables next" section).

---

## Spec coverage check

| Spec section | Covered by |
|---|---|
| Concurrency topology (sim_driver, ws_server, per-conn) | Task 5, 6 |
| `block_in_place` + `sleep_until` deadline pattern | Task 5 |
| `tokio::sync::watch` for coalescing snapshots | Task 5 (sender) + Task 6 (receiver) |
| `tokio::sync::mpsc::Unbounded` for inputs | Task 5 (receiver) + Task 6 (sender) + Task 7 (wiring) |
| Sample loop runs even when paused | Task 5 (sample arm fires unconditionally) |
| `Sim::apply_input` deferred | Spec "Deferred items" section; no task adds it |
| Pacing inputs handled inline (no queue) | Task 5 (`PacingState::apply`) |
| Channel summary table | Task 5 (sender) + Task 6 (receiver) |
| Shutdown via `tokio::signal::ctrl_c` + `JoinHandle::abort` | Task 7 |
| `core` adds `Serialize`/`Deserialize` to `Snapshot`/`AgentSnapshot` | Task 1 |
| `core` does not gain `apply_input` | (none — explicitly skipped) |
| `protocol` declares `ServerMessage`/`ClientMessage`/`PlayerInput`/`WireFormat`/`PROTOCOL_VERSION` | Task 2 |
| `protocol` roundtrip tests for every variant | Task 2 |
| `host` adds `lib.rs` exposing `config`/`sim_driver`/`ws_server` | Task 3 |
| `host` adds `config.rs` parsing `GECKOSIM_HOST_ADDR` | Task 4 |
| `host` `main.rs` switches to async + ctrl-c | Task 7 |
| Cargo deps: `tokio`, `tokio-tungstenite`, `futures-util`, `serde_json` | Task 2 (workspace) + Task 3 (host) |
| ts-rs setup deferred | (none — explicitly skipped) |
| `Delta`/`PromotedEvent` deferred | (none — explicitly skipped) |
| End-to-end Hello → Init → Snapshot stream + pause/resume | Task 8 |
| Definition of done (build/test/clippy/run + manual `wscat`) | Task 9 |

No spec section is unaddressed.
