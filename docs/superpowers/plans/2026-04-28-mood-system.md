# Mood system v0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land system #2 (`mood`, system #3 of 11 from ADR 0010), introduce the `bevy_ecs::Schedule` ceremony in `Sim`, propagate `mood` to the wire (snapshot + ts-rs regen + roundtrip fixtures), and render Valence/Arousal/Stress columns in the frontend's agent list.

**Architecture:** `Mood` becomes a `bevy_ecs::Component` (lazy-shard pattern, like `Needs`). `Sim` gains a `bevy_ecs::schedule::Schedule` field built once at construction; `Sim::tick` calls `schedule.run(&mut world)`. Both `systems::needs::decay` and the new `systems::mood::update` are refactored to idiomatic `Query<&mut T>` parameter functions. `mood::update` reads `(&Needs, &mut Mood)` and drifts each component toward a needs-derived target with `α = 0.01` per-tick inertia. `AgentSnapshot` grows a `mood` field; ts-rs regenerates the bindings; the frontend table grows three columns.

**Tech Stack:** Rust 2021, `bevy_ecs 0.16` (Schedule, Query, Component derives), `ts-rs 10` (already wired with `export-ts` feature), Next.js 16 + React 19 + Tailwind v4 + Vitest 2.

**Reference:** Spec at [`docs/superpowers/specs/2026-04-28-mood-system-design.md`](../specs/2026-04-28-mood-system-design.md). ADR 0010 (systems inventory), ADR 0011 (schema), ADR 0012 (architecture).

**VCS note:** This repo uses `jj` (jujutsu), colocated with git. **Never use raw `git`.** Each task starts with `jj new -m "<task title>"`; jj automatically snapshots edits as you work. There is no separate "commit" command.

---

## File Structure

**New files:**
- `crates/core/src/systems/mood.rs` — Task 3 (system + unit tests + drift constant)
- `crates/core/tests/mood.rs` — Task 4 (integration test through `Sim`)

**Modified files:**
- `crates/core/src/agent/mod.rs` — Task 1 (`Mood` Component derive + `neutral()` constructor)
- `crates/core/src/systems/mod.rs` — Task 3 (`pub mod mood;`)
- `crates/core/src/systems/needs.rs` — Task 2 (refactor `decay(&mut World)` → `decay(Query<&mut Needs>)`)
- `crates/core/src/sim.rs` — Tasks 2, 4, 5 (Schedule field, register both systems, `Mood::neutral()` on spawn, snapshot reads Mood)
- `crates/core/src/snapshot.rs` — Task 5 (`AgentSnapshot.mood`)
- `crates/protocol/tests/roundtrip.rs` — Task 5 (fixture grows `mood`)
- `apps/web/src/types/sim/Mood.ts` — Task 5 (auto-regen)
- `apps/web/src/types/sim/AgentSnapshot.ts` — Task 5 (auto-regen)
- `apps/web/src/components/AgentList.tsx` — Task 6 (3 new columns)
- `apps/web/src/lib/sim/reducer.test.ts` — Task 6 (fixture grows `mood`; one new assertion)

**Existing tests untouched (work as-is despite refactors):**
- `crates/core/tests/{snapshot,determinism,needs_decay,catalogs}.rs` — all use the `Sim` public API; no shape literals on `AgentSnapshot` so the field addition doesn't bite.
- `crates/host/tests/ws_smoke.rs` — asserts on tick advancement and agent count, not field values.

---

## Task 1: `Mood` Component derive + `neutral()` constructor

**Files:**
- Modify: `crates/core/src/agent/mod.rs`

Pure type addition. `Mood` already exists as a schema struct; this task adds the `bevy_ecs::component::Component` derive (lazy-shard pattern) and the `ts-rs` `cfg_attr` derives so `Mood.ts` will be generated when Task 5 runs the export. Verification: `cargo build` clean both with and without `--features export-ts`.

- [ ] **Step 1.1: Start the task commit**

```bash
jj new -m "Mood: ECS Component derive + Mood::neutral() constructor"
```

- [ ] **Step 1.2: Add the derives and constructor**

In `crates/core/src/agent/mod.rs`, find the `Mood` struct (currently around line 184). It looks like:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mood {
    pub valence: f32,
    pub arousal: f32,
    pub stress: f32,
}
```

Replace with:

```rust
/// Short-term emotional state per ADR 0011 (3-dimensional vector).
/// Doubles as the ECS component (lazy sharding — schema and component
/// share a type until a future pass needs them to diverge).
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize,
)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Mood {
    /// In `[-1, 1]`. Negative = unhappy, positive = happy.
    pub valence: f32,
    /// In `[0, 1]`. 0 = calm, 1 = alert / excited.
    pub arousal: f32,
    /// In `[0, 1]`. 0 = none, 1 = max stress.
    pub stress: f32,
}

impl Mood {
    /// Neutral mood: all components at zero. Used when spawning new
    /// agents before any needs-derived target has had a chance to drift.
    #[must_use]
    pub fn neutral() -> Self {
        Self {
            valence: 0.0,
            arousal: 0.0,
            stress: 0.0,
        }
    }
}
```

- [ ] **Step 1.3: Verify default-features build still clean**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: both clean. The new `Component` derive is purely additive; no consumers fire yet.

- [ ] **Step 1.4: Verify `--features export-ts` build clean**

```bash
cargo build -p gecko-sim-core --features export-ts
```

Expected: clean.

- [ ] **Step 1.5: Verify clippy clean for both configurations**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p gecko-sim-core --all-targets --features export-ts -- -D warnings
```

Expected: both clean.

- [ ] **Step 1.6: Confirm commit scope**

```bash
jj st
```

Expected: 1 file modified — `crates/core/src/agent/mod.rs`.

---

## Task 2: `bevy_ecs::Schedule` in `Sim`; refactor `needs::decay` to `Query` parameter

**Files:**
- Modify: `crates/core/src/sim.rs`
- Modify: `crates/core/src/systems/needs.rs`

The previous `Sim::tick` called `systems::needs::decay(&mut world)` directly. After this task, `Sim` owns a `Schedule` that has just `needs::decay` registered, and `tick` runs the schedule. `mood::update` joins in Task 4.

The refactor of `needs::decay` from `&mut World` to `Query<&mut Needs>` lets bevy inject the query and enables `Schedule` registration.

Verification gate: every existing Rust test still passes after the refactor (no behavior change). `tests/needs_decay.rs`, `tests/determinism.rs`, `tests/snapshot.rs` all go through `Sim::tick` which now goes through the schedule.

- [ ] **Step 2.1: Start the task commit**

```bash
jj new -m "Mood: introduce bevy_ecs::Schedule in Sim; refactor needs::decay to Query parameter"
```

- [ ] **Step 2.2: Refactor `crates/core/src/systems/needs.rs`**

Replace the existing `decay` function (currently `pub(crate) fn decay(world: &mut World)`) with the `Query`-parameter form:

```rust
//! ECS system: needs decay. System #1 of 11 from ADR 0010.
//!
//! Each tick (one sim-minute per ADR 0008), every agent's six needs
//! decrement by a per-need rate, saturating at zero. Replenishment is
//! the responsibility of consumer-action systems landing in later
//! passes; this system never raises a need.

use bevy_ecs::system::Query;

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
pub(crate) fn decay(mut needs: Query<&mut Needs>) {
    for mut n in needs.iter_mut() {
        n.hunger = (n.hunger - HUNGER_DECAY_PER_TICK).max(0.0);
        n.sleep = (n.sleep - SLEEP_DECAY_PER_TICK).max(0.0);
        n.social = (n.social - SOCIAL_DECAY_PER_TICK).max(0.0);
        n.hygiene = (n.hygiene - HYGIENE_DECAY_PER_TICK).max(0.0);
        n.fun = (n.fun - FUN_DECAY_PER_TICK).max(0.0);
        n.comfort = (n.comfort - COMFORT_DECAY_PER_TICK).max(0.0);
    }
}
```

The body is identical to the old version — only the function signature changed (and the unused `World` import is gone).

- [ ] **Step 2.3: Modify `crates/core/src/sim.rs` to own the Schedule**

Replace the file contents with:

```rust
//! Live `Sim` driver: wraps a `bevy_ecs::World` and advances tick state
//! via a `bevy_ecs::schedule::Schedule`.
//!
//! Honours the public API contract from ADR 0012 partially:
//!   - `new`, `tick`, `current_tick`, `snapshot`.
//!   - `delta_since`, `apply_input` deferred to a later pass.

use std::collections::HashMap;

use bevy_ecs::schedule::Schedule;
use bevy_ecs::world::World;

use crate::agent::{Accessory, AccessoryCatalog, Identity, Needs};
use crate::ids::{AccessoryId, AgentId, ObjectTypeId};
use crate::object::{ObjectCatalog, ObjectType};
use crate::rng::PrngState;
use crate::snapshot::{AgentSnapshot, Snapshot};
use crate::systems;

/// Catalog data passed into `Sim::new`. Loaded from RON files by the
/// `gecko-sim-content` crate; populated maps after a real load, empty maps
/// after `ContentBundle::default()`.
#[derive(Debug, Clone, Default)]
pub struct ContentBundle {
    pub object_types: HashMap<ObjectTypeId, ObjectType>,
    pub accessories: HashMap<AccessoryId, Accessory>,
}

/// Per-tick stats returned from `Sim::tick`. Empty placeholder; future
/// per-tick counters (decisions made, interrupts raised, promoted events
/// emitted, …) live here.
#[derive(Debug, Clone, Default)]
pub struct TickReport;

/// The live simulation. Owns its `bevy_ecs::World`, a `Schedule` of
/// per-tick systems, and the canonical clock.
pub struct Sim {
    world: World,
    schedule: Schedule,
    tick: u64,
    // Stored for determinism; needs-decay does not consume randomness, but
    // future systems will. The seed is captured here at construction time.
    #[expect(dead_code, reason = "consumed by first RNG-using system in a later pass")]
    rng: PrngState,
    next_agent_id: u64,
}

impl Sim {
    /// Construct a fresh sim with the given world seed and content bundle.
    /// Builds the per-tick `Schedule` once with all v0 systems in their
    /// canonical order; `tick` runs the schedule unchanged each call.
    #[must_use]
    pub fn new(seed: u64, content: ContentBundle) -> Self {
        let mut world = World::new();
        world.insert_resource(ObjectCatalog {
            by_id: content.object_types,
        });
        world.insert_resource(AccessoryCatalog {
            by_id: content.accessories,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(systems::needs::decay);

        Self {
            world,
            schedule,
            tick: 0,
            rng: PrngState::from_seed(seed),
            next_agent_id: 0,
        }
    }

    /// Advance the simulation by one tick (one sim-minute per ADR 0008).
    /// Runs the per-tick `Schedule` against the world.
    pub fn tick(&mut self) -> TickReport {
        self.schedule.run(&mut self.world);
        self.tick += 1;
        TickReport
    }

    /// Current tick count. Starts at 0; each `tick()` increments by 1.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    /// Borrow the loaded object-type catalog. Mirror of the
    /// `Res<ObjectCatalog>` view that systems will use.
    #[must_use]
    pub fn object_catalog(&self) -> &ObjectCatalog {
        self.world
            .get_resource::<ObjectCatalog>()
            .expect("ObjectCatalog resource is inserted in Sim::new")
    }

    /// Borrow the loaded accessory catalog. Mirror of the
    /// `Res<AccessoryCatalog>` view that systems will use.
    #[must_use]
    pub fn accessory_catalog(&self) -> &AccessoryCatalog {
        self.world
            .get_resource::<AccessoryCatalog>()
            .expect("AccessoryCatalog resource is inserted in Sim::new")
    }

    /// Spawn a fresh agent at full needs with a monotonically allocated
    /// `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced in a future pass.
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
}
```

Key changes from the previous file:
- New imports: `bevy_ecs::schedule::Schedule`, `crate::systems`.
- `Sim` gains a `schedule: Schedule` field.
- `Sim::new` builds the schedule once with `systems::needs::decay` registered.
- `Sim::tick` calls `self.schedule.run(&mut self.world)` instead of the direct function.
- All other methods unchanged.

(The `Mood::neutral()` and `mood::update` registration land in Task 4. The `snapshot` reading `Mood` lands in Task 5.)

- [ ] **Step 2.4: Verify default-features build still clean**

```bash
cargo build --workspace
```

Expected: clean. If you hit "trait `IntoSystemConfigs` not in scope" or "method `add_systems` not found", the bevy_ecs 0.16 trait import differs from `Schedule::add_systems`'s implicit one. Try adding `use bevy_ecs::schedule::IntoSystemConfigs;` (older bevy) or `use bevy_ecs::schedule::IntoScheduleConfigs;` (newer bevy) at the top of `sim.rs`. The `bevy_ecs::prelude::*` glob is also a safe fallback.

- [ ] **Step 2.5: Run all existing tests — they must still pass**

```bash
cargo test --workspace
```

Expected: all tests pass (the refactor is behavior-preserving). The `tests/needs_decay.rs::hunger_saturates_at_zero_after_full_decay_window` test is the most direct check — it exercises 480 ticks of decay through `sim.tick()` and asserts the saturation, so any regression in needs::decay's logic surfaces here.

- [ ] **Step 2.6: Verify clippy clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 2.7: Confirm commit scope**

```bash
jj st
```

Expected: 2 files modified — `crates/core/src/sim.rs` and `crates/core/src/systems/needs.rs`.

---

## Task 3: `systems::mood::update` + unit tests

**Files:**
- Modify: `crates/core/src/systems/mod.rs`
- Create: `crates/core/src/systems/mood.rs`

This task introduces the mood system body and unit-tests it directly using a hand-built `World` + single-system `Schedule`. The integration test through `Sim` (which requires `mood::update` to be registered in `Sim`'s schedule) lands in Task 4.

- [ ] **Step 3.1: Start the task commit**

```bash
jj new -m "Mood: systems::mood::update + unit tests"
```

- [ ] **Step 3.2: Add the module declaration in `crates/core/src/systems/mod.rs`**

Find the existing line `pub mod needs;` (around line 17). Append:

```rust
pub mod mood;
```

Update the docstring's status comment block from `(3) mood update` line to mark it landed:

```rust
//!   - `needs`         (1) need decay      ← landed
//!   - `personality`   (2) personality (read-only)
//!   - `mood`          (3) mood update     ← landed
//!   - `memory`        (4) memory ring & decay
```

(Match the existing `← landed` style on the needs line.)

- [ ] **Step 3.3: Write the failing unit tests in `crates/core/src/systems/mood.rs`**

Create the file with **only** the `#[cfg(test)] mod tests` block (no implementation yet) so the test module fails to compile against the not-yet-written `update` function and constants:

```rust
//! ECS system: mood update. System #3 of 11 from ADR 0010.
//!
//! Each tick (one sim-minute per ADR 0008), every agent's mood drifts
//! toward a target derived from the agent's current `Needs` with a
//! small inertia. Pure deterministic function on `Needs` (no RNG, no
//! events). See ADR 0011 for the `Mood` schema and ADR 0010 for the
//! cross-system coupling intent.

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::agent::{Mood, Needs};
    use crate::systems::mood::{update, MOOD_DRIFT_RATE_PER_TICK};

    /// Build a single-entity world with the given (Needs, Mood) and a
    /// schedule whose only system is `mood::update`. Run one tick and
    /// return the resulting Mood.
    fn run_one_tick(needs: Needs, mood: Mood) -> Mood {
        let mut world = World::new();
        let entity = world.spawn((needs, mood)).id();
        let mut schedule = Schedule::default();
        schedule.add_systems(update);
        schedule.run(&mut world);
        *world.get::<Mood>(entity).expect("Mood component present")
    }

    #[test]
    fn full_needs_drifts_valence_positive_from_neutral() {
        // mean_need = 1.0 → valence_target = 1.0
        // After 1 tick from valence=0: valence ≈ MOOD_DRIFT_RATE_PER_TICK
        let mood = run_one_tick(Needs::full(), Mood::neutral());
        assert!(
            (mood.valence - MOOD_DRIFT_RATE_PER_TICK).abs() < 1e-6,
            "valence={}",
            mood.valence
        );
        // arousal_target = 0; mood was already 0 → still 0.
        assert!(mood.arousal.abs() < 1e-6, "arousal={}", mood.arousal);
        // stress_target = 0; still 0.
        assert!(mood.stress.abs() < 1e-6, "stress={}", mood.stress);
    }

    #[test]
    fn empty_needs_drifts_valence_negative_arousal_and_stress_up() {
        // mean_need = 0.0 → valence_target = -1.0
        // arousal_target = 1.0
        // min_need = 0.0 → stress_target = 1.0
        let needs = Needs {
            hunger: 0.0,
            sleep: 0.0,
            social: 0.0,
            hygiene: 0.0,
            fun: 0.0,
            comfort: 0.0,
        };
        let mood = run_one_tick(needs, Mood::neutral());
        assert!(
            (mood.valence + MOOD_DRIFT_RATE_PER_TICK).abs() < 1e-6,
            "valence={}",
            mood.valence
        );
        assert!(
            (mood.arousal - MOOD_DRIFT_RATE_PER_TICK).abs() < 1e-6,
            "arousal={}",
            mood.arousal
        );
        assert!(
            (mood.stress - MOOD_DRIFT_RATE_PER_TICK).abs() < 1e-6,
            "stress={}",
            mood.stress
        );
    }

    #[test]
    fn worst_need_above_threshold_yields_zero_stress_target() {
        // min_need = 0.6 (above 0.5) → stress_target = 0
        // mood.stress was 0 → still 0 after one tick.
        let needs = Needs {
            hunger: 0.6,
            sleep: 0.7,
            social: 0.8,
            hygiene: 0.9,
            fun: 0.6,
            comfort: 0.7,
        };
        let mood = run_one_tick(needs, Mood::neutral());
        assert!(mood.stress.abs() < 1e-6, "stress={}", mood.stress);
    }

    #[test]
    fn worst_need_below_threshold_drives_stress_up() {
        // min_need = 0.2 → stress_target = ((0.5 - 0.2) * 2).clamp = 0.6
        // After one tick: stress ≈ 0.6 * α
        let needs = Needs {
            hunger: 0.2,
            sleep: 0.7,
            social: 0.8,
            hygiene: 0.9,
            fun: 0.6,
            comfort: 0.7,
        };
        let mood = run_one_tick(needs, Mood::neutral());
        let expected = 0.6 * MOOD_DRIFT_RATE_PER_TICK;
        assert!(
            (mood.stress - expected).abs() < 1e-6,
            "stress={} expected={}",
            mood.stress,
            expected
        );
    }

    #[test]
    fn mood_saturates_toward_target_after_many_ticks() {
        // Empty needs → targets (-1, 1, 1). After 1000 ticks at α=0.01,
        // mood reaches > 99% of target (1 - (1-α)^1000 ≈ 0.99996).
        let mut world = World::new();
        let entity = world
            .spawn((
                Needs {
                    hunger: 0.0,
                    sleep: 0.0,
                    social: 0.0,
                    hygiene: 0.0,
                    fun: 0.0,
                    comfort: 0.0,
                },
                Mood::neutral(),
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(update);
        for _ in 0..1000 {
            schedule.run(&mut world);
        }
        let mood = *world.get::<Mood>(entity).expect("Mood present");
        assert!(mood.valence < -0.99, "valence={}", mood.valence);
        assert!(mood.arousal > 0.99, "arousal={}", mood.arousal);
        assert!(mood.stress > 0.99, "stress={}", mood.stress);
    }
}
```

- [ ] **Step 3.4: Run the failing test**

```bash
cargo test -p gecko-sim-core systems::mood::tests
```

Expected: compile error — `update` and `MOOD_DRIFT_RATE_PER_TICK` are not defined.

- [ ] **Step 3.5: Implement `mood::update` and the constant**

Add to the top of `crates/core/src/systems/mood.rs` (above the `#[cfg(test)] mod tests` block):

```rust
use bevy_ecs::system::Query;

use crate::agent::{Mood, Needs};

/// Mood drifts toward its needs-derived target by this fraction each
/// tick. `α = 0.01` means mood reaches ~63% of target in 100 ticks
/// (≈ 1.67 sim-hours), saturates within ~500 ticks. Tunable.
pub const MOOD_DRIFT_RATE_PER_TICK: f32 = 0.01;

/// Stress target activates when the worst need drops below this floor.
/// Below the floor, `stress_target` rises linearly to 1.0 at need=0.
const STRESS_NEED_FLOOR: f32 = 0.5;

/// Apply one tick of mood drift to every agent. Reads the current
/// `Needs` value, computes a target mood vector, and shifts the
/// agent's `Mood` toward the target by `MOOD_DRIFT_RATE_PER_TICK`.
/// Clamps each component to its declared range.
pub(crate) fn update(mut q: Query<(&Needs, &mut Mood)>) {
    for (needs, mut mood) in q.iter_mut() {
        let mean_need = mean(needs);
        let min_need = min(needs);

        let valence_target = 2.0 * mean_need - 1.0;
        let arousal_target = (1.0 - mean_need).clamp(0.0, 1.0);
        let stress_target = ((STRESS_NEED_FLOOR - min_need) * 2.0).clamp(0.0, 1.0);

        mood.valence = (mood.valence
            + (valence_target - mood.valence) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(-1.0, 1.0);
        mood.arousal = (mood.arousal
            + (arousal_target - mood.arousal) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(0.0, 1.0);
        mood.stress = (mood.stress
            + (stress_target - mood.stress) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(0.0, 1.0);
    }
}

fn mean(n: &Needs) -> f32 {
    (n.hunger + n.sleep + n.social + n.hygiene + n.fun + n.comfort) / 6.0
}

fn min(n: &Needs) -> f32 {
    n.hunger
        .min(n.sleep)
        .min(n.social)
        .min(n.hygiene)
        .min(n.fun)
        .min(n.comfort)
}
```

- [ ] **Step 3.6: Run the unit tests to verify pass**

```bash
cargo test -p gecko-sim-core systems::mood::tests
```

Expected: all 5 tests pass.

- [ ] **Step 3.7: Verify the workspace still builds clean**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. The `mood::update` function is defined but not yet registered in any `Schedule` outside the unit tests, so `Sim` behavior is unchanged.

- [ ] **Step 3.8: Confirm commit scope**

```bash
jj st
```

Expected: 1 modified (`crates/core/src/systems/mod.rs`) + 1 new (`crates/core/src/systems/mood.rs`).

---

## Task 4: Register `mood::update` in schedule; spawn agents with `Mood::neutral()`; integration test

**Files:**
- Modify: `crates/core/src/sim.rs`
- Create: `crates/core/tests/mood.rs`

This task wires `mood::update` into `Sim`'s schedule (chained after `needs::decay`), updates `spawn_test_agent` to attach `Mood::neutral()` so newly-spawned agents have the component, and adds the integration test.

The integration test needs to drop an agent's needs to zero — provide a small test helper `spawn_test_agent_with_needs` so the integration test can seed empty needs without manipulating the world directly.

- [ ] **Step 4.1: Start the task commit**

```bash
jj new -m "Mood: register mood::update in schedule; spawn_test_agent attaches Mood::neutral; integration test"
```

- [ ] **Step 4.2: Write the failing integration test**

Create `crates/core/tests/mood.rs`:

```rust
//! Integration test: mood drifts toward needs-derived target through
//! `Sim::tick`. Confirms `mood::update` is registered in the schedule
//! and that the wire-shape change in `AgentSnapshot` (Task 5) doesn't
//! break the path. Note: this test runs even before Task 5, since it
//! reads `mood` via the entity-component path through `Sim::snapshot`.

use gecko_sim_core::{ContentBundle, Needs, Sim};

#[test]
fn empty_needs_drives_mood_toward_target_through_sim_tick() {
    let mut sim = Sim::new(0, ContentBundle::default());
    sim.spawn_test_agent_with_needs(
        "Hungry",
        Needs {
            hunger: 0.0,
            sleep: 0.0,
            social: 0.0,
            hygiene: 0.0,
            fun: 0.0,
            comfort: 0.0,
        },
    );

    for _ in 0..500 {
        sim.tick();
    }

    let snap = sim.snapshot();
    let agent = &snap.agents[0];

    // After 500 ticks at α=0.01 against targets (-1, 1, 1) from neutral,
    // mood reaches roughly (1 - 0.99^500) ≈ 99.3% of target.
    assert!(agent.mood.valence < -0.5, "valence={}", agent.mood.valence);
    assert!(agent.mood.arousal > 0.5, "arousal={}", agent.mood.arousal);
    assert!(agent.mood.stress > 0.5, "stress={}", agent.mood.stress);
}
```

This test references:
- `gecko_sim_core::Needs` (already pub-re-exported via `core::lib::pub use agent::Needs` — verify in step 4.3 if not).
- `Sim::spawn_test_agent_with_needs(name, needs)` — new helper, added in this task.
- `agent.mood` — the `AgentSnapshot.mood` field, which lands in Task 5. Until Task 5, **this assertion path won't compile**, but Task 5 is the next commit. Defer the integration-test compile gate to Task 5; for Task 4, run only the unit tests via `cargo test -p gecko-sim-core --lib` or accept that `cargo test --workspace` will fail to compile this file.

**Important:** to keep Task 4's commit green, comment out the `agent.mood` assertions and `agent` derefs in this test, with a `// TODO(Task 5): re-enable mood assertions when AgentSnapshot.mood lands` marker. Task 5 uncomments them. **Or:** keep them as written and accept that `cargo test --workspace` fails until Task 5 — and run only `cargo test -p gecko-sim-core --lib` plus the existing test files in Task 4. The simpler path is the comment-out option.

For this plan, use the comment-out approach. Replace the four `assert!(agent.mood...)` lines with:

```rust
    // TODO(Task 5): re-enable mood assertions once AgentSnapshot.mood lands.
    // After 500 ticks at α=0.01 against targets (-1, 1, 1) from neutral,
    // mood reaches roughly (1 - 0.99^500) ≈ 99.3% of target.
    let _ = agent;
    // assert!(agent.mood.valence < -0.5, "valence={}", agent.mood.valence);
    // assert!(agent.mood.arousal > 0.5, "arousal={}", agent.mood.arousal);
    // assert!(agent.mood.stress > 0.5, "stress={}", agent.mood.stress);
```

The `let _ = agent;` keeps the binding used so clippy doesn't complain about an unused variable. The function still exercises `spawn_test_agent_with_needs` + 500 ticks + snapshot, validating that the integration path compiles and runs.

- [ ] **Step 4.3: Verify `Needs` is accessible from the test path**

```bash
grep 'pub use agent::Needs' crates/core/src/lib.rs
```

If the line exists, skip to step 4.4. If not, add `Needs` to the `pub use agent::{...}` block in `crates/core/src/lib.rs`. As of plan-time, `Needs` is exported via `pub use agent::Needs;` already (verify before editing).

If not exported, add it. Use the import `gecko_sim_core::agent::Needs` instead inside the integration test if simpler.

- [ ] **Step 4.4: Run the failing test**

```bash
cargo test -p gecko-sim-core --test mood
```

Expected: compile error — `Sim::spawn_test_agent_with_needs` does not exist.

- [ ] **Step 4.5: Add the test helper and register `mood::update` in `crates/core/src/sim.rs`**

Two edits:

**Edit A.** In the `Sim::new` body, change the schedule construction from:

```rust
let mut schedule = Schedule::default();
schedule.add_systems(systems::needs::decay);
```

to:

```rust
let mut schedule = Schedule::default();
schedule.add_systems((systems::needs::decay, systems::mood::update).chain());
```

The `chain()` call enforces strict sequential ordering: `mood::update` runs after `needs::decay` in every tick.

If `chain()` is not in scope, add `use bevy_ecs::schedule::IntoSystemConfigs;` to the imports at the top of `sim.rs`. (Newer bevy versions: try `IntoScheduleConfigs` instead. Or use `bevy_ecs::prelude::*` to glob the relevant traits.)

**Edit B.** Update `spawn_test_agent` to attach `Mood::neutral()`, and add a new `spawn_test_agent_with_needs` method below it. Replace the existing method block:

```rust
    /// Spawn a fresh agent at full needs with a monotonically allocated
    /// `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced in a future pass.
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
```

with:

```rust
    /// Spawn a fresh agent at full needs and neutral mood with a
    /// monotonically allocated `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced in a future pass.
    pub fn spawn_test_agent(&mut self, name: &str) -> AgentId {
        self.spawn_test_agent_with_needs(name, Needs::full())
    }

    /// Spawn a fresh agent with explicit initial needs and neutral mood.
    /// Test-only entry point used by the mood integration test to seed
    /// empty needs without poking the ECS world directly.
    pub fn spawn_test_agent_with_needs(&mut self, name: &str, needs: Needs) -> AgentId {
        let id = AgentId::new(self.next_agent_id);
        self.next_agent_id += 1;
        self.world.spawn((
            Identity {
                id,
                name: name.to_string(),
            },
            needs,
            Mood::neutral(),
        ));
        id
    }
```

Also add `Mood` to the existing `crate::agent` import at the top of `sim.rs`. The import line currently reads:

```rust
use crate::agent::{Accessory, AccessoryCatalog, Identity, Needs};
```

Change to:

```rust
use crate::agent::{Accessory, AccessoryCatalog, Identity, Mood, Needs};
```

- [ ] **Step 4.6: Run the integration test**

```bash
cargo test -p gecko-sim-core --test mood
```

Expected: passes. The test exercises `spawn_test_agent_with_needs` + 500 `sim.tick()` + `sim.snapshot()`, all without crashing. The `agent.mood` assertions remain commented; Task 5 re-enables them.

- [ ] **Step 4.7: Run all existing tests — they must still pass**

```bash
cargo test --workspace
```

Expected: all clean.

- [ ] **Step 4.8: Verify clippy clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 4.9: Confirm commit scope**

```bash
jj st
```

Expected: 1 modified (`crates/core/src/sim.rs`) + 1 new (`crates/core/tests/mood.rs`).

---

## Task 5: `AgentSnapshot` grows `mood`; ts-rs regen; protocol roundtrip fixtures

**Files:**
- Modify: `crates/core/src/snapshot.rs`
- Modify: `crates/core/src/sim.rs` (snapshot fn reads Mood)
- Modify: `crates/protocol/tests/roundtrip.rs` (fixtures grow mood)
- Modify: `crates/core/tests/mood.rs` (uncomment the assertions)
- Modify (regen): `apps/web/src/types/sim/AgentSnapshot.ts`
- Create (regen): `apps/web/src/types/sim/Mood.ts`

This task carries the schema change to the wire. After this task the integration test from Task 4 runs to its full assertion set, ts-rs has emitted `Mood.ts` and an updated `AgentSnapshot.ts`, and `protocol/tests/roundtrip.rs` continues to lock the JSON shape.

- [ ] **Step 5.1: Start the task commit**

```bash
jj new -m "Mood: AgentSnapshot grows mood; ts-rs regen; protocol roundtrip fixtures updated"
```

- [ ] **Step 5.2: Add `mood` to `AgentSnapshot` in `crates/core/src/snapshot.rs`**

Find the existing struct:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
}
```

Replace with:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
    pub mood: Mood,
}
```

Add `Mood` to the existing import at the top of `snapshot.rs`. The current line is:

```rust
use crate::agent::Needs;
```

Change to:

```rust
use crate::agent::{Mood, Needs};
```

- [ ] **Step 5.3: Update `Sim::snapshot` in `crates/core/src/sim.rs` to read `Mood`**

Find the snapshot method's filter_map and grow it:

```rust
            .filter_map(|entity_ref| {
                let identity = entity_ref.get::<Identity>()?;
                let needs = entity_ref.get::<Needs>()?;
                let mood = entity_ref.get::<Mood>()?;
                Some(AgentSnapshot {
                    id: identity.id,
                    name: identity.name.clone(),
                    needs: *needs,
                    mood: *mood,
                })
            })
```

(Just adds the `let mood = ...` line and the `mood: *mood,` field to the struct literal.)

- [ ] **Step 5.4: Update `crates/protocol/tests/roundtrip.rs` fixture**

Find the `sample_snapshot_with_agents` function:

```rust
fn sample_snapshot_with_agents(count: usize) -> Snapshot {
    let names = ["Alice", "Bob", "Carol", "Dave", "Eve"];
    let agents = (0..count)
        .map(|i| AgentSnapshot {
            id: AgentId::new(i as u64),
            name: names.get(i).copied().unwrap_or("Agent").to_string(),
            needs: Needs::full(),
        })
        .collect();
    Snapshot { tick: 7, agents }
}
```

Replace with:

```rust
fn sample_snapshot_with_agents(count: usize) -> Snapshot {
    let names = ["Alice", "Bob", "Carol", "Dave", "Eve"];
    let agents = (0..count)
        .map(|i| AgentSnapshot {
            id: AgentId::new(i as u64),
            name: names.get(i).copied().unwrap_or("Agent").to_string(),
            needs: Needs::full(),
            mood: Mood::neutral(),
        })
        .collect();
    Snapshot { tick: 7, agents }
}
```

Add `Mood` to the existing imports at the top of the file. The current line is:

```rust
use gecko_sim_core::agent::Needs;
```

Change to:

```rust
use gecko_sim_core::agent::{Mood, Needs};
```

- [ ] **Step 5.5: Re-enable the integration-test assertions in `crates/core/tests/mood.rs`**

Find the commented-out assertions block:

```rust
    // TODO(Task 5): re-enable mood assertions once AgentSnapshot.mood lands.
    // After 500 ticks at α=0.01 against targets (-1, 1, 1) from neutral,
    // mood reaches roughly (1 - 0.99^500) ≈ 99.3% of target.
    let _ = agent;
    // assert!(agent.mood.valence < -0.5, "valence={}", agent.mood.valence);
    // assert!(agent.mood.arousal > 0.5, "arousal={}", agent.mood.arousal);
    // assert!(agent.mood.stress > 0.5, "stress={}", agent.mood.stress);
```

Replace with:

```rust
    // After 500 ticks at α=0.01 against targets (-1, 1, 1) from neutral,
    // mood reaches roughly (1 - 0.99^500) ≈ 99.3% of target.
    assert!(agent.mood.valence < -0.5, "valence={}", agent.mood.valence);
    assert!(agent.mood.arousal > 0.5, "arousal={}", agent.mood.arousal);
    assert!(agent.mood.stress > 0.5, "stress={}", agent.mood.stress);
```

- [ ] **Step 5.6: Run all Rust tests**

```bash
cargo test --workspace
```

Expected: all green. The integration test now exercises the full `agent.mood` assertion path; the protocol roundtrip continues to pass with the new field.

- [ ] **Step 5.7: Regenerate the ts-rs bindings**

```bash
cargo test -p gecko-sim-protocol --features export-ts
```

Expected: passes; writes `apps/web/src/types/sim/Mood.ts` (new) and updates `apps/web/src/types/sim/AgentSnapshot.ts` to include the new field.

- [ ] **Step 5.8: Verify the typed bindings parse**

```bash
cd apps/web && pnpm tsc --noEmit
```

Expected: clean. (The frontend reducer test will still pass too — its fixture has needs but not mood; the next test compile is in Task 6 when we update the fixture and `<AgentList>`.)

Wait — actually the fixture in `reducer.test.ts` builds `AgentSnapshot` literals and the new `mood` field is required by the type. So `pnpm tsc` will fail here. **The Task 6 changes to `reducer.test.ts` are required for this gate to be clean.**

If `pnpm tsc --noEmit` reports an error like `Property 'mood' is missing in type '{ id: 0; name: "Alice"; needs: ...; }' but required in type 'AgentSnapshot'`, that confirms the type propagated correctly. Skip the rest of step 5.8 — Task 6 will make it green.

If you want this task's commit to be green on its own, also apply the Task 6 fixture-update step inline here (move it from Task 6 step 6.4 into Task 5). But the cleaner shape is to accept that Task 5's pnpm gate is "regenerated bindings parse but the existing fixture doesn't yet match" and rely on Task 6 to close the loop.

For this plan, **commit Task 5 with the failing pnpm tsc**, knowing that Task 6 will resolve it. This mirrors the Task 4 → Task 5 pattern (Task 4 has commented-out assertions that Task 5 re-enables). Only Rust tests gate Task 5; frontend gates wait for Task 6.

- [ ] **Step 5.9: Verify Rust workspace still clean**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both clean.

- [ ] **Step 5.10: Confirm commit scope**

```bash
jj st
```

Expected: 4 modified (`crates/core/src/{snapshot,sim}.rs`, `crates/core/tests/mood.rs`, `crates/protocol/tests/roundtrip.rs`) + 1 modified (`apps/web/src/types/sim/AgentSnapshot.ts`) + 1 new (`apps/web/src/types/sim/Mood.ts`).

---

## Task 6: Frontend `<AgentList>` grows three columns; reducer test fixture updated

**Files:**
- Modify: `apps/web/src/components/AgentList.tsx`
- Modify: `apps/web/src/lib/sim/reducer.test.ts`

After Task 5, the frontend type system already knows `AgentSnapshot.mood` exists, but `reducer.test.ts`'s `fixtureSnapshot` builds an `AgentSnapshot` literal without `mood`, so `pnpm tsc` is currently failing. This task closes that loop and adds the three table columns.

- [ ] **Step 6.1: Start the task commit**

```bash
jj new -m "Mood: frontend AgentList grows valence/arousal/stress columns; reducer test fixture updated"
```

- [ ] **Step 6.2: Update the reducer test fixture in `apps/web/src/lib/sim/reducer.test.ts`**

Find the existing helper:

```ts
const fixtureSnapshot = (tick: number): Snapshot => ({
  tick,
  agents: [
    {
      id: 0,
      name: "Alice",
      needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
    },
  ],
});
```

Replace with:

```ts
const fixtureSnapshot = (tick: number): Snapshot => ({
  tick,
  agents: [
    {
      id: 0,
      name: "Alice",
      needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
      mood: { valence: 0, arousal: 0, stress: 0 },
    },
  ],
});
```

- [ ] **Step 6.3: Add a new test asserting mood round-trips through the reducer**

Append to the bottom of the `describe("reduce", () => {` block in `reducer.test.ts`, just before the closing `});`:

```ts
  it("init message preserves the mood field on the snapshot", () => {
    const snap: Snapshot = {
      tick: 10,
      agents: [
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: -0.5, arousal: 0.7, stress: 0.3 },
        },
      ],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 10, snapshot: snap },
    });
    expect(next.snapshot?.agents[0].mood).toEqual({
      valence: -0.5,
      arousal: 0.7,
      stress: 0.3,
    });
  });
```

- [ ] **Step 6.4: Run the reducer tests**

```bash
cd apps/web && pnpm test
```

Expected: 6 tests pass (the original 5 + the new one).

- [ ] **Step 6.5: Update `apps/web/src/components/AgentList.tsx`**

Replace the file contents with:

```tsx
"use client";

import { useSimConnection } from "@/lib/sim/connection";

const NEED_KEYS = ["hunger", "sleep", "social", "hygiene", "fun", "comfort"] as const;
const MOOD_KEYS = ["valence", "arousal", "stress"] as const;

export function AgentList() {
  const { state } = useSimConnection();
  const snapshot = state.snapshot;

  if (!snapshot) {
    return <p className="text-sm text-neutral-500">No data yet.</p>;
  }
  if (snapshot.agents.length === 0) {
    return <p className="text-sm text-neutral-500">No agents.</p>;
  }

  return (
    <table className="w-full border-collapse text-sm">
      <thead>
        <tr className="border-b border-neutral-300 text-left dark:border-neutral-700">
          <th className="px-2 py-1">ID</th>
          <th className="px-2 py-1">Name</th>
          {NEED_KEYS.map((k) => (
            <th key={k} className="px-2 py-1 capitalize">
              {k}
            </th>
          ))}
          {MOOD_KEYS.map((k) => (
            <th key={k} className="px-2 py-1 capitalize text-neutral-500">
              {k}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {snapshot.agents.map((agent) => (
          <tr
            key={agent.id}
            className="border-b border-neutral-200 last:border-0 dark:border-neutral-800"
          >
            <td className="px-2 py-1 font-mono">{agent.id}</td>
            <td className="px-2 py-1">{agent.name}</td>
            {NEED_KEYS.map((k) => (
              <td key={k} className="px-2 py-1 font-mono">
                {agent.needs[k].toFixed(2)}
              </td>
            ))}
            {MOOD_KEYS.map((k) => (
              <td key={k} className="px-2 py-1 font-mono text-neutral-500">
                {agent.mood[k].toFixed(2)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
```

- [ ] **Step 6.6: Run the full frontend gate**

```bash
cd apps/web && pnpm tsc --noEmit
cd apps/web && pnpm lint
cd apps/web && pnpm test
cd apps/web && pnpm build
```

Expected: all clean. `pnpm build` reports the `/` route prerendered as static.

- [ ] **Step 6.7: Manual end-to-end smoke**

In one terminal: `cargo run -p gecko-sim-host`. In another: `cd apps/web && pnpm dev`. Open http://localhost:3000.

For an autonomous-run verification (no real browser available), launch both as background processes and curl the index:

```bash
cargo run -p gecko-sim-host > /tmp/host.log 2>&1 &
HOST_PID=$!
sleep 2
( cd apps/web && pnpm dev > /tmp/web.log 2>&1 ) &
WEB_PID=$!
sleep 8
curl -s http://localhost:3000 | grep -E "valence|arousal|stress|gecko-sim" | head -10
kill $WEB_PID $HOST_PID 2>/dev/null
wait 2>/dev/null
```

Expected: `curl` output includes `Valence`, `Arousal`, `Stress` table headers in the SSR HTML.

For a real browser smoke (preferred when running interactively):
1. Confirm three new columns appear after Name/Comfort.
2. After ~1-2 sim-minutes (3-5 seconds at default 1× speed), watch valence drift slightly negative and arousal slightly positive as needs decay.
3. Click `64×` — drift accelerates visibly within ~10 seconds.
4. Stop both processes.

- [ ] **Step 6.8: Confirm commit scope**

```bash
jj st
```

Expected: 2 files modified — `apps/web/src/components/AgentList.tsx` and `apps/web/src/lib/sim/reducer.test.ts`. No other changes.

---

## Definition of done (rolled-up gate)

After Task 6 lands:

- `cargo build --workspace` clean.
- `cargo test --workspace` clean — all existing tests pass; new `mood` unit tests (5 cases) and integration test (1 case) pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test -p gecko-sim-protocol --features export-ts` regenerates types; re-running produces zero diff.
- `cd apps/web && pnpm install --frozen-lockfile && pnpm test && pnpm tsc --noEmit && pnpm lint && pnpm build` — all clean.
- Manual smoke: 3 new columns visible; mood values drift over time; `64×` accelerates the drift.
- 6 atomic jj commits matching the commit-strategy section of the spec.

## Notes for the implementer

- **bevy_ecs 0.16 trait imports.** `add_systems((a, b).chain())` needs the trait that exposes `chain()` for system tuples. In bevy_ecs 0.16 this is `IntoSystemConfigs` (older) or `IntoScheduleConfigs` (newer). If neither is in scope, try `use bevy_ecs::prelude::*;` at the top of `sim.rs`. Cargo's "method `chain` not found" error will direct you to the correct trait.
- **Task 4 commit is intentionally green-with-commented-assertions.** The integration test in `tests/mood.rs` exercises `spawn_test_agent_with_needs` + `Sim::tick` end to end, but the `agent.mood` assertions are commented until Task 5 grows `AgentSnapshot.mood`. Don't try to short-circuit by squashing Task 4 and Task 5 — the schedule/spawn/integration commit is logically distinct from the wire-shape commit.
- **Task 5 commit has a transiently-failing `pnpm tsc`.** That's by design — Task 6 fixes the fixture and adds the column. If you want every commit's full gate green (including frontend), interleave Task 5 step 5.5 (uncomment assertions) with Task 6's fixture update. The split as written keeps the Rust-side wire-shape commit and frontend column commit cleanly separable.
- **`Sim::spawn_test_agent_with_needs` is public.** Marked `pub` not `#[cfg(test)]` because the integration test file lives at `tests/mood.rs` (an integration test, not a unit test) and integration tests can only call public methods. The doc comment marks it as "test-only entry point" to discourage production use; an alternative is to gate it behind a `#[cfg(any(test, feature = "test-helpers"))]` config later if production-vs-test distinction becomes important.
- **Mood column color.** The plan styles the mood values in `text-neutral-500` (gray) to visually mark them as "derived" relative to the needs values (default text color, "primary inputs"). Tweak the styling later if the visual hierarchy doesn't read well.
