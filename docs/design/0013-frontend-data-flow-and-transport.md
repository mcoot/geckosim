# 0013 — Frontend data flow and transport

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

0011 fixed the data shapes and 0012 fixed the Rust crate structure. This doc fixes the wire — encoding, message types, snapshot/delta strategy, sample cadence, player input shape, connection lifecycle, and backpressure between the host process and the Next.js / Three.js frontend.

## Decision

### Encoding — JSON for v0

- **JSON over WebSocket.** Human-readable in devtools, trivial to inspect, no extra decoders on the frontend.
- **`ts-rs`-emitted types** (per 0012) ensure end-to-end typing without manual sync.
- A `format` field in the `Hello` handshake lets us swap to **MessagePack** or **postcard-over-WS** later without protocol re-negotiation by version. Schema-aware via serde.

### Two-tier message taxonomy

#### `Init` — sent once at handshake and on reconnect

- `protocol_version`
- World structure: spatial graph (districts, buildings, floors, rooms, outdoor zones, connectivity, dimensions per 0007)
- Content references: object catalog IDs + display info, accessory catalog
- Current full snapshot: all geckos + smart objects + macro state
- `current_tick`

#### `Stream` — sent at sample boundaries

- `Delta`: per-entity-full-state for any changed entity, plus added/removed lists.
- `Snapshot`: periodic full snapshot every **60 ticks (1 sim-hour)** for resync robustness.
- `PromotedEvent` (per 0009): the host maintains a cursor over the sim's promoted-event ring; at each sample boundary it sends events newer than the cursor and advances. From the client's perspective, "as it occurs" is bounded by the 30 Hz sample rate.

### Renderer-facing state (much smaller than sim state)

Per-gecko per-tick:

- `id, current_leaf, position, facing, action_phase, mood`

Per-gecko once (cached client-side, re-sent only on change):

- `appearance, name, age, gender`

Per-smart-object per-tick:

- `id, location, position, visual_state` (e.g. fridge open/closed, cafe serving)

Macro per-tick (for HUD):

- Current weather, active events, current sim-time, key macro variables

The renderer never receives full sim state — needs vectors, memories, relationships, etc. stay server-side and surface only when the client requests an inspection (a future input type, deferred).

### Sample rate — 30 Hz wall-clock, decoupled from sim tick rate

- Server samples sim state at **~30 Hz wall-clock** and emits a `Delta`. Independent of sim tick rate.
- At 1× speed (1 sim-tick / sec), most samples carry trivial deltas — cheap.
- At 64× (64 sim-ticks / sec), each sample spans ~2 sim ticks; the delta reflects net change.
- Renderer interpolates between consecutive samples for visual smoothness (per 0008). Interpolation operates on samples, not raw sim ticks.
- **Known limitation:** at high speeds where multiple sim ticks span a single sample, linear interpolation between samples may produce visible discontinuities (e.g. an agent appearing to teleport across a wall if it changed leaf area within the gap). Acceptable for v0 — at slower speeds the gaps shrink, and high-speed observation is for time-skip rather than scrutiny.

### Player input

```
PlayerInput =
  | SetSpeed { multiplier: f32 }
  | TogglePause
  | SetPolicy { var: PolicyVar, value: PolicyValue }
  | NudgeAgent { agent_id: AgentId, kind: NudgeKind }   // inspect / intervene
  | RequestSave { name: Option<String> }
  | RequestLoad { save_id: SaveId }
  | RequestRestart { seed: Option<u64> }
```

- All inputs **tick-stamped on receipt by the host** (per 0008, for replay determinism).
- Inputs queued; applied at the start of the next sim tick.

### Connection lifecycle

1. Client connects WS to host.
2. Server sends `Hello { protocol_version, capabilities, format }`.
3. Client sends `ClientHello { last_known_tick: Option<u64> }` (none on a fresh connect).
4. Server sends the full `Init` payload.
5. Server begins streaming `Stream` messages at 30 Hz.
6. Client may send `PlayerInput` at any time.
7. **Reconnect:** for v0, the server always responds with a fresh `Init` regardless of `last_known_tick`. Smart delta-cache resync deferred.

### Backpressure & catch-up

- WebSocket has natural backpressure; if the frontend stalls, the server's send buffer fills.
- Server policy: **coalesce.** When the buffer already holds a queued `Delta`, replace it with the latest delta rather than appending. `Snapshot` and `PromotedEvent` are never dropped.
- No client-side acknowledgements at v0. Best-effort streaming; the renderer always shows the latest server-known state.

### Save / load placement

- Save and load travel **over the same WebSocket** as `RequestSave` / `RequestLoad` inputs. Keeps the v0 surface area to one transport.
- Server returns a save acknowledgement (and optionally the save bytes for client-side download).
- Load triggers a fresh `Init` to the connected client.

## Bandwidth estimate (v0 worst case)

- ~1000 geckos × ~150 bytes per gecko (JSON) ≈ **150 KB per delta**.
- 30 Hz sample rate ≈ **4.5 MB/s**.
- Comfortable on local. Acceptable for remote dev. Trigger to swap to MessagePack/postcard when this becomes a real constraint.

## Out of scope for v0 (deferred)

- **Area-of-interest culling.** Server sends all entities regardless of camera. Add later if bandwidth bites.
- **Live content reload** (mid-run RON edits) — restart-required for v0.
- **Smart resync from delta cache** — fresh `Init` on every reconnect.
- **Multi-client / spectator mode** — single-client only. When this lands, `PlayerInput` will need a sender field (global-scope inputs like `RequestSave` / `SetPolicy` currently have no notion of who issued them).
- **Compression** — revisit alongside encoding swap.
- **Auth, TLS, rate limiting** — single-user local sim.
- **Inspection messages** (request full sim-side state for a specific agent) — add when the inspection UI demands it.

## Consequences

- The `protocol` crate (per 0012) defines `InitMessage`, `StreamMessage`, `PlayerInput`, `Hello`, `ClientHello`, plus the per-entity wire types. All `#[derive(TS, Serialize, Deserialize)]`.
- The `host` crate runs the WebSocket server and the 30 Hz sample loop, decoupled from the sim's tick loop.
- The frontend type tree is auto-generated by `ts-rs` into `apps/web/src/types/sim/`.
- Renderer state management holds the most recent two `Stream` payloads for interpolation; full `Snapshot` payloads replace local state outright.

## Open questions

- **Concrete WebSocket library** (`tokio-tungstenite`, `axum`'s WS, etc.) — implementation choice for `host`.
- **Promoted-event UI surfacing** — toast, news ticker, log panel? Frontend concern, not architectural.
- **Inspection message shape** — when we add the "click a gecko to see its needs/memories" UI, what does that request/response look like? Defer.
- **Save artifact storage** — where saves live (server filesystem? client download? both?) — operational, not architectural.
