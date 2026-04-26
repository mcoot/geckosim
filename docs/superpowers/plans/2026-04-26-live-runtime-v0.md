# Live runtime v0 implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **VCS:** This repo is jj-tracked (colocated with git). Use `jj`, never raw `git`. Each task ends with `jj desc` (current commit) and a verify step. The next task starts with `jj new` only if `jj st` shows `@` is non-empty.

**Goal:** Stand up a real `bevy_ecs::World` inside `crates/core` with the `Sim` API surface from ADR 0012 (partial — `new`/`tick`/`current_tick`/`snapshot`), one ECS system (needs decay), and a `host` binary that demos the sim ticking deterministically.

**Architecture:** `Sim` wraps a `bevy_ecs::World`, a tick counter, a `PrngState`, and a monotonic `next_agent_id`. `Identity` and `Needs` are the only ECS components this pass; both live in `core/src/agent/mod.rs` (the existing `Needs` schema struct gets a `Component` derive — no parallel type). `systems::needs::decay` mutates `Needs` saturating at 0.0. `Snapshot` is a deterministic `Vec<AgentSnapshot>` sorted by `AgentId`. `Sim::tick` calls the system directly — no `bevy_ecs::Schedule` ceremony until the second system lands. `host/main.rs` boots a `Sim`, spawns Alice/Bob/Charlie, snapshots, ticks 100×, snapshots again, logs both via `tracing::info!`.

**Tech Stack:** Rust 2021 edition, `bevy_ecs` 0.16, `tracing` + `tracing-subscriber`, `rand_pcg`. No new workspace deps — everything is already in `Cargo.toml` from the scaffold pass.

**Spec:** [`docs/superpowers/specs/2026-04-26-live-runtime-v0-design.md`](../specs/2026-04-26-live-runtime-v0-design.md).

**Pre-flight:** Before Task 1, confirm the workspace is clean:

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

All three must succeed. If they don't, fix that before starting.

---

## Task 1: Add `Component` derive + `Needs::full()` to existing `Needs` struct

**Files:**
- Modify: `crates/core/src/agent/mod.rs` (`Needs` struct around line 86)

The existing schema `Needs` becomes both the schema field type and the ECS component. Reuse, not duplication.

- [ ] **Step 1: Start the commit**

```bash
jj st
```

If `@` shows changes, run `jj new`. If it's empty (just the spec commit was the previous task — `@` will be empty here), reuse it. Then describe:

```bash
jj desc -m "Live runtime v0: add Component derive and full() to Needs"
```

- [ ] **Step 2: Write the failing test (inline `#[cfg(test)]` block)**

Append to `crates/core/src/agent/mod.rs`:

```rust
#[cfg(test)]
mod needs_component_tests {
    use super::Needs;
    use bevy_ecs::world::World;

    #[test]
    fn needs_full_is_all_ones() {
        let n = Needs::full();
        assert_eq!(n.hunger, 1.0);
        assert_eq!(n.sleep, 1.0);
        assert_eq!(n.social, 1.0);
        assert_eq!(n.hygiene, 1.0);
        assert_eq!(n.fun, 1.0);
        assert_eq!(n.comfort, 1.0);
    }

    #[test]
    fn needs_can_be_inserted_as_component() {
        let mut world = World::new();
        let entity = world.spawn(Needs::full()).id();
        let needs = world.get::<Needs>(entity).expect("Needs component present");
        assert_eq!(needs.hunger, 1.0);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test -p gecko-sim-core --lib needs_component_tests
```

Expected: compile error — either `Needs::full` not found, or `Needs` does not implement `Component` for `world.spawn(...)`.

- [ ] **Step 4: Add `Component` derive and `Needs::full()`**

In `crates/core/src/agent/mod.rs`, modify the `Needs` derive line and append the impl. Find:

```rust
/// All six need values, each in `[0, 1]`. Per ADR 0011.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub sleep: f32,
    pub social: f32,
    pub hygiene: f32,
    pub fun: f32,
    pub comfort: f32,
}
```

Replace the derive line and add the constructor immediately after the struct:

```rust
/// All six need values, each in `[0, 1]`. Per ADR 0011. Doubles as the
/// ECS component for needs (lazy sharding — schema and component share a
/// type until a future pass needs them to diverge).
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub sleep: f32,
    pub social: f32,
    pub hygiene: f32,
    pub fun: f32,
    pub comfort: f32,
}

impl Needs {
    /// All needs at maximum (`1.0`). Convenience for spawning fresh agents.
    #[must_use]
    pub fn full() -> Self {
        Self {
            hunger: 1.0,
            sleep: 1.0,
            social: 1.0,
            hygiene: 1.0,
            fun: 1.0,
            comfort: 1.0,
        }
    }
}
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-core --lib needs_component_tests
```

Expected: 2 passed.

- [ ] **Step 6: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 7: Verify the commit description is set**

```bash
jj st
```

The description should be `Live runtime v0: add Component derive and full() to Needs`. The change ID will appear above the file diff. No additional command — jj already snapshotted the edits into `@`.

---

## Task 2: Add `Identity` ECS component

**Files:**
- Modify: `crates/core/src/agent/mod.rs` (append `Identity` struct + tests)

`Identity` does not exist as a schema struct (the `Gecko` mono uses inline `id` and `name` fields). Genuinely new.

- [ ] **Step 1: Start the commit**

```bash
jj st
```

If `@` is non-empty (the Task 1 commit is described above), run `jj new`. Then describe:

```bash
jj new
jj desc -m "Live runtime v0: add Identity ECS component"
```

- [ ] **Step 2: Write the failing test**

Append to `crates/core/src/agent/mod.rs`:

```rust
#[cfg(test)]
mod identity_component_tests {
    use super::{Identity, Needs};
    use crate::ids::AgentId;
    use bevy_ecs::world::World;

    #[test]
    fn identity_can_be_inserted_alongside_needs() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Identity {
                    id: AgentId::new(7),
                    name: "Alice".to_string(),
                },
                Needs::full(),
            ))
            .id();
        let id = world.get::<Identity>(entity).expect("Identity component present");
        assert_eq!(id.id, AgentId::new(7));
        assert_eq!(id.name, "Alice");
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test -p gecko-sim-core --lib identity_component_tests
```

Expected: compile error — `Identity` not found.

- [ ] **Step 4: Add the `Identity` struct**

In `crates/core/src/agent/mod.rs`, locate the section header `// Identity / appearance` (around line 19). Immediately after that header comment block (before the existing `Gender` enum), insert:

```rust
/// ECS component holding stable identity for an agent entity.
///
/// Lazy-sharded projection of `Gecko`'s identity fields (`id`, `name`).
/// The `Gecko` schema struct keeps its inline fields; `Identity` is
/// the runtime ECS view used by systems and snapshots.
#[derive(bevy_ecs::component::Component, Debug, Clone)]
pub struct Identity {
    pub id: crate::ids::AgentId,
    pub name: String,
}
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-core --lib identity_component_tests
```

Expected: 1 passed.

- [ ] **Step 6: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 7: Verify the commit**

```bash
jj st
```

Description: `Live runtime v0: add Identity ECS component`.

---

## Task 3: `Sim` skeleton + `ContentBundle` + `TickReport`

**Files:**
- Create: `crates/core/src/sim.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod sim` + re-exports)
- Modify: `crates/core/tests/smoke.rs` (extend with `sim_ticks` test)

`tick()` increments the counter. The needs-decay call gets wired in Task 5. This task is the bare skeleton.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "Live runtime v0: Sim skeleton with ContentBundle and TickReport"
```

- [ ] **Step 2: Write the failing test (extend existing smoke.rs)**

Modify `crates/core/tests/smoke.rs`. Replace the file's contents with:

```rust
//! Smoke test: confirm the headline schema types compile and are reachable
//! from outside the crate via the public surface.

use gecko_sim_core::agent::{Mood, Needs, Personality};
use gecko_sim_core::ids::{AgentId, OwnerRef};
use gecko_sim_core::object::{Predicate, SmartObject};
use gecko_sim_core::{Color, ContentBundle, PrngState, Sim, Tick, TickReport, Vec2};

#[test]
fn ids_construct_and_round_trip() {
    let a = AgentId::new(42);
    assert_eq!(a.raw(), 42);
}

#[test]
fn primitives_construct() {
    let _ = Color::new(255, 128, 0);
    let _ = Vec2::new(1.0, 2.0);
    let _ = Tick::new(0);
    let _ = PrngState::from_seed(0xDEAD_BEEF);
}

#[test]
fn schema_types_are_reachable() {
    let _ = std::mem::size_of::<Needs>();
    let _ = std::mem::size_of::<Personality>();
    let _ = std::mem::size_of::<Mood>();
    let _ = std::mem::size_of::<SmartObject>();
    let _ = std::mem::size_of::<Predicate>();
    let _ = std::mem::size_of::<OwnerRef>();
}

#[test]
fn sim_ticks() {
    let mut sim = Sim::new(0, ContentBundle::default());
    assert_eq!(sim.current_tick(), 0);
    let _: TickReport = sim.tick();
    assert_eq!(sim.current_tick(), 1);
}
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test -p gecko-sim-core --test smoke
```

Expected: compile error — `Sim`, `ContentBundle`, `TickReport` not found in `gecko_sim_core`.

- [ ] **Step 4: Create `crates/core/src/sim.rs`**

```rust
//! Live `Sim` driver: wraps a `bevy_ecs::World` and advances tick state.
//!
//! Honours the public API contract from ADR 0012 partially:
//!   - `new`, `tick`, `current_tick`, `snapshot` (snapshot lands in Task 4).
//!   - `delta_since`, `apply_input` deferred to the WS pass.

use bevy_ecs::world::World;

use crate::rng::PrngState;

/// Catalog data passed into `Sim::new`. Empty placeholder until RON
/// content loading lands; lives in `core` because ADR 0012 fixes the
/// dep direction at `core ← content`.
#[derive(Debug, Clone, Default)]
pub struct ContentBundle;

/// Per-tick stats returned from `Sim::tick`. Empty placeholder; future
/// per-tick counters (decisions made, interrupts raised, promoted events
/// emitted, …) live here.
#[derive(Debug, Clone, Default)]
pub struct TickReport;

/// The live simulation. Owns its `bevy_ecs::World` and the canonical clock.
pub struct Sim {
    world: World,
    tick: u64,
    // Stored for determinism; needs-decay does not consume randomness, but
    // future systems will. The seed is captured here at construction time.
    #[allow(dead_code)]
    rng: PrngState,
    next_agent_id: u64,
}

impl Sim {
    /// Construct a fresh sim with the given world seed and (currently empty)
    /// content bundle.
    #[must_use]
    pub fn new(seed: u64, _content: ContentBundle) -> Self {
        Self {
            world: World::new(),
            tick: 0,
            rng: PrngState::from_seed(seed),
            next_agent_id: 0,
        }
    }

    /// Advance the simulation by one tick (one sim-minute per ADR 0008).
    pub fn tick(&mut self) -> TickReport {
        // Systems land here. Task 5 wires `systems::needs::decay`.
        self.tick += 1;
        TickReport
    }

    /// Current tick count. Starts at 0; each `tick()` increments by 1.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick
    }
}
```

- [ ] **Step 5: Wire up `lib.rs` re-exports**

Modify `crates/core/src/lib.rs`. Replace its contents with:

```rust
//! Gecko-sim core: schema types and the ECS-based simulation engine.

pub mod agent;
pub mod decision;
pub mod events;
pub mod ids;
pub mod macro_;
pub mod object;
pub mod rng;
pub mod save;
pub mod sim;
pub mod systems;
pub mod time;
pub mod world;

// Convenience re-exports of the most-used public types.
pub use ids::{
    AccessoryId, AdvertisementId, AgentId, BuildingId, BusinessId, CrimeIncidentId, EmploymentId,
    HouseholdId, HousingId, LeafAreaId, MemoryEntryId, ObjectId, ObjectTypeId, OwnerRef,
    PromotedEventId,
};
pub use rng::PrngState;
pub use sim::{ContentBundle, Sim, TickReport};
pub use time::Tick;
pub use world::{Color, Vec2};
```

- [ ] **Step 6: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-core --test smoke
```

Expected: 4 passed (3 existing + the new `sim_ticks`).

- [ ] **Step 7: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 8: Verify the commit**

```bash
jj st
```

Description: `Live runtime v0: Sim skeleton with ContentBundle and TickReport`.

---

## Task 4: `Snapshot` + `AgentSnapshot` + `Sim::spawn_test_agent` + `Sim::snapshot`

**Files:**
- Create: `crates/core/src/snapshot.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod snapshot` + re-exports)
- Modify: `crates/core/src/sim.rs` (add `spawn_test_agent`, `snapshot`)
- Create: `crates/core/tests/snapshot.rs`

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "Live runtime v0: Snapshot, spawn_test_agent, and Sim::snapshot"
```

- [ ] **Step 2: Write the failing test**

Create `crates/core/tests/snapshot.rs`:

```rust
//! Integration test: spawning agents and producing a deterministic snapshot.

use gecko_sim_core::ids::AgentId;
use gecko_sim_core::{ContentBundle, Sim};

#[test]
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

    // Sorted by AgentId ascending.
    assert_eq!(snap.agents[0].id, AgentId::new(0));
    assert_eq!(snap.agents[0].name, "Alice");
    assert_eq!(snap.agents[0].needs.hunger, 1.0);

    assert_eq!(snap.agents[1].id, AgentId::new(1));
    assert_eq!(snap.agents[1].name, "Bob");

    assert_eq!(snap.agents[2].id, AgentId::new(2));
    assert_eq!(snap.agents[2].name, "Charlie");
}
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test -p gecko-sim-core --test snapshot
```

Expected: compile error — `spawn_test_agent`, `snapshot`, `Snapshot` not found.

- [ ] **Step 4: Create `crates/core/src/snapshot.rs`**

```rust
//! Snapshot type: a deterministic, by-value view of sim state at a tick.
//!
//! Agents are sorted by `AgentId` ascending so two `Sim` instances built
//! from the same seed and same calls produce byte-equal `Snapshot`s.

use crate::agent::Needs;
use crate::ids::AgentId;

/// Full sim state at a tick boundary. `PartialEq` is required by the
/// determinism test in the test suite.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
}

/// Per-agent snapshot row. Holds the slice of state this pass exposes;
/// other groupings (Personality, Mood, Spatial, …) extend this type as
/// their first consumer system lands.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
}
```

- [ ] **Step 5: Add `pub mod snapshot` and re-exports to `lib.rs`**

Modify `crates/core/src/lib.rs`. Add `pub mod snapshot;` to the module list (alphabetical — between `save` and `sim`). Add the re-export to the convenience block at the bottom:

```rust
pub use snapshot::{AgentSnapshot, Snapshot};
```

The full module list in `lib.rs` should now read:

```rust
pub mod agent;
pub mod decision;
pub mod events;
pub mod ids;
pub mod macro_;
pub mod object;
pub mod rng;
pub mod save;
pub mod sim;
pub mod snapshot;
pub mod systems;
pub mod time;
pub mod world;
```

And the re-export block:

```rust
pub use ids::{
    AccessoryId, AdvertisementId, AgentId, BuildingId, BusinessId, CrimeIncidentId, EmploymentId,
    HouseholdId, HousingId, LeafAreaId, MemoryEntryId, ObjectId, ObjectTypeId, OwnerRef,
    PromotedEventId,
};
pub use rng::PrngState;
pub use sim::{ContentBundle, Sim, TickReport};
pub use snapshot::{AgentSnapshot, Snapshot};
pub use time::Tick;
pub use world::{Color, Vec2};
```

- [ ] **Step 6: Add `spawn_test_agent` and `snapshot` to `Sim`**

Modify `crates/core/src/sim.rs`. Add the imports at the top (under the existing `use` lines):

```rust
use crate::agent::{Identity, Needs};
use crate::ids::AgentId;
use crate::snapshot::{AgentSnapshot, Snapshot};
```

Then add these methods inside `impl Sim` (below `current_tick`):

```rust
    /// Spawn a fresh agent at full needs with a monotonically allocated
    /// `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced when RON content loading lands.
    pub fn spawn_test_agent(&mut self, name: &str) -> AgentId {
        let id = AgentId::new(self.next_agent_id);
        self.next_agent_id += 1;
        self.world.spawn((
            Identity {
                id,
                name: name.to_string(),
            },
            Needs::full(),
        ));
        id
    }

    /// Capture the full sim state at the current tick. Agents are sorted
    /// by `AgentId` ascending for determinism.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let mut agents: Vec<AgentSnapshot> = self
            .world
            .iter_entities()
            .filter_map(|entity_ref| {
                let identity = entity_ref.get::<Identity>()?;
                let needs = entity_ref.get::<Needs>()?;
                Some(AgentSnapshot {
                    id: identity.id,
                    name: identity.name.clone(),
                    needs: *needs,
                })
            })
            .collect();
        agents.sort_by_key(|a| a.id);
        Snapshot {
            tick: self.tick,
            agents,
        }
    }
```

`snapshot` honours ADR 0012's `&self` signature via `World::iter_entities()` (returns `EntityRef`s with `get::<T>() -> Option<&T>` — no `&mut World` needed). If a future bevy_ecs revision changes that API, the fallback is `&mut self` plus a cached `QueryState`; do not switch unless `iter_entities` actually breaks.

- [ ] **Step 7: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-core --test snapshot
```

Expected: 1 passed.

- [ ] **Step 8: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. If clippy complains about the `&mut self` on `snapshot`, do not fix by silencing — leave the doc note in place; it's the chosen tradeoff.

- [ ] **Step 9: Verify the commit**

```bash
jj st
```

Description: `Live runtime v0: Snapshot, spawn_test_agent, and Sim::snapshot`.

---

## Task 5: `systems::needs::decay` + wire into `Sim::tick`

**Files:**
- Create: `crates/core/src/systems/needs.rs`
- Modify: `crates/core/src/systems/mod.rs` (declare submodule)
- Modify: `crates/core/src/sim.rs` (call decay from `tick`)
- Create: `crates/core/tests/needs_decay.rs`

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "Live runtime v0: needs decay system"
```

- [ ] **Step 2: Write the failing integration test**

Create `crates/core/tests/needs_decay.rs`:

```rust
//! Integration test: needs decay over many ticks saturates at zero
//! and decreases other needs by the expected amount.

use gecko_sim_core::systems::needs::{
    COMFORT_DECAY_PER_TICK, FUN_DECAY_PER_TICK, HUNGER_DECAY_PER_TICK, HYGIENE_DECAY_PER_TICK,
    SLEEP_DECAY_PER_TICK, SOCIAL_DECAY_PER_TICK,
};
use gecko_sim_core::{ContentBundle, Sim};

const HUNGER_TICKS: u64 = 480;
const TOL: f32 = 1e-5;

#[test]
fn hunger_saturates_at_zero_after_full_decay_window() {
    let mut sim = Sim::new(0, ContentBundle::default());
    sim.spawn_test_agent("Alice");

    for _ in 0..HUNGER_TICKS {
        sim.tick();
    }

    let snap = sim.snapshot();
    let needs = snap.agents[0].needs;

    // Hunger fully drained; saturating subtraction floors at 0.0.
    assert!(needs.hunger.abs() < TOL, "hunger = {}", needs.hunger);

    // Other needs decreased by exactly N * rate.
    let expected = |rate: f32| 1.0 - HUNGER_TICKS as f32 * rate;
    assert!((needs.sleep   - expected(SLEEP_DECAY_PER_TICK  )).abs() < TOL);
    assert!((needs.social  - expected(SOCIAL_DECAY_PER_TICK )).abs() < TOL);
    assert!((needs.hygiene - expected(HYGIENE_DECAY_PER_TICK)).abs() < TOL);
    assert!((needs.fun     - expected(FUN_DECAY_PER_TICK    )).abs() < TOL);
    assert!((needs.comfort - expected(COMFORT_DECAY_PER_TICK)).abs() < TOL);
}
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test -p gecko-sim-core --test needs_decay
```

Expected: compile error — `gecko_sim_core::systems::needs` does not exist (the module file hasn't been created).

- [ ] **Step 4: Create `crates/core/src/systems/needs.rs`**

```rust
//! ECS system: needs decay. System #1 of 11 from ADR 0010.
//!
//! Each tick (one sim-minute per ADR 0008), every agent's six needs
//! decrement by a per-need rate, saturating at zero. Replenishment is
//! the responsibility of consumer-action systems landing in later
//! passes; this system never raises a need.

use bevy_ecs::world::World;

use crate::agent::Needs;

// Decay rates: each need empties from 1.0 to 0.0 over the listed
// sim-time. Placeholders for v0 — retunable when advertisement
// scoring lands. See ADR 0011 for the schema.
pub const HUNGER_DECAY_PER_TICK: f32 = 1.0 / 480.0; // empties in  8 sim-hours
pub const SLEEP_DECAY_PER_TICK: f32 = 1.0 / 960.0; // empties in 16 sim-hours
pub const SOCIAL_DECAY_PER_TICK: f32 = 1.0 / 720.0; // empties in 12 sim-hours
pub const HYGIENE_DECAY_PER_TICK: f32 = 1.0 / 480.0; // empties in  8 sim-hours
pub const FUN_DECAY_PER_TICK: f32 = 1.0 / 600.0; // empties in 10 sim-hours
pub const COMFORT_DECAY_PER_TICK: f32 = 1.0 / 360.0; // empties in  6 sim-hours

/// Apply one tick of needs decay to every entity with a `Needs` component.
///
/// Saturating subtraction at zero. No upper clamp — replenishment systems
/// are responsible for keeping values in `[0, 1]` from above.
pub(crate) fn decay(world: &mut World) {
    let mut query = world.query::<&mut Needs>();
    for mut needs in query.iter_mut(world) {
        needs.hunger = (needs.hunger - HUNGER_DECAY_PER_TICK).max(0.0);
        needs.sleep = (needs.sleep - SLEEP_DECAY_PER_TICK).max(0.0);
        needs.social = (needs.social - SOCIAL_DECAY_PER_TICK).max(0.0);
        needs.hygiene = (needs.hygiene - HYGIENE_DECAY_PER_TICK).max(0.0);
        needs.fun = (needs.fun - FUN_DECAY_PER_TICK).max(0.0);
        needs.comfort = (needs.comfort - COMFORT_DECAY_PER_TICK).max(0.0);
    }
}
```

`decay` is `pub(crate)` because the only legitimate caller is `Sim::tick` inside this crate. The constants are `pub` so tests (and future tuning code) can reference them.

- [ ] **Step 5: Declare the `needs` submodule**

Modify `crates/core/src/systems/mod.rs`. Replace its contents with:

```rust
//! ECS systems per ADR 0010 / 0012.
//!
//! Each v0 system from ADR 0010 lands as its own submodule:
//!   - `needs`         (1) need decay      ← landed
//!   - `personality`   (2) personality (read-only)
//!   - `mood`          (3) mood update
//!   - `memory`        (4) memory ring & decay
//!   - `relationships` (5) relationship updates
//!   - `skills`        (6) skill gain
//!   - `money`         (7) wages, transactions
//!   - `housing`       (8) residence assignment
//!   - `employment`    (9) job scheduling
//!   - `health`        (10) condition + vitality
//!   - `crime`         (11) crime + consequences
//!
//! Other systems join in later passes alongside additional ECS components.

pub mod needs;
```

- [ ] **Step 6: Wire `decay` into `Sim::tick`**

Modify `crates/core/src/sim.rs`. The current body of `tick` is:

```rust
    pub fn tick(&mut self) -> TickReport {
        // Systems land here. Task 5 wires `systems::needs::decay`.
        self.tick += 1;
        TickReport
    }
```

Replace it with:

```rust
    pub fn tick(&mut self) -> TickReport {
        crate::systems::needs::decay(&mut self.world);
        self.tick += 1;
        TickReport
    }
```

- [ ] **Step 7: Run the new test to verify it passes**

```bash
cargo test -p gecko-sim-core --test needs_decay
```

Expected: 1 passed.

- [ ] **Step 8: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean, including the previous tests still passing.

- [ ] **Step 9: Verify the commit**

```bash
jj st
```

Description: `Live runtime v0: needs decay system`.

---

## Task 6: Determinism integration test

**Files:**
- Create: `crates/core/tests/determinism.rs`

No production-code changes — this test exercises the existing surface and locks in the determinism guarantee.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "Live runtime v0: determinism integration test"
```

- [ ] **Step 2: Write the test**

Create `crates/core/tests/determinism.rs`:

```rust
//! Integration test: same seed + same calls → byte-equal Snapshot.
//!
//! Bakes in the determinism discipline from ADR 0008 so any future
//! source of nondeterminism (HashMap iteration order, Instant::now,
//! unsorted query results, …) is caught immediately.

use gecko_sim_core::{ContentBundle, Sim, Snapshot};

fn run(seed: u64, ticks: u64) -> Snapshot {
    let mut sim = Sim::new(seed, ContentBundle::default());
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");
    for _ in 0..ticks {
        sim.tick();
    }
    sim.snapshot()
}

#[test]
fn same_seed_same_snapshot_after_100_ticks() {
    assert_eq!(run(42, 100), run(42, 100));
}

#[test]
fn same_seed_same_snapshot_at_tick_zero() {
    assert_eq!(run(42, 0), run(42, 0));
}
```

- [ ] **Step 3: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-core --test determinism
```

Expected: 2 passed. (No "fail-first" step here — the production code already supports this; we're locking the contract.)

- [ ] **Step 4: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 5: Verify the commit**

```bash
jj st
```

Description: `Live runtime v0: determinism integration test`.

---

## Task 7: Update `host/src/main.rs` to demo the sim

**Files:**
- Modify: `crates/host/src/main.rs`

`host` has no automated tests this pass per the scaffold spec. Verification is `cargo run -p gecko-sim-host` and an eyeball check on the logs.

- [ ] **Step 1: Start the commit**

```bash
jj new
jj desc -m "Live runtime v0: host demo runs Sim for 100 ticks"
```

- [ ] **Step 2: Replace `crates/host/src/main.rs`**

Current contents:

```rust
use tracing_subscriber::EnvFilter;

// allow: idiomatic extendable-main pattern; body will use `?` as host grows
#[allow(clippy::unnecessary_wraps)]
fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

Replace with:

```rust
use gecko_sim_core::{ContentBundle, Sim};
use tracing_subscriber::EnvFilter;

const DEMO_SEED: u64 = 0xDEAD_BEEF;
const DEMO_TICKS: u64 = 100;

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

The previous `#[allow(clippy::unnecessary_wraps)]` is removed because the body now genuinely needs the `Result` return shape (no early `?`s yet, but the call surface is broader and the allow is no longer accurate enough to justify).

If clippy still flags `unnecessary_wraps`, restore the allow and move on — it's idiomatic for a `main` that will grow.

- [ ] **Step 3: Run the binary**

```bash
cargo run -p gecko-sim-host
```

Expected: two `tracing::info!` lines, one for the initial snapshot (all needs at 1.0), one after 100 ticks. Approximate values after 100 ticks:

| need    | 100 × rate | expected value |
|---------|-----------|----------------|
| hunger  | 0.20833   | ≈ 0.79167      |
| sleep   | 0.10417   | ≈ 0.89583      |
| social  | 0.13889   | ≈ 0.86111      |
| hygiene | 0.20833   | ≈ 0.79167      |
| fun     | 0.16667   | ≈ 0.83333      |
| comfort | 0.27778   | ≈ 0.72222      |

Eyeball: all six values must be visibly less than 1.0; none below 0.0; all three agents (Alice, Bob, Charlie) appear in both snapshots.

Process exit code: 0.

- [ ] **Step 4: Run the full check suite**

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean.

- [ ] **Step 5: Verify the commit**

```bash
jj st
```

Description: `Live runtime v0: host demo runs Sim for 100 ticks`.

---

## Task 8: Final verification + (optional) squash to one commit

**Files:** none modified — verification only.

The spec calls for "one atomic jj commit" as the end state. The chain produced by Tasks 1–7 is seven small commits; folding them into one is optional and non-destructive (jj makes commit history mutable).

- [ ] **Step 1: Run the full DoD check suite**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p gecko-sim-host
```

All four must succeed. Eyeball the host output as in Task 7 Step 3.

- [ ] **Step 2: Inspect the commit chain**

```bash
jj log -r 'trunk()..@' --no-graph -T 'change_id.shortest() ++ "  " ++ description.first_line() ++ "\n"'
```

Expected (top to bottom):

```
<id>  Live runtime v0: host demo runs Sim for 100 ticks
<id>  Live runtime v0: determinism integration test
<id>  Live runtime v0: needs decay system
<id>  Live runtime v0: Snapshot, spawn_test_agent, and Sim::snapshot
<id>  Live runtime v0: Sim skeleton with ContentBundle and TickReport
<id>  Live runtime v0: add Identity ECS component
<id>  Live runtime v0: add Component derive and full() to Needs
<id>  Capture live runtime v0 design spec
<id>  Address scaffold review follow-ups: …
<id>  Scaffold workspace and v0 schema types from 0011/0012
…
```

- [ ] **Step 3: Decide on squashing**

If the project lead (the user) wants one atomic commit per the spec's DoD, squash the seven implementation commits (not the spec commit). Squash from oldest to newest:

```bash
# Move @ to the second-oldest implementation commit (the one above
# "add Component derive and full() to Needs"), then squash repeatedly.
# In practice: easier to use `jj squash --from <child> --into <parent>`
# repeatedly, or to walk @ down with `jj prev -e` and squash up.
```

Recommended sequence (with the change IDs from Step 2's output, in order from oldest implementation commit to newest):

```bash
# Stand at the newest implementation commit:
jj edit <task7-change-id>
# Squash each implementation commit into its parent until only one remains.
# Start with the second-oldest squashing into the oldest:
jj edit <task2-change-id>
jj squash -m "Live runtime v0: bevy_ecs World, Sim API, needs decay, host demo"
# Continue: now Task 2 is folded into Task 1's commit. Repeat for Task 3:
jj edit <task3-now-on-folded-base>
jj squash -m "Live runtime v0: bevy_ecs World, Sim API, needs decay, host demo"
# … and so on through Task 7.
```

If the squash sequence above feels error-prone or you'd prefer to keep the per-task chain for review, **skip squashing**. The commits are already atomic per-task and tell a clean story. The spec's "one commit" is aspirational, not load-bearing.

Default recommendation: **keep the per-task chain.** It tells a better implementation story than a single monolithic commit and lets future readers see the order things landed in. Squashing is an option, not a requirement.

- [ ] **Step 4: Mark the implementation as complete**

```bash
jj log -r 'trunk()..@' --no-graph -T 'description.first_line() ++ "\n"' | head -10
```

Sanity-check the log shows the expected commits in sensible order. The pass is done.

---

## Spec coverage check

| Spec section | Covered by |
|---|---|
| `Sim` struct (fields, signatures) | Task 3 (skeleton) + Task 4 (snapshot/spawn) + Task 5 (tick body) |
| `Identity` + `Needs` components | Task 1 (Needs) + Task 2 (Identity) |
| `systems/needs.rs` constants and `decay` | Task 5 |
| `Snapshot` + `AgentSnapshot` + sort by AgentId | Task 4 |
| `TickReport` empty struct | Task 3 |
| `ContentBundle` empty struct in `core::sim` | Task 3 |
| Module changes per crate | Tasks 1–7 (covered file-by-file) |
| `PrngState::from_seed` | Already exists in scaffold; verified usable in Task 3 |
| `host/src/main.rs` demo | Task 7 |
| Smoke test `sim_ticks` | Task 3 |
| `needs_decay.rs` integration test | Task 5 |
| `determinism.rs` integration test | Task 6 |
| Definition of done (build/test/clippy/run) | Task 8 |

No spec section is unaddressed.
