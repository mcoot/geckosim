# WS transport v0 — minimal control plane over WebSocket

- **Date:** 2026-04-27
- **Status:** Approved
- **Scope:** Third implementation pass on the gecko sim. Stands up a real WebSocket server in `crates/host`, a thin envelope-protocol in `crates/protocol`, serde wire-derives in `crates/core`, and the smallest interactive control loop (pause / set-speed) over the wire.
- **Predecessor:** [`2026-04-26-live-runtime-v0-design.md`](2026-04-26-live-runtime-v0-design.md) — `Sim` is `bevy_ecs`-backed, ticks deterministically, exposes `new`/`tick`/`current_tick`/`snapshot`/`spawn_test_agent`. `delta_since` and `apply_input` are deferred from that pass.

## Goal

End state: `cargo run -p gecko-sim-host` boots a `Sim`, spawns three test agents, and serves a WebSocket endpoint at `127.0.0.1:9001`. A connecting client receives a `Hello` followed by an `Init` (current snapshot), then a `Snapshot` payload at ~30 Hz wall-clock, indefinitely. The client can send `SetSpeed` and `TogglePause` to control the sim's tick pacing; effects are observable in the `tick` field of subsequent snapshots within ~200 ms. The host shuts down cleanly on Ctrl-C.

This is the smallest "B-shape" slice from ADR 0013 — scaffold + minimal control plane — with no `Delta`, no `PromotedEvent`, no inspection, no save/load.

## Non-goals (deferred — see "Deferred items" section)

- No `Delta` or `delta_since`. Snapshot-only on the wire.
- No `Sim::apply_input`. v0 has no sim-bound `PlayerInput` variants (only driver-bound ones); the API method waits for the first sim-bound input.
- No `PromotedEvent` stream.
- No reconnect logic beyond "fresh `Init` every time" — `ClientHello.last_known_tick` is parsed and ignored.
- No `ts-rs` setup. The frontend pass introduces it.
- No MessagePack / postcard. JSON-only; `WireFormat` enum reserves the seat.
- No auth / TLS / non-loopback bind.
- No save/load over the wire (`RequestSave`/`RequestLoad`/`RequestRestart` variants of `PlayerInput` not added).
- No other `PlayerInput` variants (`SetPolicy`/`NudgeAgent`) — those land with their consumer systems.
- No multi-client coordination. Multiple connections are accepted and broadcast the same stream; inputs from any client apply, with no sender attribution. ADR 0013 calls this single-client v0; the broadcast-by-default behaviour is a free side-effect of using `tokio::sync::watch`.
- No `bevy_ecs::Schedule` ceremony in `core`. Still single-system; introduced when system #2 lands.

## Architecture

### Concurrency topology

The `host` binary runs a single tokio multi-threaded runtime. Three task families live inside it:

```
            ┌───────────────────────────┐
            │      sim_driver task      │
            │  owns Sim, speed state    │
            │  ─ tick_interval(1/speed) │
            │  ─ sample_interval(33ms)  │
            │  ─ block_in_place(tick)   │
            └──┬──────────────┬─────────┘
               │ snapshot_tx  │ pacing inputs (handled inline)
               ▼              │
       watch::channel<Snapshot>
               │
               ▼
            ┌─────────────────┐                ┌──────────────┐
            │ ws_server task  │── accept ──▶  │ per-conn task │
            │ tokio::TcpListn │                │ ─ Hello/Init  │
            └─────────────────┘                │ ─ snap → WS   │
                                               │ ─ WS → input  │
                                               └──────┬────────┘
                                                      │
                                            input_tx (mpsc::Unbounded)
                                                      │
                                                      ▼
                                         (back into sim_driver)
```

**sim_driver** owns `Sim` and the driver-side `speed: f32` (initial `1.0`) and `last_nonzero_speed: f32` (initial `1.0`). Body is a single `tokio::select!` loop with three arms:

1. **Tick deadline fires.** The tick deadline is computed each loop iteration as `Some(last_tick_at + Duration::from_secs_f32(1.0 / speed))` when `speed > 0.0`, and `None` otherwise. The arm uses `tokio::time::sleep_until(deadline).await` against the next deadline; when paused, the arm holds an `std::future::pending()` (never resolves), so `select!` cannot fire it. We use recomputed `sleep_until` rather than a long-lived `tokio::time::Interval` because tick rate changes dynamically with `SetSpeed`, and `Interval` doesn't gracefully reconfigure mid-flight. When the deadline hits, the driver runs `tokio::task::block_in_place(|| sim.tick())`. `block_in_place` is the no-spawn equivalent of `spawn_blocking`: it tells the runtime "this task is about to block briefly" and re-routes other tasks off this worker. It requires the multi-threaded runtime, which we use. Choosing `block_in_place` over `spawn_blocking` avoids moving `Sim` into a closure and back, which is awkward across an `&mut` boundary.

2. **`sample_interval` fires (33 ms, always).** `let snap = sim.snapshot(); snapshot_tx.send_replace(snap);`. Uses a long-lived `tokio::time::Interval` because the sample rate is fixed. The sample loop runs even when paused — clients keep receiving identical snapshots. This is how the renderer distinguishes "paused" from "frozen connection". `send_replace` clobbers any unread value — `watch` is inherently coalescing, satisfying ADR 0013's "replace queued delta with the latest" requirement for free.

3. **`input_rx` receives a `PlayerInput`.** Handled inline. v0 only has driver-bound variants:
   - `SetSpeed { multiplier }` → if `multiplier.is_nan()`, treat as `0.0` (defensive — `f32::clamp` propagates NaN); else `speed = multiplier.clamp(0.0, 64.0)`; if the resulting `speed > 0.0`, also update `last_nonzero_speed`.
   - `TogglePause` → if `speed == 0.0`, set `speed = last_nonzero_speed`; else set `speed = 0.0`.
   Tick deadline is recomputed on the next loop iteration.

**ws_server** owns a `tokio::net::TcpListener` and an accept loop. Each accepted connection spawns a per-connection task with a clone of `input_tx` and a fresh `snapshot_rx = snapshot_tx.subscribe()`.

**Per-connection task** flow:

1. `tokio_tungstenite::accept_async(stream).await` → upgrade to WS.
2. Send `ServerMessage::Hello { protocol_version: 1, format: WireFormat::Json }` (JSON-encoded text frame).
3. Await one client frame; deserialize as `ClientMessage::ClientHello`. The `last_known_tick` field is parsed and ignored at v0; reconnect always returns a fresh `Init`.
4. `let snap = snapshot_rx.borrow_and_update().clone();` → send `ServerMessage::Init { current_tick: snap.tick, snapshot: snap }`.
5. Main loop:
   ```
   loop {
       tokio::select! {
           changed = snapshot_rx.changed() => {
               if changed.is_err() { break; }  // sim_driver dropped
               let snap = snapshot_rx.borrow_and_update().clone();
               ws.send(ServerMessage::Snapshot { snapshot: snap }).await?;
           }
           msg = ws.next() => match msg {
               Some(Ok(WsMessage::Text(s))) => {
                   if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&s) {
                       if let ClientMessage::PlayerInput(input) = client_msg {
                           let _ = input_tx.send(input);  // unbounded; never blocks
                       }
                   }
               }
               Some(Ok(WsMessage::Close(_))) | None => break,
               Some(Ok(_)) => {}  // ignore Binary/Ping/Pong; tungstenite handles Ping internally
               Some(Err(_)) => break,
           }
       }
   }
   ```

Backpressure: a slow client's `ws.send().await` blocks only that connection's task. `watch` keeps only the latest snapshot — the connection just sees newer data when it next reads the watch. Sim is unaffected.

### Channel summary

| Channel | Type | Sender | Receiver | Purpose |
|---|---|---|---|---|
| `input_tx`/`input_rx` | `tokio::sync::mpsc::Unbounded<PlayerInput>` | per-conn (any) | sim_driver (single) | client → sim pacing inputs |
| `snapshot_tx`/`snapshot_rx` | `tokio::sync::watch<Snapshot>` | sim_driver | per-conn (many) | sim → all clients, coalescing |

Unbounded mpsc is safe for inputs — they're user-driven (mouse / keyboard rate). Bounding can land if/when input volume is dominated by NudgeAgent or similar.

### Shutdown

`main` awaits `tokio::signal::ctrl_c()`. On signal, it aborts the `sim_driver` and `ws_server` task handles via `JoinHandle::abort`. Tokio drops in-flight WS connections; tungstenite closes the underlying TCP. Best-effort; no graceful close frames.

## Module changes by crate

### `gecko-sim-core`

- **Modified:** `src/snapshot.rs` adds `serde::{Serialize, Deserialize}` derives on `Snapshot` and `AgentSnapshot`. `src/agent/mod.rs` adds the same derives on `Needs`. `src/ids.rs` adds the same derives on `AgentId` (the only ID needed by the v0 wire snapshot). Other ID types (`ObjectId`, `BuildingId`, …) gain serde derives when their first wire consumer lands.
- **New:** none.
- **Cargo.toml:** add `serde.workspace = true` to `[dependencies]`.
- **Untouched:** `sim.rs`, `systems/`, `world/`, `object/`, `decision/`, `macro_/`, `events/`, `time/`, `save/`, `rng/`. Notably, **`Sim::apply_input` is not added** — see "Deferred items".

### `gecko-sim-content`

- **Untouched.**

### `gecko-sim-protocol`

- **New:** `src/messages.rs` containing `ServerMessage`, `ClientMessage`, `PlayerInput`, `WireFormat`, and `pub const PROTOCOL_VERSION: u32 = 1;`.
- **Modified:** `src/lib.rs` declares `mod messages;` and re-exports.
- **Cargo.toml:** add `serde.workspace = true` (already declared as a workspace dep but not depended-on by `protocol` yet).

### `gecko-sim-host`

- **New:** `src/lib.rs` (declares `pub mod config; pub mod sim_driver; pub mod ws_server;` so the integration test can drive the modules in-process); `src/config.rs` (parses `GECKOSIM_HOST_ADDR`, default `127.0.0.1:9001`); `src/sim_driver.rs` (the `select!` loop); `src/ws_server.rs` (`TcpListener` accept loop + per-conn task).
- **Modified:** `src/main.rs` becomes a thin binary entrypoint that imports from `gecko_sim_host::{config, sim_driver, ws_server}` and wires them under `#[tokio::main(flavor = "multi_thread")]` with a ctrl-c handler.
- **Cargo.toml:** add `tokio` (with `rt-multi-thread`, `macros`, `net`, `sync`, `time`, `signal`), `tokio-tungstenite`, `futures-util`, `serde_json` as workspace-deferred deps. Workspace-level `Cargo.toml` declares the version pins for these new crates so other crates pick them up consistently later.

## Wire types (`protocol/src/messages.rs`)

```rust
use gecko_sim_core::Snapshot;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WireFormat {
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Hello {
        protocol_version: u32,
        format: WireFormat,
    },
    Init {
        current_tick: u64,
        snapshot: Snapshot,
    },
    Snapshot {
        snapshot: Snapshot,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    ClientHello {
        last_known_tick: Option<u64>,
    },
    PlayerInput(PlayerInput),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayerInput {
    SetSpeed { multiplier: f32 },
    TogglePause,
}
```

`#[serde(tag = "type")]` produces `{"type": "hello", "protocol_version": 1, "format": "json"}` etc. — readable in browser devtools, easy for the future TS frontend.

## `host/src/main.rs`

```rust
use gecko_sim_core::{ContentBundle, Sim};
use tracing_subscriber::EnvFilter;

mod config;
mod sim_driver;
mod ws_server;

const DEMO_SEED: u64 = 0xDEAD_BEEF;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));

    let mut sim = Sim::new(DEMO_SEED, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");

    let addr = config::listen_addr()?;
    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel(sim.snapshot());

    let driver = tokio::spawn(sim_driver::run(sim, input_rx, snapshot_tx));
    let server = tokio::spawn(ws_server::run(addr, input_tx, snapshot_rx));
    tracing::info!(%addr, "ws transport listening");

    tokio::signal::ctrl_c().await?;
    tracing::info!("ctrl-c received, shutting down");
    driver.abort();
    server.abort();
    Ok(())
}
```

## Tests

### `protocol/tests/roundtrip.rs`

For each variant of `ServerMessage`, `ClientMessage`, `PlayerInput`, `WireFormat`: serde-roundtrip via `serde_json::to_string` → `serde_json::from_str`, assert `PartialEq` round-trip. Locks the wire format. Sample fixtures should include `Snapshot`s with 0, 1, and 3 agents.

### `host/tests/ws_smoke.rs`

End-to-end integration test driven by tokio:

1. Construct a `Sim` with three agents, channels, run `sim_driver::run` on a tokio task.
2. Bind a `TcpListener` on `127.0.0.1:0` (ephemeral port), run `ws_server::run` on it.
3. Connect a `tokio_tungstenite` client.
4. Assert receive `Hello { protocol_version: 1, format: Json }`.
5. Send `ClientHello { last_known_tick: None }`.
6. Assert receive `Init` with `current_tick == 0` and three agents.
7. Assert receive ≥ 3 `Snapshot` messages within 500 ms; assert their `tick` is monotonically non-decreasing.
8. Send `PlayerInput::TogglePause`. Capture `tick` value; sleep 200 ms; assert subsequent `Snapshot`s carry the same `tick`.
9. Send `PlayerInput::TogglePause` again; assert ticks resume advancing.

Test uses `tokio::time::pause()` carefully or just real wall-clock with generous tolerances. Real wall-clock keeps the test honest about the actual pacing path.

### Existing tests

`crates/core/tests/{smoke,needs_decay,determinism}.rs` continue to pass unchanged (no behaviour change in `core` beyond adding serde derives).

## Definition of done

- `cargo build --workspace` clean.
- `cargo test --workspace` — all existing tests pass; the new protocol roundtrip and WS smoke tests pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo run -p gecko-sim-host` boots, listens on `127.0.0.1:9001`, and (manual eyeball) `wscat -c ws://127.0.0.1:9001/` shows a `hello` → `init` → stream of `snapshot` messages.
- One atomic `jj` commit. Description placeholder: `WS transport v0: tokio-tungstenite + watch-broadcast snapshot stream + SetSpeed/TogglePause`.

## Trace to ADRs

- **ADR 0008 (time):** `SetSpeed` paces ticks at `1.0 / multiplier` sec wall-clock; pause is `multiplier == 0.0`. Sample loop is independent of tick rate, fixed at 30 Hz, per ADR 0013's decoupling.
- **ADR 0010 (systems):** untouched. Still one system (`needs::decay`) running per tick.
- **ADR 0011 (schema):** wire types are direct serde projections of `core` types; the schema-of-record stays in `core`. `Identity` component remains internal (ECS-only) — the wire view goes through `Snapshot`/`AgentSnapshot`.
- **ADR 0012 (architecture):**
  - `Sim::apply_input` is listed in the public API but kept deferred — see "Deferred items".
  - `core` gains a serde dependency. ADR 0012 anticipates this for wire types via `protocol`; deriving directly on `core` types is the chosen execution (per Q3).
  - `host` owns concurrency; `core` is sync.
- **ADR 0013 (transport):**
  - JSON-over-WS, `Hello` handshake, format-negotiation field, two-tier message taxonomy. `Init`, `Snapshot` land. `Delta`, `PromotedEvent` deferred.
  - 30 Hz sample loop wall-clock, decoupled from tick rate.
  - Coalescing on the server side via `watch::send_replace`.
  - Single-client v0 — multi-client falls out of `watch` for free; left untouched.
  - Reconnect → fresh `Init`. `ClientHello.last_known_tick` parsed but ignored.

## Deferred items (carry forward to later passes)

These are explicitly punted; capturing them here so the next pass-author knows the surface they own.

| Item | Triggers landing | Lives in |
|---|---|---|
| `Sim::apply_input` | First sim-bound `PlayerInput` (`SetPolicy` or `NudgeAgent`) | `core::sim` |
| `Sim::delta_since` + `StreamMessage::Delta` | Sim state grows large enough that snapshot bandwidth bites (ADR 0013 quotes ~150 KB/snapshot @ 1000 agents as the comfort threshold) | `core::sim` + `protocol::messages` |
| `PromotedEvent` stream | Events system (per ADR 0009) | `core::events` + `protocol::messages` |
| Smart resync on reconnect via `last_known_tick` | When delta cache exists | `host::ws_server` + `core::sim` |
| `WireFormat::MessagePack` / `WireFormat::Postcard` | Bandwidth pressure | `protocol::messages` + `host::ws_server` |
| `ts-rs` derives + emission step | Frontend pass | `protocol` build step + `apps/web/src/types/sim/` |
| Auth, TLS, non-loopback bind | Multi-machine deployment (likely never for v0 lifecycle) | `host::config` + `host::ws_server` |
| `RequestSave` / `RequestLoad` / `RequestRestart` | Save pass | `protocol::messages` + `host` (driver-bound) |
| `SetPolicy` / `NudgeAgent` | Macro state lands; inspection UI lands | `protocol::messages` + `core::sim::apply_input` |
| `PlayerInput` sender-attribution field | Multi-client / spectator mode | `protocol::messages` |
| Coalescing-with-priority (Snapshot/Event never dropped) | When `Delta` and `PromotedEvent` both stream — today `watch` coalesces all snapshots, and there are no events | `host::ws_server` send loop |
| `bevy_ecs::Schedule` ceremony in `core::sim::tick` | System #2 lands | `core::sim` |
| World structure + object catalog in `Init` | When `world/` and `object/` get spawned | `protocol::messages::ServerMessage::Init` |

## What this pass enables next

With JSON-over-WS live and a real input control plane, three independent next passes are unblocked:

1. **Frontend scaffold pass** — Next.js + Three.js skeleton in `apps/web/`, `ts-rs` wired into `protocol`, a connected client that renders the agent list and controls speed/pause.
2. **RON content pass** — `content` crate's loaders; `ContentBundle` becomes non-empty; `spawn_test_agent` retires.
3. **Second system pass** — pick another system from ADR 0010 (likely `mood`); introduce `bevy_ecs::Schedule`. Snapshot grows a `mood` field; the wire shape grows trivially because the wire is `core::Snapshot` directly.
