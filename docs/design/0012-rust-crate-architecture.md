# 0012 — Rust crate architecture

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

0011 fixed the data shapes; this doc fixes how those shapes live in code — ECS choice, crate layout, hosting model, threading, type generation, save format, and tooling. It is the bridge from schema to scaffolding.

## Decision

### ECS — `bevy_ecs` standalone

Use the **`bevy_ecs` crate**, not the full Bevy engine.

- Mature, well-documented, fast archetypal storage.
- Naturally provides SOA: the monolithic `Gecko` from 0011 shards into separate components (`Needs`, `Personality`, `Mood`, …), each in dense per-archetype arrays — gives cache-friendly hot loops without hand-rolling.
- Heterogeneous smart objects (fridges have `FoodCount`; chairs don't) are exactly what ECS shines at.
- Standalone — pulls in storage and query plumbing, not rendering / windowing / input.

Alternatives considered: `hecs` (lighter but smaller ecosystem), hand-rolled SOA + arenas (tighter, much more upfront cost). Defer either to a measured perf need.

### Crate layout — 4-crate workspace

```
gecko-sim/                        (workspace root)
├── Cargo.toml                    workspace + shared lints
├── rustfmt.toml
├── crates/
│   ├── core/                     sim engine — no I/O, no net, no UI
│   ├── content/                  RON loaders, content schemas, validation
│   ├── protocol/                 wire types shared with frontend (TS auto-gen)
│   └── host/                     binary: loads content, runs sim, serves WS
├── content/                      RON files (object catalog, accessory catalog, …)
└── apps/
    └── web/                      Next.js / Three.js frontend (separate from Rust workspace)
```

- **`core`** has zero I/O dependencies — keeps it WASM-compilable later (per 0002 future) and trivially testable.
- **`content`** depends on `core` (uses its types) but not vice versa.
- **`protocol`** depends on `core` for type re-exports; emits TS bindings.
- **`host`** depends on all three; brings I/O, threading, WS server.

### Module organization within `core`

```
core/src/
├── lib.rs           pub Sim + public API
├── world/           spatial graph (district/building/floor/room/zone), pathfinding
├── agent/           Gecko ECS components and helpers
├── object/          SmartObject components + ObjectType catalog handle
├── decision/        utility AI scoring, action commitment, interrupts
├── systems/         one submodule per 0010 system (needs, mood, memory, relationships, …)
├── macro_/          macro state + macro tick logic per 0009
├── events/          promoted event channel
├── rng/             seeded sub-stream management
├── time/            clock, tick scheduler
└── save/            serde → SaveData
```

### Identity surfaces

The sim has three identity surfaces and one mapping pattern:

- **Stable IDs** (`AgentId`, `ObjectId`, `BuildingId`, `LeafAreaId`, `HousingId`, `EmploymentId`, `HouseholdId`, `BusinessId`, `CrimeIncidentId`, `MemoryEntryId`, `AccessoryId`, `PromotedEventId`). `u64` newtypes per 0011. **Canonical** — used in saves, the wire protocol (per 0013), and all cross-references between sim entities.
- **`bevy_ecs::Entity` handles.** Opaque to consumers; allocated by the ECS at entity creation. **Never serialized.** Used only inside `core` for queries.
- **Wire IDs** (per 0013). The same stable IDs above, serialized directly. No translation layer between the protocol and the sim.

`core` maintains bidirectional `StableId ↔ Entity` maps as ECS resources. On save, only the stable IDs are written. On load, stable IDs are read first, fresh `Entity` handles are allocated by the ECS, and the maps are rebuilt before any system runs.

The public `Sim` API methods (`snapshot`, `delta_since`, `apply_input`) speak in stable IDs only.

### Hosting model — native binary + WebSocket for v0

- **v0:** native `host` binary; Next.js frontend connects via WebSocket. Frontend hot-reloads independently of the sim.
- **Future:** compile `core` to WASM for browser-only deployment. Architecture supports it; not designed around it.

Implications:

- `core` does not depend on `std::net`, async runtimes, or OS-thread APIs directly.
- `host` owns concurrency, the WS server, wall-clock pacing.

### Threading — single-threaded sim for v0

- Sim runs single-threaded — determinism (per 0008) is much easier without parallelism-induced ordering nondeterminism.
- `host` has additional threads for I/O and the WS server, but they communicate with sim only via:
  - **Input queues** drained at the start of each tick.
  - **Snapshot reads** taken at tick boundaries (sim is paused for the read).
- `bevy_ecs` supports single-threaded schedulers.
- Profile-driven parallelism can be added later for specific systems with deterministic merge ordering.

### Sim core public API

```rust
pub struct Sim { /* private */ }

impl Sim {
    pub fn new(seed: u64, content: ContentBundle) -> Self;
    pub fn from_save(save: SaveData, content: ContentBundle) -> Self;

    pub fn tick(&mut self) -> TickReport;            // advance one micro tick
    pub fn current_tick(&self) -> u64;

    pub fn snapshot(&self) -> Snapshot;              // full state at current tick
    pub fn delta_since(&self, last_tick: u64) -> Delta;

    pub fn apply_input(&mut self, input: PlayerInput);  // queued for next tick
    pub fn save(&self) -> SaveData;
}
```

Pure synchronous API. Host owns wall-clock pacing, concurrency, and any speed multipliers from 0008 (achieved by calling `tick()` more frequently).

### TS type generation — `ts-rs`

- `protocol` crate annotates wire types with `#[derive(TS)]`.
- A build step (e.g. `cargo test -p protocol --features export-ts` or a dedicated bin) emits `.ts` files into the frontend tree (target path TBD in 0013).
- Frontend consumes typed `Snapshot`, `Delta`, `PlayerInput`, etc. — no manual sync.

Alternative considered: `specta`. Defer to a future revisit if `specta`'s ergonomics improve materially.

### Save format

- **Production: `postcard`.** Compact, fast, `no_std`-friendly (helps the WASM future), schema-aware via serde.
- **Debug: JSON.** `serde_json` dump for inspection, content authoring sanity-checks, and bug reports.
- Every save carries a **schema version** prefix; the loader can refuse incompatible saves or run migrations.

### Workspace tooling

- Cargo workspace with shared `[workspace.lints]` — deny `warnings` and a curated clippy set.
- `rustfmt.toml` at the root.
- CI runs:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -D warnings`
  - `cargo test --workspace`
- **`tracing`** for structured logging; spans double as profiling hooks.
- **Errors:** `thiserror` in `core` / `content` / `protocol` (typed errors); `anyhow` at host boundaries (where typing buys nothing).

## Consequences

- Scaffolding work for v0 is roughly: workspace skeleton → `core` types from 0011 → `content` loader for the smart-object catalog → `host` with a stub WS server → minimal frontend connection. 0013 fills in the wire protocol.
- Adding a new system from 0010 means a new submodule under `systems/` plus components under `agent/` (or `object/`), a registration in the schedule, and any RON content needed.
- Compiling to WASM later requires `host` to grow a WASM-compatible variant or be replaced by a thin browser shell; `core` and `content` should already work.
- Determinism is owned by `core`. Anything in `host` that touches sim state at non-tick-boundary cadence is a bug.

## Open questions

- **WS framing & message types** — defined in 0013.
- **Profiling tool choice** (`puffin`, `tracy`, etc.) — pick when first needed.
- **Specific Bevy ECS version pin** — implementation detail; track in `Cargo.toml`.
- **Save migration strategy** beyond version refusal — write only when first save-incompatible change ships.
- **Test strategy specifics** — unit tests inside each module; integration tests at the `Sim` API; deterministic replay tests as a category. Concrete patterns established when scaffolding lands.
