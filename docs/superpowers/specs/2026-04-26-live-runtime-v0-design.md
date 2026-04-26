# Live runtime v0 — minimal "it ticks" pass

- **Date:** 2026-04-26
- **Status:** Approved
- **Scope:** Second implementation pass on the gecko sim. Stands up a `bevy_ecs::World` inside `crates/core`, defines the `Sim` API surface from ADR 0012 (partially), and runs one system end-to-end.
- **Predecessor:** [`2026-04-26-scaffold-workspace-v0-design.md`](2026-04-26-scaffold-workspace-v0-design.md) — the workspace + 0011 schema types are assumed to be in place.

## Goal

End state: `cargo run -p gecko-sim-host` boots a `Sim`, spawns three test agents, snapshots, ticks 100×, snapshots again, logs initial vs final needs via `tracing::info!`, and exits 0. The sim is real (`bevy_ecs::World`-backed, deterministic, with a real system mutating real components) — not a stub.

## Non-goals

- No RON content loading. `ContentBundle` exists as an empty struct to honour ADR 0012's `Sim::new` signature; real loading lands in a later pass.
- No WS server, wire types, or `protocol`-crate code. The `protocol` crate is untouched.
- No frontend work. `apps/web/` placeholder is untouched.
- No additional systems beyond needs decay. The other ten systems from ADR 0010 land in later passes.
- No additional ECS components beyond `Identity` and `Needs`. Other groupings (Personality, Mood, Spatial, Inventory, Memory, Relationships, decision state, …) get sharded into components when their first consumer system lands.
- No `delta_since`, `apply_input`, `Delta`, or `PlayerInput`. They are omitted entirely (not stubbed) and arrive with the WS pass.
- No `bevy_ecs::Schedule` ceremony. With one system, a direct call from `Sim::tick` is enough; schedule plumbing lands with system #2.
- No per-agent RNG sub-streams. Needs decay is deterministic without RNG; one main `PrngState` on `Sim` is enough for now. The per-agent sub-stream pattern from ADR 0008 lands with the first RNG-consuming system.
- No CI workflow. No `rust-toolchain.toml`.

## Architecture

### `Sim` struct

Lives in `crates/core/src/sim.rs`, re-exported from `lib.rs`.

```rust
pub struct Sim {
    world: bevy_ecs::world::World,
    tick: u64,
    rng: PrngState,
    next_agent_id: u64,
}

impl Sim {
    pub fn new(seed: u64, _content: ContentBundle) -> Self;
    pub fn tick(&mut self) -> TickReport;
    pub fn current_tick(&self) -> u64;
    pub fn snapshot(&self) -> Snapshot;
    pub fn spawn_test_agent(&mut self, name: &str) -> AgentId;
}
```

`spawn_test_agent` is doc-marked `# Note` as a placeholder until content loading lands. The signature is intentionally unstable.

`ContentBundle` is consumed by name only this pass — `Sim::new` ignores its (empty) contents. Keeping the parameter honours ADR 0012's API and avoids a breaking signature change later.

`new` initialises `world: World::new()`, `tick: 0`, `rng: PrngState::from_seed(seed)`, `next_agent_id: 0`. (`PrngState::from_seed` is a small constructor added in this pass over `rand_pcg::Pcg64Mcg::seed_from_u64`.)

`tick` runs `systems::needs::decay(&mut self.world)`, increments `self.tick`, returns `TickReport::default()`.

`current_tick` returns `self.tick`.

`snapshot` queries `(&Identity, &Needs)` from the world and assembles `Snapshot`.

`spawn_test_agent` allocates an `AgentId` from `next_agent_id`, increments the counter, and `world.spawn((Identity { id, name: name.into() }, Needs::full()))`.

### Components

In `crates/core/src/agent/mod.rs`, alongside the existing schema types:

```rust
#[derive(bevy_ecs::component::Component, Debug, Clone)]
pub struct Identity {
    pub id: AgentId,
    pub name: String,
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct Needs {
    pub hunger: f32,
    pub sleep: f32,
    pub social: f32,
    pub hygiene: f32,
    pub fun: f32,
    pub comfort: f32,
}

impl Needs {
    pub fn full() -> Self { /* all 1.0 */ }
}
```

Each `f32 ∈ [0, 1]`. 1.0 = fully satisfied, 0.0 = critical. The existing `Gecko` struct from the schema is not deleted — it stays as the schema-of-record. The components are independent projections of the slice of state this pass uses.

### `systems/needs.rs`

```rust
pub const HUNGER_DECAY_PER_TICK:  f32 = 1.0 / 480.0;  // empties in  8h
pub const SLEEP_DECAY_PER_TICK:   f32 = 1.0 / 960.0;  // empties in 16h
pub const SOCIAL_DECAY_PER_TICK:  f32 = 1.0 / 720.0;  // empties in 12h
pub const HYGIENE_DECAY_PER_TICK: f32 = 1.0 / 480.0;  // empties in  8h
pub const FUN_DECAY_PER_TICK:     f32 = 1.0 / 600.0;  // empties in 10h
pub const COMFORT_DECAY_PER_TICK: f32 = 1.0 / 360.0;  // empties in  6h

pub(crate) fn decay(world: &mut bevy_ecs::world::World) {
    let mut q = world.query::<&mut Needs>();
    for mut n in q.iter_mut(world) {
        n.hunger  = (n.hunger  - HUNGER_DECAY_PER_TICK ).max(0.0);
        n.sleep   = (n.sleep   - SLEEP_DECAY_PER_TICK  ).max(0.0);
        n.social  = (n.social  - SOCIAL_DECAY_PER_TICK ).max(0.0);
        n.hygiene = (n.hygiene - HYGIENE_DECAY_PER_TICK).max(0.0);
        n.fun     = (n.fun     - FUN_DECAY_PER_TICK    ).max(0.0);
        n.comfort = (n.comfort - COMFORT_DECAY_PER_TICK).max(0.0);
    }
}
```

Saturating-at-zero subtraction. No upper clamp here — replenishment systems will be responsible for that. Constants are `pub` so tests can reference them.

These rates are placeholders. They are retunable once advertisement scoring exists; the doc comments note the target empty-time, and that's the contract the values uphold.

### `Snapshot` type

In a new `crates/core/src/snapshot.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
}
```

`PartialEq` is required for the determinism test. `Serialize`/`Deserialize` are not added this pass — `Snapshot` only crosses crates inside the workspace; wire serialisation belongs to the WS pass.

`Snapshot::agents` ordering: deterministic via sort by `id` ascending inside `Sim::snapshot`. Agents land in the world via `spawn_test_agent` in caller-determined order, but `bevy_ecs` query iteration order is not contractually stable across versions; an explicit sort by stable `AgentId` removes that variable.

### `TickReport` type

In `crates/core/src/sim.rs`:

```rust
#[derive(Debug, Clone, Default)]
pub struct TickReport;
```

Empty placeholder. Future per-tick stats (counts of decisions made, interrupts raised, promoted events emitted, …) live here.

### `ContentBundle` type

In `crates/core/src/sim.rs` alongside `Sim`:

```rust
#[derive(Debug, Clone, Default)]
pub struct ContentBundle;
```

Empty placeholder. Lives in `core` because ADR 0012 fixes the dep direction at `core ← content`; placing `ContentBundle` in `gecko-sim-content` would force `core` to depend on `content` to honour `Sim::new(seed, ContentBundle)`. Future RON loaders in `gecko-sim-content` will return `core::ContentBundle` (or extend it) — content depends on core, not the reverse.

## Module changes by crate

### `gecko-sim-core`
- **New:** `src/sim.rs` (module + `Sim` + `TickReport` + `ContentBundle`); `src/snapshot.rs`; `src/systems/needs.rs`.
- **Modified:** `src/lib.rs` re-exports `Sim`, `TickReport`, `ContentBundle`, `Snapshot`, `AgentSnapshot`. `src/agent/mod.rs` adds `Identity` component and a `Component` derive on the existing `Needs` struct, plus a `Needs::full()` constructor. `src/systems/mod.rs` declares the `needs` submodule.
- **Already in place from scaffold pass:** `PrngState::from_seed(u64) -> Self` already exists in `src/rng/mod.rs` — no change needed.
- **Untouched:** `ids.rs`, `world/mod.rs`, `object/mod.rs`, `decision/mod.rs`, `macro_/mod.rs`, `events/mod.rs`, `time/mod.rs`, `save/mod.rs`.

### `gecko-sim-content`
- **Untouched.** Real loaders land in a later pass and will return `core::ContentBundle`.

### `gecko-sim-protocol`
- **Untouched.**

### `gecko-sim-host`
- **Modified:** `src/main.rs` constructs `Sim`, spawns three agents, captures pre/post snapshots, logs them.

## `host/src/main.rs`

```rust
use gecko_sim_core::{ContentBundle, Sim};
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));

    let mut sim = Sim::new(0xDEAD_BEEF, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");

    let initial = sim.snapshot();
    tracing::info!(?initial, "initial snapshot");

    for _ in 0..100 {
        sim.tick();
    }

    let after = sim.snapshot();
    tracing::info!(?after, "snapshot after 100 ticks");

    Ok(())
}
```

`tracing` Debug-formats the `Snapshot` directly. Both snapshots in one log line each is fine for v0; structured field logging arrives with the WS pass.

## Tests

All three integration tests live in `crates/core/tests/`. Each is a single `#[test]` function constructing a `Sim` directly.

### `smoke.rs` (extends existing)

```rust
use gecko_sim_core::ids::AgentId;
use gecko_sim_core::{ContentBundle, Sim};

#[test]
fn ids_construct() {
    let _ = AgentId(0);
}

#[test]
fn sim_ticks() {
    let mut sim = Sim::new(0, ContentBundle::default());
    assert_eq!(sim.current_tick(), 0);
    sim.tick();
    assert_eq!(sim.current_tick(), 1);
}
```

### `needs_decay.rs`

Spawns one agent at full needs, ticks 480 times (the hunger empty-duration), asserts:
- `hunger ≈ 0.0` within `1e-5` (saturated cleanly).
- Other needs decreased by `480 * rate` within `1e-5` of their expected value.

Tolerance accounts for `f32` accumulation error over 480 subtractions.

### `determinism.rs`

```rust
use gecko_sim_core::{ContentBundle, Sim, Snapshot};

fn run(seed: u64, ticks: u64) -> Snapshot {
    let mut sim = Sim::new(seed, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");
    for _ in 0..ticks { sim.tick(); }
    sim.snapshot()
}

#[test]
fn same_seed_same_snapshot() {
    assert_eq!(run(42, 100), run(42, 100));
}
```

Bakes in determinism discipline early, even though needs decay alone is trivially deterministic. The test is cheap and will catch any future regression that introduces nondeterminism (e.g. iterating `HashMap`s in user-visible order, calling `Instant::now`, etc.).

## Definition of done

- `cargo build --workspace` clean.
- `cargo test --workspace` — all three integration tests pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo run -p gecko-sim-host` logs initial and post-100-tick snapshots and exits 0. Manual eyeball: hunger has decreased by `100/480 ≈ 0.208`; other needs by their respective amounts; nothing went negative.
- One atomic `jj` commit. Description TBD when the implementation plan is written; placeholder: `Live runtime v0: bevy_ecs World, Sim API, needs decay, host demo`.

## Trace to ADRs

- **ADR 0008 (time):** `Sim::tick` advances `current_tick` by 1; `current_tick` is the canonical clock. Per-agent RNG sub-streams deferred — needs decay does not consume randomness.
- **ADR 0010 (systems):** "needs decay" is system #1 of 11. The other ten land in later passes.
- **ADR 0011 (schema):** `Identity` + `Needs` components are projections of the corresponding slice of `Gecko`. The schema-of-record `Gecko` struct stays in place; ECS sharding is lazy.
- **ADR 0012 (architecture):** `Sim::new`/`tick`/`current_tick`/`snapshot` honoured. `delta_since`/`apply_input` omitted (not stubbed) until the WS pass. Module layout matches verbatim. `core` owns determinism; `host` only logs.
- **ADR 0013 (transport):** untouched this pass.

## What this pass enables next

With a live `Sim` ticking deterministically and one system in place, the next passes have a clear order of attack:

1. **WS scaffold pass** — `protocol` crate's wire types, snapshot/delta serialisation (`Snapshot` gains `Serialize`), a barebones WS server in `host`, frontend still deferred. `delta_since` and `apply_input` land here.
2. **RON content pass** — `content` crate gains real loaders; `ContentBundle` becomes non-empty; `spawn_test_agent` is replaced by content-driven agent generation.
3. **Second system pass** — picks one more system from ADR 0010 (likely mood, since it depends only on needs that already exist) and introduces a real `bevy_ecs::Schedule`.

These three are independent and can be sequenced however the work prioritises.
