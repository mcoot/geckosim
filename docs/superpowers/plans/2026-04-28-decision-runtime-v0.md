# Decision-runtime v0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the seed catalog to life — agents pick advertisements from `Res<ObjectCatalog>`, score them via the v0 utility-AI formula, commit to a winning action, and apply its effects when the action's `duration_ticks` elapse. Establishes the action lifecycle (commit → execute → complete) without spatial walking and without interrupt handling.

**Architecture:** Three new ECS components on agents (`Personality`, `CurrentAction`, `RecentActionsRing`), one `Component` derive on `SmartObject` so per-instance state lives in the world, two new resources (`CurrentTick`, `SimRngResource`), and two new systems (`decision::execute`, `decision::decide`) registered after `(needs::decay, mood::update)` via `.chain()`. Pure scoring / predicate / effect-application helpers live as separate submodules under `core::systems::decision::*` so they can be unit-tested without ECS scaffolding. `AgentSnapshot` grows a `current_action: Option<CurrentActionView>` field; ts-rs regenerates; the frontend table grows a "Doing" column.

**Tech Stack:** Rust 2021, `bevy_ecs 0.16` (Schedule, Query, Component, Resource), `ts-rs 10` (already wired with `export-ts` feature), Next.js 16 + React 19 + Tailwind v4 + Vitest.

**Reference:** Spec at [`docs/superpowers/specs/2026-04-28-decision-runtime-v0-design.md`](../specs/2026-04-28-decision-runtime-v0-design.md). ADR 0004 (decision model), ADR 0011 (schema; advertisement contract), ADR 0010 (systems inventory), ADR 0008 (time semantics), ADR 0013 (transport).

**VCS note:** This repo uses `jj` (jujutsu), colocated with git. **Never use raw `git`.** Each task starts with `jj new -m "<task title>"`; jj automatically snapshots edits as you work. There is no separate "commit" command.

---

## File Structure

**New files:**
- `crates/core/src/systems/decision/mod.rs` — Task 3 (module declarations + re-exports)
- `crates/core/src/systems/decision/scoring.rs` — Task 3 (pure score helpers + unit tests)
- `crates/core/src/systems/decision/predicates.rs` — Task 3 (pure predicate evaluator + unit tests)
- `crates/core/src/systems/decision/effects.rs` — Task 3 (pure effect applicator + unit tests)
- `crates/core/src/systems/decision/execute.rs` — Task 4 (execute system + unit tests)
- `crates/core/src/systems/decision/decide.rs` — Task 5 (decide system + unit tests)
- `crates/core/tests/decision.rs` — Task 6 (integration test through real seed catalog)
- `crates/core/tests/common/mod.rs` — Task 6 (helper to load workspace `content/`)

**Modified files (Rust):**
- `crates/core/src/ids.rs` — Task 1 (`LeafAreaId::DEFAULT` const)
- `crates/core/src/object/mod.rs` — Task 1 (`Component` derive on `SmartObject`)
- `crates/core/src/agent/mod.rs` — Task 2 (`Component` derive on `Personality` + `Default` derive)
- `crates/core/src/decision/mod.rs` — Task 2 (new `CurrentAction` and `RecentActionsRing` components; `IDLE_DURATION_TICKS` const)
- `crates/core/src/time/mod.rs` — Task 2 (`CurrentTick` resource)
- `crates/core/src/sim.rs` — Tasks 2, 6, 7 (resource init, schedule registration, spawn helpers, snapshot projection)
- `crates/core/src/systems/mod.rs` — Task 3 (`pub mod decision;` declaration)
- `crates/core/src/snapshot.rs` — Task 7 (`CurrentActionView` + `AgentSnapshot.current_action`)
- `crates/core/src/lib.rs` — Tasks 2, 7 (re-export new public types)
- `crates/protocol/tests/roundtrip.rs` — Task 7 (fixtures grow `current_action: None`; new round-trip test)
- `crates/host/src/main.rs` — Task 8 (call `sim.spawn_one_of_each_object_type` after `Sim::new`)

**Modified files (frontend):**
- `apps/web/src/types/sim/AgentSnapshot.ts` — Task 7 (auto-regen)
- `apps/web/src/types/sim/CurrentActionView.ts` — Task 7 (auto-regen, new file)
- `apps/web/src/components/AgentList.tsx` — Task 9 (Doing column)
- `apps/web/src/lib/sim/reducer.test.ts` — Task 9 (fixture grows `current_action`; new round-trip test)

**Existing files untouched (work as-is despite refactors):**
- `crates/core/tests/{snapshot,determinism,needs_decay,catalogs,mood}.rs` — all use `Sim` public API; `AgentSnapshot` field addition is additive; tick semantics shift (increment-before-run) preserves external behavior.
- `crates/host/tests/ws_smoke.rs` — asserts on tick advancement and agent count, not field values.

---

## Task 1: `SmartObject` Component derive + `LeafAreaId::DEFAULT`

**Files:**
- Modify: `crates/core/src/ids.rs`
- Modify: `crates/core/src/object/mod.rs`

Schema-side prep. Adds the `bevy_ecs::component::Component` derive on `SmartObject` (lazy-shard pattern) and a `LeafAreaId::DEFAULT` constant for the v0 spatial stub. No behavior change.

- [ ] **Step 1.1: Start the task commit**

```bash
jj new -m "Decision: SmartObject Component derive + LeafAreaId::DEFAULT"
```

- [ ] **Step 1.2: Add `LeafAreaId::DEFAULT` in `crates/core/src/ids.rs`**

After the `id_newtype!(...)` macro invocations (around line 48), append:

```rust
impl LeafAreaId {
    /// v0 stub: every agent and smart-object instance lives in this single
    /// implicit leaf area until the spatial pass introduces a real world
    /// graph (ADR 0007). The decision-runtime's spatial predicate evaluator
    /// returns `true` for all `Predicate::Spatial(_)` variants at v0.
    pub const DEFAULT: Self = Self::new(0);
}
```

- [ ] **Step 1.3: Add `Component` derive on `SmartObject`**

In `crates/core/src/object/mod.rs`, find the `SmartObject` struct (around line 47):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartObject {
    pub id: ObjectId,
    pub type_id: ObjectTypeId,
    pub location: LeafAreaId,
    pub position: Vec2,
    pub owner: Option<OwnerRef>,
    pub state: StateMap,
}
```

Replace with:

```rust
/// Per-instance smart-object state (per ADR 0011). Doubles as the ECS
/// component on smart-object entities (lazy-sharding — schema and
/// component share a type until a future pass needs them to diverge).
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartObject {
    pub id: ObjectId,
    pub type_id: ObjectTypeId,
    pub location: LeafAreaId,
    pub position: Vec2,
    pub owner: Option<OwnerRef>,
    pub state: StateMap,
}
```

- [ ] **Step 1.4: Verify default-features build clean**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: clean. The `Component` derive is purely additive — no consumers fire yet. The `DEFAULT` const is unused.

- [ ] **Step 1.5: Verify `--features export-ts` build clean**

```bash
cargo build -p gecko-sim-protocol --features export-ts
```

Expected: clean. `SmartObject` doesn't yet have a `ts-rs` derive (it's not on the wire); the build just confirms no regressions.

- [ ] **Step 1.6: Verify clippy clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean. The new `DEFAULT` const may need `#[allow(dead_code)]` if clippy flags it. If so, add:

```rust
#[allow(dead_code, reason = "consumed by spawn_test_object in Task 6")]
pub const DEFAULT: Self = Self::new(0);
```

Drop the allow in Task 6 once the const is used.

- [ ] **Step 1.7: Confirm commit scope**

```bash
jj st
```

Expected: 2 files modified — `crates/core/src/ids.rs` and `crates/core/src/object/mod.rs`.

---

## Task 2: Agent components + `CurrentTick` resource + spawn updates

**Files:**
- Modify: `crates/core/src/agent/mod.rs` (Personality)
- Modify: `crates/core/src/decision/mod.rs` (CurrentAction, RecentActionsRing, IDLE_DURATION_TICKS)
- Modify: `crates/core/src/time/mod.rs` (CurrentTick resource)
- Modify: `crates/core/src/sim.rs` (resource init, tick semantics shift, spawn updates, replace rng field)
- Modify: `crates/core/src/lib.rs` (re-exports)

This task does the bulk of the schema and Sim plumbing. After it lands, every spawned agent carries the new components and the schedule is ready to host decision systems (added in Tasks 4-6).

- [ ] **Step 2.1: Start the task commit**

```bash
jj new -m "Decision: Personality/CurrentAction/RecentActionsRing components; CurrentTick resource; spawn updates"
```

- [ ] **Step 2.2: Add `Component` + `Default` derives to `Personality`**

In `crates/core/src/agent/mod.rs`, find the `Personality` struct (around line 162):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Personality {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
}
```

Replace with:

```rust
/// Big Five personality components; each in `[-1, 1]`. Per ADR 0011.
/// Doubles as the ECS component (lazy sharding). At v0 every agent gets
/// `Personality::default()` (all zeros) until the personality system pass
/// lands — until then `personality_modifier` in the score formula stays
/// at 1.0.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Personality {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
}
```

(Adds `bevy_ecs::component::Component` and `Default` to the derive list. The `Default` derive yields all-zero floats — exactly what we want.)

- [ ] **Step 2.3: Add `CurrentAction` and `RecentActionsRing` components in `crates/core/src/decision/mod.rs`**

Append at the end of `crates/core/src/decision/mod.rs` (after the existing `RecentActionEntry` struct):

```rust
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// ECS components for the decision runtime (per ADR 0011)
// ---------------------------------------------------------------------------

/// Wrapper around the optional committed action so it lives as an ECS
/// component. `None` means the agent is awaiting a decision next tick.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, PartialEq, Default, Serialize, Deserialize,
)]
pub struct CurrentAction(pub Option<CommittedAction>);

/// Bounded ring of recent action templates. FIFO eviction at 16 entries
/// (per ADR 0011). Used by the recency penalty in scoring.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, PartialEq, Default, Serialize, Deserialize,
)]
pub struct RecentActionsRing {
    pub entries: VecDeque<RecentActionEntry>,
}

impl RecentActionsRing {
    /// Per ADR 0011.
    pub const CAPACITY: usize = 16;

    /// Push one entry, evicting the oldest if at capacity.
    pub fn push(&mut self, entry: RecentActionEntry) {
        if self.entries.len() >= Self::CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// True if any entry's `ad_template` matches `(type_id, ad_id)`.
    #[must_use]
    pub fn contains(&self, type_id: crate::ids::ObjectTypeId, ad_id: crate::ids::AdvertisementId) -> bool {
        self.entries
            .iter()
            .any(|e| e.ad_template == (type_id, ad_id))
    }
}

/// `SelfAction(Idle)` duration when no advertisements survive predicate
/// filtering. Re-decides 5 ticks later rather than every tick.
pub const IDLE_DURATION_TICKS: u32 = 5;
```

- [ ] **Step 2.4: Add `CurrentTick` resource in `crates/core/src/time/mod.rs`**

Append to the end of `crates/core/src/time/mod.rs`:

```rust
/// Current sim tick exposed to ECS systems via `Res<CurrentTick>`. Mirrors
/// `Sim::current_tick()`. Updated at the start of every `Sim::tick` call,
/// before the schedule runs, so systems see the tick they're processing.
///
/// At construction `CurrentTick(0)`; after `N` calls to `Sim::tick`, `N`.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CurrentTick(pub u64);
```

- [ ] **Step 2.5: Re-export `CurrentTick` and the new components from `crates/core/src/lib.rs`**

Find the existing `pub use` block. Add:

```rust
pub use decision::{CurrentAction, RecentActionsRing, IDLE_DURATION_TICKS};
pub use time::{CurrentTick, Tick};
```

(`Tick` may already be re-exported; in that case just add `CurrentTick` next to it.)

- [ ] **Step 2.6: Restructure `Sim` in `crates/core/src/sim.rs`**

Replace the contents of `crates/core/src/sim.rs` with:

```rust
//! Live `Sim` driver: wraps a `bevy_ecs::World` and advances tick state
//! via a `bevy_ecs::schedule::Schedule`.
//!
//! Honours the public API contract from ADR 0012 partially:
//!   - `new`, `tick`, `current_tick`, `snapshot`.
//!   - `delta_since`, `apply_input` deferred to a later pass.

use std::collections::HashMap;

use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
use bevy_ecs::world::World;

use crate::agent::{Accessory, AccessoryCatalog, Identity, Mood, Needs, Personality};
use crate::decision::{CurrentAction, RecentActionsRing};
use crate::ids::{AccessoryId, AgentId, ObjectTypeId};
use crate::object::{ObjectCatalog, ObjectType};
use crate::rng::PrngState;
use crate::snapshot::{AgentSnapshot, Snapshot};
use crate::systems;
use crate::time::CurrentTick;

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

/// Wrapper around the global `PrngState` so systems can borrow it via
/// `ResMut<SimRngResource>`. Per-agent RNG sub-streams are deferred per
/// the spec's "RNG plumbing" section.
#[derive(bevy_ecs::prelude::Resource, Debug)]
pub struct SimRngResource(pub PrngState);

/// The live simulation. Owns its `bevy_ecs::World`, a `Schedule` of
/// per-tick systems, and the canonical clock.
pub struct Sim {
    world: World,
    schedule: Schedule,
    tick: u64,
    next_agent_id: u64,
    next_object_id: u64,
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
        world.insert_resource(SimRngResource(PrngState::from_seed(seed)));
        world.insert_resource(CurrentTick(0));

        let mut schedule = Schedule::default();
        schedule.add_systems((systems::needs::decay, systems::mood::update).chain());

        Self {
            world,
            schedule,
            tick: 0,
            next_agent_id: 0,
            next_object_id: 0,
        }
    }

    /// Advance the simulation by one tick (one sim-minute per ADR 0008).
    /// Increments the tick counter first so systems see the tick they're
    /// processing via `Res<CurrentTick>`.
    pub fn tick(&mut self) -> TickReport {
        self.tick += 1;
        *self.world.resource_mut::<CurrentTick>() = CurrentTick(self.tick);
        self.schedule.run(&mut self.world);
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

    /// Spawn a fresh agent at full needs and neutral mood with a
    /// monotonically allocated `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced in a future pass.
    pub fn spawn_test_agent(&mut self, name: &str) -> AgentId {
        self.spawn_test_agent_with_needs(name, Needs::full())
    }

    /// Spawn a fresh agent with explicit initial needs, neutral mood,
    /// default personality, and decision-runtime components (no current
    /// action, empty recent-actions ring). Test-only entry point.
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
            Personality::default(),
            CurrentAction::default(),
            RecentActionsRing::default(),
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
                let mood = entity_ref.get::<Mood>()?;
                Some(AgentSnapshot {
                    id: identity.id,
                    name: identity.name.clone(),
                    needs: *needs,
                    mood: *mood,
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

Key differences from the previous version:
- `Sim` no longer has a `rng` field — replaced by `SimRngResource` inserted into the world.
- Added `next_object_id: u64` for Task 6's spawn helper.
- `Sim::new` inserts `SimRngResource` and `CurrentTick(0)` into the world.
- `Sim::tick` increments `self.tick` BEFORE running the schedule and syncs `CurrentTick`.
- `spawn_test_agent_with_needs` attaches `Personality::default()`, `CurrentAction::default()`, `RecentActionsRing::default()` in the spawn bundle.
- `Sim::snapshot` is unchanged at this task — Task 7 grows it.

- [ ] **Step 2.7: Verify default-features build clean**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: all green. Existing tests `tests/{snapshot,determinism,needs_decay,catalogs,mood}.rs` continue to pass — the additional components on agents are harmless (no system queries them yet), and the tick-semantics shift is invisible from outside `Sim`.

- [ ] **Step 2.8: Verify clippy clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 2.9: Confirm commit scope**

```bash
jj st
```

Expected: 5 files modified — `crates/core/src/agent/mod.rs`, `decision/mod.rs`, `time/mod.rs`, `sim.rs`, `lib.rs`. (Plus the `LeafAreaId::DEFAULT` clippy-allow drop if you took that path in Task 1 — won't appear here.)

---

## Task 3: scoring + predicates + effects pure helpers + unit tests

**Files:**
- Modify: `crates/core/src/systems/mod.rs`
- Create: `crates/core/src/systems/decision/mod.rs`
- Create: `crates/core/src/systems/decision/scoring.rs`
- Create: `crates/core/src/systems/decision/predicates.rs`
- Create: `crates/core/src/systems/decision/effects.rs`

This task lands the pure-functional helpers — no ECS scaffolding, no schedule wiring. Each submodule owns a slice of the decision-runtime logic and is unit-tested inline.

The systems (`decide`, `execute`) that call these helpers land in Tasks 4 and 5.

- [ ] **Step 3.1: Start the task commit**

```bash
jj new -m "Decision: scoring/predicates/effects pure helpers + unit tests"
```

- [ ] **Step 3.2: Declare the new submodule in `crates/core/src/systems/mod.rs`**

After the existing `pub mod mood;` line, append:

```rust
pub mod decision;
```

Update the docstring's status comment block:

```rust
//!   - `needs`         (1) need decay      ← landed
//!   - `personality`   (2) personality (read-only)
//!   - `mood`          (3) mood update     ← landed
//!   - `memory`        (4) memory ring & decay
//!   - …
//!
//! Plus cross-cutting:
//!   - `decision`      utility-AI scoring + commit + execute (per ADR 0004)  ← landed
```

- [ ] **Step 3.3: Create `crates/core/src/systems/decision/mod.rs`**

```rust
//! Decision runtime per ADR 0004 / 0011. Two systems registered in the
//! per-tick schedule (in this order, after `mood::update`):
//!
//! 1. `execute` — completes any committed action whose `expected_end_tick`
//!    has been reached: applies effects atomically, pushes a recent-actions
//!    ring entry, clears the agent's `current_action`.
//! 2. `decide`  — for each agent without a current action, evaluates every
//!    advertisement against preconditions, scores the survivors, picks
//!    weighted-random from top-N, commits the winner.
//!
//! Pure helpers for each phase live in their own submodule so they can be
//! unit-tested without ECS scaffolding.

pub mod effects;
pub mod predicates;
pub mod scoring;
```

(`execute` and `decide` modules are added in Tasks 4 and 5 respectively.)

- [ ] **Step 3.4: Write the failing scoring tests in `crates/core/src/systems/decision/scoring.rs`**

Create the file with **only** the test module so it fails to compile against the not-yet-written helpers:

```rust
//! Pure scoring helpers per ADR 0011's "Action evaluation contract".
//!
//! Score formula:
//!     base * personality * mood * (1 - recency) + noise
//!
//! All factors stay non-negative; modifier clamps land at `0.1`.

#[cfg(test)]
mod tests {
    use crate::agent::{Mood, Needs, Personality};
    use crate::decision::{RecentActionEntry, RecentActionsRing};
    use crate::ids::{AdvertisementId, ObjectTypeId};
    use crate::object::{ScoreTemplate, SituationalModifier};
    use crate::systems::decision::scoring::{
        base_utility, mood_modifier, personality_modifier, recency_penalty, weighted_pick,
    };
    use crate::agent::{Need, MoodDim};

    fn empty_score_template() -> ScoreTemplate {
        ScoreTemplate {
            need_weights: vec![],
            personality_weights: Personality::default(),
            situational_modifiers: vec![],
        }
    }

    #[test]
    fn base_utility_zero_when_need_full() {
        let needs = Needs::full();
        let template = ScoreTemplate {
            need_weights: vec![(Need::Hunger, 1.0)],
            ..empty_score_template()
        };
        assert!((base_utility(&needs, &template)).abs() < 1e-6);
    }

    #[test]
    fn base_utility_max_when_need_empty() {
        let needs = Needs {
            hunger: 0.0,
            sleep: 1.0,
            social: 1.0,
            hygiene: 1.0,
            fun: 1.0,
            comfort: 1.0,
        };
        let template = ScoreTemplate {
            need_weights: vec![(Need::Hunger, 1.0), (Need::Sleep, 0.5)],
            ..empty_score_template()
        };
        // hunger pressure = 1.0 * 1.0 = 1.0; sleep pressure = 0.0 * 0.5 = 0.0
        assert!((base_utility(&needs, &template) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn personality_modifier_one_at_zero_personality() {
        let p = Personality::default();
        let weights = Personality {
            openness: 0.5,
            ..Personality::default()
        };
        // dot = 0; modifier = 1 + 0.5*0 = 1.0
        assert!((personality_modifier(&p, &weights) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn personality_modifier_clamped_at_floor() {
        // Construct a strongly opposing personality + weight pair so the
        // raw modifier would go below 0.1.
        let p = Personality {
            openness: 1.0,
            conscientiousness: 1.0,
            extraversion: 1.0,
            agreeableness: 1.0,
            neuroticism: 1.0,
        };
        let weights = Personality {
            openness: -1.0,
            conscientiousness: -1.0,
            extraversion: -1.0,
            agreeableness: -1.0,
            neuroticism: -1.0,
        };
        // dot = -5; raw = 1 + 0.5*(-5) = -1.5; clamped to 0.1
        assert!((personality_modifier(&p, &weights) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn mood_modifier_compounds_multiplicatively() {
        let mood = Mood {
            valence: 0.5,
            arousal: 0.5,
            stress: 0.0,
        };
        let modifiers = vec![
            SituationalModifier::MoodWeight {
                dim: MoodDim::Valence,
                weight: 1.0,
            },
            SituationalModifier::MoodWeight {
                dim: MoodDim::Arousal,
                weight: 1.0,
            },
        ];
        // (1 + 1*0.5) * (1 + 1*0.5) = 1.5 * 1.5 = 2.25
        assert!((mood_modifier(&mood, &modifiers) - 2.25).abs() < 1e-6);
    }

    #[test]
    fn mood_modifier_default_one_when_no_mood_weights() {
        let mood = Mood {
            valence: 0.5,
            arousal: 0.5,
            stress: 0.0,
        };
        let modifiers = vec![];
        assert!((mood_modifier(&mood, &modifiers) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn recency_penalty_zero_when_not_in_ring() {
        let ring = RecentActionsRing::default();
        assert!(
            (recency_penalty(&ring, ObjectTypeId::new(1), AdvertisementId::new(1))).abs() < 1e-6
        );
    }

    #[test]
    fn recency_penalty_half_when_in_ring() {
        let mut ring = RecentActionsRing::default();
        ring.push(RecentActionEntry {
            ad_template: (ObjectTypeId::new(1), AdvertisementId::new(1)),
            completed_tick: 100,
        });
        assert!(
            (recency_penalty(&ring, ObjectTypeId::new(1), AdvertisementId::new(1)) - 0.5).abs()
                < 1e-6
        );
    }

    #[test]
    fn weighted_pick_returns_only_candidate() {
        let mut rng = rand_pcg::Pcg32::new(0, 0);
        use rand::Rng;
        let candidates = vec![(AdvertisementId::new(1), 1.0)];
        let picked = weighted_pick(&candidates, &mut rng);
        assert_eq!(picked, Some(AdvertisementId::new(1)));
    }

    #[test]
    fn weighted_pick_none_on_empty() {
        let mut rng = rand_pcg::Pcg32::new(0, 0);
        let candidates: Vec<(AdvertisementId, f32)> = vec![];
        assert_eq!(weighted_pick(&candidates, &mut rng), None);
    }

    #[test]
    fn weighted_pick_falls_back_to_uniform_on_zero_total() {
        let mut rng = rand_pcg::Pcg32::new(0, 0);
        let candidates = vec![
            (AdvertisementId::new(1), 0.0),
            (AdvertisementId::new(2), 0.0),
        ];
        // Pick something — both candidates equally likely. Just confirm
        // we get one of them, not None.
        let picked = weighted_pick(&candidates, &mut rng);
        assert!(picked.is_some());
        let id = picked.unwrap();
        assert!(id == AdvertisementId::new(1) || id == AdvertisementId::new(2));
    }

    #[test]
    fn weighted_pick_picks_proportional_to_weight() {
        // With score 100x larger, the high-weight candidate should win
        // overwhelmingly. Run 100 picks and assert the high-weight wins
        // > 90 times.
        let mut rng = rand_pcg::Pcg32::new(42, 0);
        let candidates = vec![
            (AdvertisementId::new(1), 100.0),
            (AdvertisementId::new(2), 1.0),
        ];
        let mut wins_high = 0;
        for _ in 0..100 {
            if weighted_pick(&candidates, &mut rng) == Some(AdvertisementId::new(1)) {
                wins_high += 1;
            }
        }
        assert!(wins_high > 90, "wins_high={}", wins_high);
    }
}
```

- [ ] **Step 3.5: Run the failing test**

```bash
cargo test -p gecko-sim-core systems::decision::scoring
```

Expected: compile errors — the imported helpers don't exist yet.

- [ ] **Step 3.6: Implement the scoring helpers**

Add to the top of `crates/core/src/systems/decision/scoring.rs` (above the `#[cfg(test)] mod tests` block):

```rust
use rand::Rng;

use crate::agent::{Mood, MoodDim, Need, Needs, Personality};
use crate::decision::RecentActionsRing;
use crate::ids::{AdvertisementId, ObjectTypeId};
use crate::object::{ScoreTemplate, SituationalModifier};

/// Multiplier applied to the score of an advertisement that the agent
/// recently performed (per ADR 0011). Halves the score.
pub const RECENCY_PENALTY: f32 = 0.5;

/// Sensitivity coefficient for the personality-dot-product modifier
/// (per ADR 0011). Tunable.
pub const PERSONALITY_SENSITIVITY: f32 = 0.5;

/// Floor for personality and mood modifiers; clamps the raw modifier so
/// the score stays strictly positive.
pub const MODIFIER_FLOOR: f32 = 0.1;

/// Per ADR 0011: `base_utility = Σ over (need, factor) in need_weights:
/// factor * (1.0 - need_value(needs, need))`. Pressure rises as the need
/// drops; weights are content-defined (non-negative in seed catalog).
#[must_use]
pub fn base_utility(needs: &Needs, template: &ScoreTemplate) -> f32 {
    template
        .need_weights
        .iter()
        .map(|(need, factor)| factor * (1.0 - need_value(needs, *need)))
        .sum()
}

/// Per ADR 0011: `1.0 + sensitivity * dot(personality, weights)`,
/// clamped to `[MODIFIER_FLOOR, +∞]`.
#[must_use]
pub fn personality_modifier(personality: &Personality, weights: &Personality) -> f32 {
    let dot = personality.openness * weights.openness
        + personality.conscientiousness * weights.conscientiousness
        + personality.extraversion * weights.extraversion
        + personality.agreeableness * weights.agreeableness
        + personality.neuroticism * weights.neuroticism;
    (1.0 + PERSONALITY_SENSITIVITY * dot).max(MODIFIER_FLOOR)
}

/// Multiplies `(1 + weight * mood_value)` over each `MoodWeight` modifier;
/// returns 1.0 when the ad has no `MoodWeight` entries. Clamps each factor
/// at `MODIFIER_FLOOR`. Other `SituationalModifier` variants are no-ops at
/// v0 (they contribute `1.0`).
#[must_use]
pub fn mood_modifier(mood: &Mood, modifiers: &[SituationalModifier]) -> f32 {
    let mut product = 1.0;
    for modifier in modifiers {
        if let SituationalModifier::MoodWeight { dim, weight } = modifier {
            let factor = (1.0 + weight * mood_value(mood, *dim)).max(MODIFIER_FLOOR);
            product *= factor;
        }
    }
    product
}

/// `RECENCY_PENALTY` if the ad was performed recently (matched by
/// `(ObjectTypeId, AdvertisementId)` template), else 0.
#[must_use]
pub fn recency_penalty(
    ring: &RecentActionsRing,
    type_id: ObjectTypeId,
    ad_id: AdvertisementId,
) -> f32 {
    if ring.contains(type_id, ad_id) {
        RECENCY_PENALTY
    } else {
        0.0
    }
}

/// Pick one candidate at random, weighted by score. If all weights are
/// zero, falls back to uniform random over the candidate list. Returns
/// `None` if the list is empty.
#[must_use]
pub fn weighted_pick<R: Rng + ?Sized>(
    candidates: &[(AdvertisementId, f32)],
    rng: &mut R,
) -> Option<AdvertisementId> {
    if candidates.is_empty() {
        return None;
    }
    let total: f32 = candidates.iter().map(|(_, score)| *score).sum();
    if total <= 0.0 {
        // Uniform fallback.
        let idx = rng.gen_range(0..candidates.len());
        return Some(candidates[idx].0);
    }
    let mut roll: f32 = rng.gen::<f32>() * total;
    for (id, score) in candidates {
        if roll < *score {
            return Some(*id);
        }
        roll -= *score;
    }
    // Numerical edge case (roll just under total): return the last candidate.
    candidates.last().map(|(id, _)| *id)
}

fn need_value(needs: &Needs, need: Need) -> f32 {
    match need {
        Need::Hunger => needs.hunger,
        Need::Sleep => needs.sleep,
        Need::Social => needs.social,
        Need::Hygiene => needs.hygiene,
        Need::Fun => needs.fun,
        Need::Comfort => needs.comfort,
    }
}

fn mood_value(mood: &Mood, dim: MoodDim) -> f32 {
    match dim {
        MoodDim::Valence => mood.valence,
        MoodDim::Arousal => mood.arousal,
        MoodDim::Stress => mood.stress,
    }
}
```

- [ ] **Step 3.7: Run the scoring tests**

```bash
cargo test -p gecko-sim-core systems::decision::scoring
```

Expected: all 11 tests pass.

- [ ] **Step 3.8: Write the failing predicate tests in `crates/core/src/systems/decision/predicates.rs`**

```rust
//! Predicate evaluation per ADR 0011's "Action evaluation contract".
//!
//! At v0:
//! - `AgentNeed` and `ObjectState` evaluate against the agent's `Needs`
//!   and the smart object's `StateMap` respectively.
//! - `Spatial(_)` always passes (every entity lives in `LeafAreaId::DEFAULT`).
//! - `AgentSkill`, `AgentInventory`, `AgentRelationship`, `MacroState`,
//!   `TimeOfDay` always fail (the systems they depend on don't exist yet,
//!   so any ad referencing them is filtered out).

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::agent::{Need, Needs};
    use crate::object::{Op, Predicate, SpatialReq, StateValue};
    use crate::systems::decision::predicates::{evaluate, EvalContext};

    fn ctx_with_needs(needs: Needs) -> EvalContext<'static> {
        // Static empty state map for the lifetime of the test fn.
        // We can't construct one inline, so we leak a default for tests.
        // (Tests don't run in tight memory-constrained environments.)
        let leaked: &'static HashMap<String, StateValue> = Box::leak(Box::new(HashMap::new()));
        EvalContext {
            needs: Box::leak(Box::new(needs)),
            object_state: leaked,
        }
    }

    #[test]
    fn agent_need_lt_passes_when_below_threshold() {
        let ctx = ctx_with_needs(Needs {
            hunger: 0.3,
            sleep: 1.0,
            social: 1.0,
            hygiene: 1.0,
            fun: 1.0,
            comfort: 1.0,
        });
        let pred = Predicate::AgentNeed(Need::Hunger, Op::Lt, 0.6);
        assert!(evaluate(&pred, &ctx));
    }

    #[test]
    fn agent_need_lt_fails_when_above_threshold() {
        let ctx = ctx_with_needs(Needs::full());
        let pred = Predicate::AgentNeed(Need::Hunger, Op::Lt, 0.6);
        assert!(!evaluate(&pred, &ctx));
    }

    #[test]
    fn object_state_eq_bool_passes_when_matched() {
        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        let leaked: &'static HashMap<String, StateValue> = Box::leak(Box::new(state));
        let ctx = EvalContext {
            needs: Box::leak(Box::new(Needs::full())),
            object_state: leaked,
        };
        let pred = Predicate::ObjectState("stocked".to_string(), Op::Eq, StateValue::Bool(true));
        assert!(evaluate(&pred, &ctx));
    }

    #[test]
    fn object_state_missing_key_fails() {
        let ctx = ctx_with_needs(Needs::full());
        let pred = Predicate::ObjectState("missing".to_string(), Op::Eq, StateValue::Bool(true));
        assert!(!evaluate(&pred, &ctx));
    }

    #[test]
    fn spatial_always_passes() {
        let ctx = ctx_with_needs(Needs::full());
        for req in [
            SpatialReq::SameLeafArea,
            SpatialReq::AdjacentArea,
            SpatialReq::KnownPlace,
        ] {
            assert!(evaluate(&Predicate::Spatial(req), &ctx));
        }
    }

    #[test]
    fn agent_skill_always_fails_at_v0() {
        let ctx = ctx_with_needs(Needs::full());
        let pred = Predicate::AgentSkill(crate::agent::Skill::Social, Op::Gt, 0.5);
        assert!(!evaluate(&pred, &ctx));
    }
}
```

- [ ] **Step 3.9: Run the failing test**

```bash
cargo test -p gecko-sim-core systems::decision::predicates
```

Expected: compile errors — `evaluate` and `EvalContext` don't exist.

- [ ] **Step 3.10: Implement the predicate evaluator**

Add to the top of `crates/core/src/systems/decision/predicates.rs`:

```rust
use crate::agent::{Need, Needs};
use crate::object::{Op, Predicate, StateMap, StateValue};

/// Read-only context for predicate evaluation. Carries everything an
/// `evaluate(...)` call might need from the agent + object.
pub struct EvalContext<'a> {
    pub needs: &'a Needs,
    pub object_state: &'a StateMap,
}

/// Evaluate a single predicate against the agent and (if applicable) the
/// smart-object state. Returns `false` for predicate variants whose
/// referent systems don't exist at v0 (AgentSkill/AgentInventory/
/// AgentRelationship/MacroState/TimeOfDay) — the ad gets filtered out.
#[must_use]
pub fn evaluate(predicate: &Predicate, ctx: &EvalContext<'_>) -> bool {
    match predicate {
        Predicate::AgentNeed(need, op, threshold) => {
            apply_op_f32(need_value(ctx.needs, *need), *op, *threshold)
        }
        Predicate::ObjectState(key, op, expected) => ctx
            .object_state
            .get(key)
            .is_some_and(|actual| compare_state_value(actual, *op, expected)),
        Predicate::Spatial(_) => true,
        // v0: missing systems → predicate fails → ad filtered out.
        Predicate::AgentSkill(_, _, _)
        | Predicate::AgentInventory(_, _, _)
        | Predicate::AgentRelationship(_, _, _, _)
        | Predicate::MacroState(_, _, _)
        | Predicate::TimeOfDay(_) => false,
    }
}

fn apply_op_f32(lhs: f32, op: Op, rhs: f32) -> bool {
    match op {
        Op::Lt => lhs < rhs,
        Op::Le => lhs <= rhs,
        Op::Eq => (lhs - rhs).abs() < f32::EPSILON,
        Op::Ge => lhs >= rhs,
        Op::Gt => lhs > rhs,
        Op::Ne => (lhs - rhs).abs() >= f32::EPSILON,
    }
}

fn compare_state_value(actual: &StateValue, op: Op, expected: &StateValue) -> bool {
    match (actual, expected) {
        (StateValue::Bool(a), StateValue::Bool(b)) => match op {
            Op::Eq => a == b,
            Op::Ne => a != b,
            _ => false,
        },
        (StateValue::Int(a), StateValue::Int(b)) => match op {
            Op::Lt => a < b,
            Op::Le => a <= b,
            Op::Eq => a == b,
            Op::Ge => a >= b,
            Op::Gt => a > b,
            Op::Ne => a != b,
        },
        (StateValue::Float(a), StateValue::Float(b)) => apply_op_f32(*a, op, *b),
        (StateValue::Text(a), StateValue::Text(b)) => match op {
            Op::Eq => a == b,
            Op::Ne => a != b,
            _ => false,
        },
        // Type mismatch — predicate fails.
        _ => false,
    }
}

fn need_value(needs: &Needs, need: Need) -> f32 {
    match need {
        Need::Hunger => needs.hunger,
        Need::Sleep => needs.sleep,
        Need::Social => needs.social,
        Need::Hygiene => needs.hygiene,
        Need::Fun => needs.fun,
        Need::Comfort => needs.comfort,
    }
}
```

- [ ] **Step 3.11: Run the predicate tests**

```bash
cargo test -p gecko-sim-core systems::decision::predicates
```

Expected: all 6 tests pass.

- [ ] **Step 3.12: Write the failing effect tests in `crates/core/src/systems/decision/effects.rs`**

```rust
//! Effect application per ADR 0011's "Effect application" section.
//!
//! v0: only `AgentNeedDelta` and `AgentMoodDelta` are wired. Other variants
//! log a `tracing::warn!` no-op so unsupported content can flow through
//! the loader without crashing.

#[cfg(test)]
mod tests {
    use crate::agent::{Mood, MoodDim, Need, Needs};
    use crate::object::Effect;
    use crate::systems::decision::effects::{apply, EffectTarget};

    #[test]
    fn agent_need_delta_applies() {
        let mut needs = Needs {
            hunger: 0.3,
            ..Needs::full()
        };
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
        };
        apply(&Effect::AgentNeedDelta(Need::Hunger, 0.4), &mut target);
        assert!((needs.hunger - 0.7).abs() < 1e-6, "hunger={}", needs.hunger);
    }

    #[test]
    fn agent_need_delta_clamps_at_one() {
        let mut needs = Needs {
            hunger: 0.9,
            ..Needs::full()
        };
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
        };
        apply(&Effect::AgentNeedDelta(Need::Hunger, 0.5), &mut target);
        assert!((needs.hunger - 1.0).abs() < 1e-6);
    }

    #[test]
    fn agent_need_delta_clamps_at_zero() {
        let mut needs = Needs {
            hunger: 0.1,
            ..Needs::full()
        };
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
        };
        apply(&Effect::AgentNeedDelta(Need::Hunger, -0.5), &mut target);
        assert!(needs.hunger.abs() < 1e-6);
    }

    #[test]
    fn agent_mood_delta_applies_to_valence() {
        let mut needs = Needs::full();
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
        };
        apply(&Effect::AgentMoodDelta(MoodDim::Valence, 0.5), &mut target);
        assert!((mood.valence - 0.5).abs() < 1e-6);
    }

    #[test]
    fn unsupported_effect_does_not_panic() {
        let mut needs = Needs::full();
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
        };
        // MoneyDelta is not yet implemented; should warn and no-op.
        apply(&Effect::MoneyDelta(100), &mut target);
        // No assertion — just confirm we didn't panic.
    }
}
```

- [ ] **Step 3.13: Run the failing test**

```bash
cargo test -p gecko-sim-core systems::decision::effects
```

Expected: compile errors — `apply` and `EffectTarget` don't exist.

- [ ] **Step 3.14: Implement the effect applicator**

Add to the top of `crates/core/src/systems/decision/effects.rs`:

```rust
use crate::agent::{Mood, MoodDim, Need, Needs};
use crate::object::Effect;

/// Mutable references to the agent's effect-targeted components. v0 covers
/// `Needs` and `Mood`; other components (`Skills`, `Money`, `Inventory`,
/// `Memory`, …) join when their systems land.
pub struct EffectTarget<'a> {
    pub needs: &'a mut Needs,
    pub mood: &'a mut Mood,
}

/// Apply one effect to the agent's target components. Unsupported variants
/// log a `tracing::warn!` and return without modifying state, so the loader
/// can ship content with future-system effects without crashing the sim.
pub fn apply(effect: &Effect, target: &mut EffectTarget<'_>) {
    match effect {
        Effect::AgentNeedDelta(need, delta) => {
            let value = need_field_mut(target.needs, *need);
            *value = (*value + delta).clamp(0.0, 1.0);
        }
        Effect::AgentMoodDelta(dim, delta) => {
            let (value, lo, hi) = mood_field_mut(target.mood, *dim);
            *value = (*value + delta).clamp(lo, hi);
        }
        // v0: not yet implemented.
        Effect::AgentSkillDelta(_, _)
        | Effect::MoneyDelta(_)
        | Effect::InventoryDelta(_, _)
        | Effect::MemoryGenerate { .. }
        | Effect::RelationshipDelta(_, _, _)
        | Effect::HealthConditionChange(_)
        | Effect::PromotedEvent(_, _) => {
            tracing::warn!(
                ?effect,
                "decision::effects::apply: effect kind not yet implemented; no-op",
            );
        }
    }
}

fn need_field_mut(needs: &mut Needs, kind: Need) -> &mut f32 {
    match kind {
        Need::Hunger => &mut needs.hunger,
        Need::Sleep => &mut needs.sleep,
        Need::Social => &mut needs.social,
        Need::Hygiene => &mut needs.hygiene,
        Need::Fun => &mut needs.fun,
        Need::Comfort => &mut needs.comfort,
    }
}

/// Returns `(field, lower_bound, upper_bound)` for the mood dimension.
fn mood_field_mut(mood: &mut Mood, dim: MoodDim) -> (&mut f32, f32, f32) {
    match dim {
        MoodDim::Valence => (&mut mood.valence, -1.0, 1.0),
        MoodDim::Arousal => (&mut mood.arousal, 0.0, 1.0),
        MoodDim::Stress => (&mut mood.stress, 0.0, 1.0),
    }
}
```

- [ ] **Step 3.15: Run the effect tests**

```bash
cargo test -p gecko-sim-core systems::decision::effects
```

Expected: all 5 tests pass.

- [ ] **Step 3.16: Verify the workspace still builds clean**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all green. None of these helpers are called yet; this task is pure additions.

- [ ] **Step 3.17: Confirm commit scope**

```bash
jj st
```

Expected: 1 modified (`crates/core/src/systems/mod.rs`) + 4 new files under `crates/core/src/systems/decision/`.

---

## Task 4: `execute` system + unit tests

**Files:**
- Create: `crates/core/src/systems/decision/execute.rs`
- Modify: `crates/core/src/systems/decision/mod.rs` (declare submodule)

The `execute` system completes any committed action whose `expected_end_tick` has been reached: applies effects atomically, pushes a recent-actions ring entry (for object-targeted actions), clears the agent's `current_action`. Idle self-actions complete the same way but skip effects + ring entry.

- [ ] **Step 4.1: Start the task commit**

```bash
jj new -m "Decision: execute system + unit tests"
```

- [ ] **Step 4.2: Declare the submodule in `crates/core/src/systems/decision/mod.rs`**

Add `pub mod execute;` after the existing `pub mod scoring;` line. The full file:

```rust
//! Decision runtime per ADR 0004 / 0011. Two systems registered in the
//! per-tick schedule (in this order, after `mood::update`):
//!
//! 1. `execute` — completes any committed action whose `expected_end_tick`
//!    has been reached: applies effects atomically, pushes a recent-actions
//!    ring entry, clears the agent's `current_action`.
//! 2. `decide`  — for each agent without a current action, evaluates every
//!    advertisement against preconditions, scores the survivors, picks
//!    weighted-random from top-N, commits the winner.
//!
//! Pure helpers for each phase live in their own submodule so they can be
//! unit-tested without ECS scaffolding.

pub mod effects;
pub mod execute;
pub mod predicates;
pub mod scoring;
```

- [ ] **Step 4.3: Write the failing tests**

Create `crates/core/src/systems/decision/execute.rs`:

```rust
//! ECS system: execute. Completes any committed action whose
//! `expected_end_tick` has been reached this tick.
//!
//! For object-targeted actions: looks up the advertisement via the
//! catalog, applies each `Effect` to the agent's components, pushes a
//! `RecentActionEntry` into the agent's recent-actions ring, clears
//! `CurrentAction`.
//!
//! For self-actions (`Idle`/`Wait`): clears `CurrentAction` only — no
//! effects, no ring entry.

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::agent::{Mood, Needs};
    use crate::decision::{
        ActionRef, CommittedAction, CurrentAction, Phase, RecentActionsRing, SelfActionKind,
    };
    use crate::ids::{AdvertisementId, ObjectId, ObjectTypeId};
    use crate::object::{
        Advertisement, Effect, InterruptClass, ObjectCatalog, ObjectType, Op, Predicate,
        ScoreTemplate, SmartObject, StateValue,
    };
    use crate::systems::decision::execute::execute;
    use crate::time::CurrentTick;
    use crate::world::Vec2;
    use crate::ids::LeafAreaId;
    use crate::agent::{Need, Personality};
    use std::collections::HashMap;

    fn fridge_object_type() -> ObjectType {
        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        ObjectType {
            id: ObjectTypeId::new(1),
            display_name: "Fridge".to_string(),
            mesh_id: crate::object::MeshId(1),
            default_state: state,
            advertisements: vec![Advertisement {
                id: AdvertisementId::new(1),
                display_name: "Eat snack".to_string(),
                preconditions: vec![Predicate::AgentNeed(Need::Hunger, Op::Lt, 0.6)],
                effects: vec![Effect::AgentNeedDelta(Need::Hunger, 0.4)],
                duration_ticks: 10,
                interrupt_class: InterruptClass::NeedsThresholdOnly,
                score_template: ScoreTemplate {
                    need_weights: vec![(Need::Hunger, 1.0)],
                    personality_weights: Personality::default(),
                    situational_modifiers: vec![],
                },
            }],
        }
    }

    fn build_world(
        agent_needs: Needs,
        action: Option<CommittedAction>,
        current_tick: u64,
    ) -> (World, bevy_ecs::entity::Entity) {
        let mut world = World::new();
        let fridge = fridge_object_type();
        let mut object_types = HashMap::new();
        object_types.insert(fridge.id, fridge);
        world.insert_resource(ObjectCatalog { by_id: object_types });
        world.insert_resource(CurrentTick(current_tick));

        // One smart-object instance.
        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        world.spawn(SmartObject {
            id: ObjectId::new(0),
            type_id: ObjectTypeId::new(1),
            location: LeafAreaId::DEFAULT,
            position: Vec2::ZERO,
            owner: None,
            state,
        });

        let agent = world
            .spawn((
                agent_needs,
                Mood::neutral(),
                CurrentAction(action),
                RecentActionsRing::default(),
            ))
            .id();
        (world, agent)
    }

    #[test]
    fn completed_action_applies_effects_and_clears_current_action() {
        let action = CommittedAction {
            action: ActionRef::Object {
                object: ObjectId::new(0),
                ad: AdvertisementId::new(1),
            },
            started_tick: 0,
            expected_end_tick: 10,
            phase: Phase::Performing,
            target_position: None,
        };
        let (mut world, agent) = build_world(
            Needs {
                hunger: 0.3,
                ..Needs::full()
            },
            Some(action),
            10,
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(execute);
        schedule.run(&mut world);

        let needs = world.get::<Needs>(agent).unwrap();
        assert!((needs.hunger - 0.7).abs() < 1e-6, "hunger={}", needs.hunger);
        let current = world.get::<CurrentAction>(agent).unwrap();
        assert!(current.0.is_none(), "current_action should be None");
        let ring = world.get::<RecentActionsRing>(agent).unwrap();
        assert_eq!(ring.entries.len(), 1);
        assert_eq!(
            ring.entries[0].ad_template,
            (ObjectTypeId::new(1), AdvertisementId::new(1))
        );
    }

    #[test]
    fn in_progress_action_does_not_complete() {
        let action = CommittedAction {
            action: ActionRef::Object {
                object: ObjectId::new(0),
                ad: AdvertisementId::new(1),
            },
            started_tick: 0,
            expected_end_tick: 10,
            phase: Phase::Performing,
            target_position: None,
        };
        let (mut world, agent) = build_world(
            Needs {
                hunger: 0.3,
                ..Needs::full()
            },
            Some(action),
            5, // current_tick < expected_end_tick
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(execute);
        schedule.run(&mut world);

        let needs = world.get::<Needs>(agent).unwrap();
        assert!((needs.hunger - 0.3).abs() < 1e-6, "hunger should be unchanged");
        let current = world.get::<CurrentAction>(agent).unwrap();
        assert!(current.0.is_some(), "current_action should still be set");
    }

    #[test]
    fn idle_self_action_clears_without_effects() {
        let action = CommittedAction {
            action: ActionRef::SelfAction(SelfActionKind::Idle),
            started_tick: 0,
            expected_end_tick: 5,
            phase: Phase::Performing,
            target_position: None,
        };
        let (mut world, agent) = build_world(Needs::full(), Some(action), 5);
        let mut schedule = Schedule::default();
        schedule.add_systems(execute);
        schedule.run(&mut world);

        let current = world.get::<CurrentAction>(agent).unwrap();
        assert!(current.0.is_none());
        let ring = world.get::<RecentActionsRing>(agent).unwrap();
        // Idle does NOT add a ring entry.
        assert!(ring.entries.is_empty());
    }

    #[test]
    fn no_action_is_noop() {
        let (mut world, agent) = build_world(Needs::full(), None, 5);
        let mut schedule = Schedule::default();
        schedule.add_systems(execute);
        schedule.run(&mut world);

        let current = world.get::<CurrentAction>(agent).unwrap();
        assert!(current.0.is_none());
    }
}
```

- [ ] **Step 4.4: Run the failing test**

```bash
cargo test -p gecko-sim-core systems::decision::execute
```

Expected: compile error — `execute` function not found.

- [ ] **Step 4.5: Implement `execute`**

Add to the top of `crates/core/src/systems/decision/execute.rs`:

```rust
use bevy_ecs::system::{Query, Res};

use crate::agent::{Mood, Needs};
use crate::decision::{
    ActionRef, CurrentAction, RecentActionEntry, RecentActionsRing,
};
use crate::ids::{AdvertisementId, ObjectId};
use crate::object::{Advertisement, ObjectCatalog, SmartObject};
use crate::systems::decision::effects::{apply as apply_effect, EffectTarget};
use crate::time::CurrentTick;

/// Run the execute phase of the decision runtime: complete any agent
/// whose committed action has reached its `expected_end_tick`.
///
/// For object-targeted actions: applies the ad's effects, pushes a
/// `RecentActionEntry`, clears `CurrentAction`. For self-actions
/// (`Idle`/`Wait`): clears `CurrentAction` only.
pub(crate) fn execute(
    catalog: Res<ObjectCatalog>,
    current_tick: Res<CurrentTick>,
    objects: Query<&SmartObject>,
    mut agents: Query<(
        &mut Needs,
        &mut Mood,
        &mut RecentActionsRing,
        &mut CurrentAction,
    )>,
) {
    for (mut needs, mut mood, mut ring, mut current) in &mut agents {
        let Some(action) = &current.0 else {
            continue;
        };
        if current_tick.0 < action.expected_end_tick {
            continue;
        }

        match action.action {
            ActionRef::Object { object, ad } => {
                if let Some((type_id, advertisement)) =
                    lookup_advertisement(&catalog, &objects, object, ad)
                {
                    let mut target = EffectTarget {
                        needs: &mut needs,
                        mood: &mut mood,
                    };
                    for effect in &advertisement.effects {
                        apply_effect(effect, &mut target);
                    }
                    ring.push(RecentActionEntry {
                        ad_template: (type_id, ad),
                        completed_tick: current_tick.0,
                    });
                } else {
                    tracing::warn!(
                        ?object,
                        ?ad,
                        "decision::execute: advertisement not found in catalog; clearing action"
                    );
                }
            }
            ActionRef::SelfAction(_) => {
                // No effects, no ring entry — just clear.
            }
        }
        current.0 = None;
    }
}

/// Resolve `(ObjectId, AdvertisementId)` to `(ObjectTypeId, &Advertisement)`
/// via the world's smart-object instances and the catalog.
fn lookup_advertisement<'a>(
    catalog: &'a ObjectCatalog,
    objects: &Query<&SmartObject>,
    object_id: ObjectId,
    ad_id: AdvertisementId,
) -> Option<(crate::ids::ObjectTypeId, &'a Advertisement)> {
    let object = objects.iter().find(|o| o.id == object_id)?;
    let object_type = catalog.by_id.get(&object.type_id)?;
    let advertisement = object_type.advertisements.iter().find(|a| a.id == ad_id)?;
    Some((object.type_id, advertisement))
}
```

- [ ] **Step 4.6: Run the unit tests**

```bash
cargo test -p gecko-sim-core systems::decision::execute
```

Expected: all 4 tests pass.

- [ ] **Step 4.7: Verify the workspace still builds clean**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. `execute` is defined but not yet registered in `Sim`'s schedule (Task 6). Add `#[allow(dead_code)]` to `pub(crate) fn execute` if clippy fires; remove in Task 6:

```rust
#[allow(dead_code, reason = "registered in Sim::new in Task 6")]
pub(crate) fn execute(...)
```

- [ ] **Step 4.8: Confirm commit scope**

```bash
jj st
```

Expected: 1 modified (`crates/core/src/systems/decision/mod.rs`) + 1 new (`crates/core/src/systems/decision/execute.rs`).

---

## Task 5: `decide` system + unit tests

**Files:**
- Create: `crates/core/src/systems/decision/decide.rs`
- Modify: `crates/core/src/systems/decision/mod.rs` (declare submodule)

The `decide` system fires for every agent with `current_action = None`: builds the candidate list, filters by predicates, scores, picks weighted-random from top-3, commits the winner. Falls back to `SelfAction(Idle)` when no candidates survive.

- [ ] **Step 5.1: Start the task commit**

```bash
jj new -m "Decision: decide system + unit tests"
```

- [ ] **Step 5.2: Declare the submodule in `crates/core/src/systems/decision/mod.rs`**

Add `pub mod decide;` after the existing module declarations:

```rust
pub mod decide;
pub mod effects;
pub mod execute;
pub mod predicates;
pub mod scoring;
```

- [ ] **Step 5.3: Write the failing tests**

Create `crates/core/src/systems/decision/decide.rs`:

```rust
//! ECS system: decide. For each agent without a current action, builds
//! the candidate-advertisement list, filters by predicates, scores the
//! survivors, picks weighted-random from top-N, and commits.
//!
//! Falls back to `SelfAction(Idle)` (with `IDLE_DURATION_TICKS = 5`) when
//! no advertisements survive predicate filtering.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::agent::{Mood, Need, Needs, Personality};
    use crate::decision::{
        ActionRef, CurrentAction, IDLE_DURATION_TICKS, Phase, RecentActionsRing, SelfActionKind,
    };
    use crate::ids::{AdvertisementId, LeafAreaId, ObjectId, ObjectTypeId};
    use crate::object::{
        Advertisement, Effect, InterruptClass, MeshId, ObjectCatalog, ObjectType, Op, Predicate,
        ScoreTemplate, SmartObject, StateValue,
    };
    use crate::sim::SimRngResource;
    use crate::systems::decision::decide::decide;
    use crate::time::CurrentTick;
    use crate::world::Vec2;
    use crate::rng::PrngState;

    fn fridge_object_type() -> ObjectType {
        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        ObjectType {
            id: ObjectTypeId::new(1),
            display_name: "Fridge".to_string(),
            mesh_id: MeshId(1),
            default_state: state,
            advertisements: vec![Advertisement {
                id: AdvertisementId::new(1),
                display_name: "Eat snack".to_string(),
                preconditions: vec![Predicate::AgentNeed(Need::Hunger, Op::Lt, 0.6)],
                effects: vec![Effect::AgentNeedDelta(Need::Hunger, 0.4)],
                duration_ticks: 10,
                interrupt_class: InterruptClass::NeedsThresholdOnly,
                score_template: ScoreTemplate {
                    need_weights: vec![(Need::Hunger, 1.0)],
                    personality_weights: Personality::default(),
                    situational_modifiers: vec![],
                },
            }],
        }
    }

    fn build_world(agent_needs: Needs) -> (World, bevy_ecs::entity::Entity) {
        let mut world = World::new();
        let fridge = fridge_object_type();
        let mut object_types = HashMap::new();
        object_types.insert(fridge.id, fridge);
        world.insert_resource(ObjectCatalog { by_id: object_types });
        world.insert_resource(CurrentTick(0));
        world.insert_resource(SimRngResource(PrngState::from_seed(42)));

        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        world.spawn(SmartObject {
            id: ObjectId::new(0),
            type_id: ObjectTypeId::new(1),
            location: LeafAreaId::DEFAULT,
            position: Vec2::ZERO,
            owner: None,
            state,
        });

        let agent = world
            .spawn((
                agent_needs,
                Mood::neutral(),
                Personality::default(),
                CurrentAction::default(),
                RecentActionsRing::default(),
            ))
            .id();
        (world, agent)
    }

    #[test]
    fn hungry_agent_commits_eat_snack() {
        let (mut world, agent) = build_world(Needs {
            hunger: 0.3,
            ..Needs::full()
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(decide);
        schedule.run(&mut world);

        let current = world.get::<CurrentAction>(agent).unwrap();
        let action = current.0.as_ref().expect("CurrentAction should be set");
        match action.action {
            ActionRef::Object { object, ad } => {
                assert_eq!(object, ObjectId::new(0));
                assert_eq!(ad, AdvertisementId::new(1));
            }
            _ => panic!("expected Object action"),
        }
        assert_eq!(action.started_tick, 0);
        assert_eq!(action.expected_end_tick, 10); // duration_ticks = 10
        assert_eq!(action.phase, Phase::Performing);
    }

    #[test]
    fn full_needs_agent_falls_back_to_idle() {
        // hunger = 1.0 > 0.6 → AgentNeed(Hunger, Lt, 0.6) fails → no candidates.
        let (mut world, agent) = build_world(Needs::full());
        let mut schedule = Schedule::default();
        schedule.add_systems(decide);
        schedule.run(&mut world);

        let current = world.get::<CurrentAction>(agent).unwrap();
        let action = current.0.as_ref().expect("CurrentAction should be set");
        match action.action {
            ActionRef::SelfAction(kind) => {
                assert_eq!(kind, SelfActionKind::Idle);
            }
            _ => panic!("expected SelfAction(Idle), got {:?}", action.action),
        }
        assert_eq!(action.expected_end_tick, IDLE_DURATION_TICKS as u64);
    }

    #[test]
    fn agent_with_existing_action_is_skipped() {
        let (mut world, agent) = build_world(Needs {
            hunger: 0.3,
            ..Needs::full()
        });
        // Pre-commit a different action.
        world
            .get_mut::<CurrentAction>(agent)
            .unwrap()
            .0 = Some(crate::decision::CommittedAction {
                action: ActionRef::SelfAction(SelfActionKind::Wait),
                started_tick: 0,
                expected_end_tick: 100,
                phase: Phase::Performing,
                target_position: None,
            });

        let mut schedule = Schedule::default();
        schedule.add_systems(decide);
        schedule.run(&mut world);

        // Action unchanged.
        let action = world.get::<CurrentAction>(agent).unwrap().0.as_ref().unwrap();
        assert!(matches!(
            action.action,
            ActionRef::SelfAction(SelfActionKind::Wait)
        ));
    }
}
```

- [ ] **Step 5.4: Run the failing test**

```bash
cargo test -p gecko-sim-core systems::decision::decide
```

Expected: compile error — `decide` function not found.

- [ ] **Step 5.5: Implement `decide`**

Add to the top of `crates/core/src/systems/decision/decide.rs`:

```rust
use bevy_ecs::system::{Query, Res, ResMut};
use rand::Rng;

use crate::agent::{Mood, Needs, Personality};
use crate::decision::{
    ActionRef, CommittedAction, CurrentAction, IDLE_DURATION_TICKS, Phase, RecentActionsRing,
    SelfActionKind,
};
use crate::ids::{AdvertisementId, ObjectId, ObjectTypeId};
use crate::object::{Advertisement, ObjectCatalog, SmartObject};
use crate::sim::SimRngResource;
use crate::systems::decision::predicates::{evaluate, EvalContext};
use crate::systems::decision::scoring::{
    base_utility, mood_modifier, personality_modifier, recency_penalty, weighted_pick,
};
use crate::time::CurrentTick;

/// Pick the top-N highest-scoring candidates before weighted-pick.
const TOP_N: usize = 3;

/// Noise scale: each candidate's score gets a uniform `[0, NOISE_SCALE)`
/// addition. Per ADR 0011 this lets equal-scoring candidates break ties
/// stochastically.
const NOISE_SCALE: f32 = 0.1;

/// Run the decide phase: for each agent with no current action, choose
/// the next action via the v0 utility-AI scorer.
pub(crate) fn decide(
    catalog: Res<ObjectCatalog>,
    current_tick: Res<CurrentTick>,
    mut sim_rng: ResMut<SimRngResource>,
    objects: Query<&SmartObject>,
    mut agents: Query<(
        &Needs,
        &Mood,
        &Personality,
        &RecentActionsRing,
        &mut CurrentAction,
    )>,
) {
    // PrngState wraps Pcg64Mcg in a tuple struct; reach into the inner
    // RNG (which implements rand::Rng via the blanket impl on RngCore).
    let rng = &mut sim_rng.0.0;
    for (needs, mood, personality, ring, mut current) in &mut agents {
        if current.0.is_some() {
            continue;
        }
        let next = pick_next_action(
            needs,
            mood,
            personality,
            ring,
            &catalog,
            &objects,
            current_tick.0,
            rng,
        );
        current.0 = Some(next);
    }
}

#[allow(clippy::too_many_arguments)]
fn pick_next_action<R: Rng + ?Sized>(
    needs: &Needs,
    mood: &Mood,
    personality: &Personality,
    ring: &RecentActionsRing,
    catalog: &ObjectCatalog,
    objects: &Query<&SmartObject>,
    current_tick: u64,
    rng: &mut R,
) -> CommittedAction {
    // Build candidates filtered by predicates and scored.
    let mut scored: Vec<(ObjectId, ObjectTypeId, AdvertisementId, u32, f32)> = Vec::new();
    for object in objects.iter() {
        let Some(object_type) = catalog.by_id.get(&object.type_id) else {
            continue;
        };
        for ad in &object_type.advertisements {
            let ctx = EvalContext {
                needs,
                object_state: &object.state,
            };
            if !ad.preconditions.iter().all(|p| evaluate(p, &ctx)) {
                continue;
            }
            let score =
                score_advertisement(needs, mood, personality, ring, object_type.id, ad, rng);
            scored.push((object.id, object_type.id, ad.id, ad.duration_ticks, score));
        }
    }

    // Sort descending by score; truncate to top-N.
    scored.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(TOP_N);

    if scored.is_empty() {
        return CommittedAction {
            action: ActionRef::SelfAction(SelfActionKind::Idle),
            started_tick: current_tick,
            expected_end_tick: current_tick + IDLE_DURATION_TICKS as u64,
            phase: Phase::Performing,
            target_position: None,
        };
    }

    let id_score: Vec<(AdvertisementId, f32)> = scored
        .iter()
        .map(|(_, _, ad_id, _, score)| (*ad_id, *score))
        .collect();
    let picked_ad = weighted_pick(&id_score, rng).expect("non-empty after early return");
    let (object_id, _type_id, _ad_id, duration_ticks, _score) = scored
        .into_iter()
        .find(|(_, _, ad_id, _, _)| *ad_id == picked_ad)
        .expect("picked id is from the scored list");

    CommittedAction {
        action: ActionRef::Object {
            object: object_id,
            ad: picked_ad,
        },
        started_tick: current_tick,
        expected_end_tick: current_tick + duration_ticks as u64,
        phase: Phase::Performing,
        target_position: None,
    }
}

fn score_advertisement<R: Rng + ?Sized>(
    needs: &Needs,
    mood: &Mood,
    personality: &Personality,
    ring: &RecentActionsRing,
    type_id: ObjectTypeId,
    ad: &Advertisement,
    rng: &mut R,
) -> f32 {
    let base = base_utility(needs, &ad.score_template);
    let pers = personality_modifier(personality, &ad.score_template.personality_weights);
    let md = mood_modifier(mood, &ad.score_template.situational_modifiers);
    let pen = recency_penalty(ring, type_id, ad.id);
    let noise = rng.gen::<f32>() * NOISE_SCALE;
    base * pers * md * (1.0 - pen) + noise
}
```

- [ ] **Step 5.6: Run the unit tests**

```bash
cargo test -p gecko-sim-core systems::decision::decide
```

Expected: all 3 tests pass.

- [ ] **Step 5.7: Verify the workspace still builds clean**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. `decide` may need `#[allow(dead_code, reason = "registered in Sim::new in Task 6")]` until Task 6 wires it into the schedule.

- [ ] **Step 5.8: Confirm commit scope**

```bash
jj st
```

Expected: 1 modified (`crates/core/src/systems/decision/mod.rs`) + 1 new (`crates/core/src/systems/decision/decide.rs`).

---

## Task 6: Register systems + `spawn_test_object` + integration test

**Files:**
- Modify: `crates/core/src/sim.rs` (schedule registration, spawn helpers)
- Modify: `crates/core/src/systems/decision/{decide,execute}.rs` (drop dead_code allows)
- Modify: `crates/core/src/ids.rs` (drop DEFAULT allow if Task 1 added one)
- Create: `crates/core/tests/decision.rs`
- Create: `crates/core/tests/common/mod.rs`

This task wires the two systems into `Sim`'s schedule with the canonical `(needs::decay, mood::update, decision::execute, decision::decide).chain()` order, adds the `spawn_test_object` and `spawn_one_of_each_object_type` helpers, and lands the integration test that exercises the full vertical slice through the real seed catalog.

- [ ] **Step 6.1: Start the task commit**

```bash
jj new -m "Decision: register systems in schedule; spawn_test_object; integration test"
```

- [ ] **Step 6.2: Drop the `dead_code` allows from Tasks 4 and 5**

In `crates/core/src/systems/decision/execute.rs`, find the `pub(crate) fn execute(...)` declaration. If it's preceded by `#[allow(dead_code, reason = "registered in Sim::new in Task 6")]`, delete that attribute line.

Same for `crates/core/src/systems/decision/decide.rs` and `pub(crate) fn decide(...)`.

If Task 1's `LeafAreaId::DEFAULT` carried a similar `#[allow(dead_code)]`, also drop it now (it'll be used by `spawn_test_object`).

- [ ] **Step 6.3: Write the failing integration test**

Create `crates/core/tests/common/mod.rs`:

```rust
//! Shared helpers for `crates/core/tests/*.rs` integration tests.

use std::path::PathBuf;

use gecko_sim_core::ContentBundle;

/// Resolve the workspace-root `content/` directory and load the seed
/// catalog. Equivalent to `gecko-sim-content::load_from_dir(<workspace>/content)`.
pub fn seed_content_bundle() -> ContentBundle {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("content");
    gecko_sim_content::load_from_dir(&root)
        .unwrap_or_else(|e| panic!("loading seed content from {}: {e}", root.display()))
}
```

Create `crates/core/tests/decision.rs`:

```rust
//! Integration test for the decision-runtime v0: agents pick + execute
//! advertisements end-to-end through `Sim::tick`.

mod common;

use gecko_sim_core::ids::LeafAreaId;
use gecko_sim_core::{Needs, Sim, Vec2};

#[test]
fn agent_eats_from_fridge_when_hungry() {
    let mut sim = Sim::new(0, common::seed_content_bundle());
    sim.spawn_test_agent_with_needs(
        "Hungry",
        Needs {
            hunger: 0.3,
            ..Needs::full()
        },
    );
    sim.spawn_one_of_each_object_type(LeafAreaId::DEFAULT, Vec2::ZERO);

    // The fridge ad takes 10 ticks. The first tick decides; ticks 2-11
    // execute; tick 11 completes (since started_tick=1 and duration=10
    // means expected_end_tick=11). Run an extra few ticks for slack.
    for _ in 0..15 {
        sim.tick();
    }

    let snap = sim.snapshot();
    let agent = &snap.agents[0];
    assert!(
        agent.needs.hunger > 0.6,
        "hunger restored from 0.3 to {}",
        agent.needs.hunger
    );
}
```

This test depends on `Sim::spawn_one_of_each_object_type`, which we add in step 6.4. Also requires `gecko-sim-content` as a `[dev-dependencies]` of `crates/core`. Check `crates/core/Cargo.toml`:

```bash
grep 'gecko-sim-content' crates/core/Cargo.toml || echo "NOT PRESENT"
```

If not present, add to `crates/core/Cargo.toml` `[dev-dependencies]`:

```toml
gecko-sim-content.workspace = true
```

- [ ] **Step 6.4: Run the failing test**

```bash
cargo test -p gecko-sim-core --test decision
```

Expected: compile error — `Sim::spawn_one_of_each_object_type` doesn't exist.

- [ ] **Step 6.5: Wire systems into the schedule and add spawn helpers in `crates/core/src/sim.rs`**

Three edits:

**Edit A.** Update the schedule construction in `Sim::new`. Find:

```rust
let mut schedule = Schedule::default();
schedule.add_systems((systems::needs::decay, systems::mood::update).chain());
```

Replace with:

```rust
let mut schedule = Schedule::default();
schedule.add_systems(
    (
        systems::needs::decay,
        systems::mood::update,
        systems::decision::execute::execute,
        systems::decision::decide::decide,
    )
        .chain(),
);
```

**Edit B.** Add the `spawn_test_object` and `spawn_one_of_each_object_type` methods. Below the existing `spawn_test_agent_with_needs` method, append:

```rust
    /// Spawn a smart-object instance of the given catalog type. Test-only
    /// entry point until content-driven instance spawning lands.
    /// Reads the type's `default_state` from the catalog and stamps it on
    /// the new instance. Returns the freshly allocated `ObjectId`.
    ///
    /// Panics if `type_id` is not in the loaded `ObjectCatalog`.
    pub fn spawn_test_object(
        &mut self,
        type_id: ObjectTypeId,
        location: crate::ids::LeafAreaId,
        position: crate::world::Vec2,
    ) -> crate::ids::ObjectId {
        let id = crate::ids::ObjectId::new(self.next_object_id);
        self.next_object_id += 1;
        let default_state = self
            .world
            .resource::<ObjectCatalog>()
            .by_id
            .get(&type_id)
            .unwrap_or_else(|| panic!("ObjectTypeId {type_id:?} not in catalog"))
            .default_state
            .clone();
        self.world.spawn(crate::object::SmartObject {
            id,
            type_id,
            location,
            position,
            owner: None,
            state: default_state,
        });
        id
    }

    /// Spawn one instance of every loaded `ObjectType`. Convenience for
    /// the host's seed-instance spawn at startup.
    pub fn spawn_one_of_each_object_type(
        &mut self,
        location: crate::ids::LeafAreaId,
        position: crate::world::Vec2,
    ) -> Vec<crate::ids::ObjectId> {
        let type_ids: Vec<ObjectTypeId> = self
            .world
            .resource::<ObjectCatalog>()
            .by_id
            .keys()
            .copied()
            .collect();
        type_ids
            .into_iter()
            .map(|t| self.spawn_test_object(t, location, position))
            .collect()
    }
```

**Edit C.** Add the necessary imports at the top of the file. Update the `crate::ids` import line to include `ObjectId`. Find:

```rust
use crate::ids::{AccessoryId, AgentId, ObjectTypeId};
```

Replace with:

```rust
use crate::ids::{AccessoryId, AgentId, LeafAreaId, ObjectId, ObjectTypeId};
```

(The `LeafAreaId` and `ObjectId` imports are useful for the host wiring in Task 8.)

Determinism: the iteration order over `type_ids` from `HashMap::keys()` is non-deterministic across runs. For Task 6's integration test we need `spawn_one_of_each_object_type` to produce **deterministic ObjectIds** for the same content bundle. **Sort the type_ids before iterating:**

```rust
let mut type_ids: Vec<ObjectTypeId> = self
    .world
    .resource::<ObjectCatalog>()
    .by_id
    .keys()
    .copied()
    .collect();
type_ids.sort();  // <-- add this line
```

(The `id_newtype!` macro derives `Ord` on `ObjectTypeId`, so this works directly.)

- [ ] **Step 6.6: Run the integration test**

```bash
cargo test -p gecko-sim-core --test decision
```

Expected: passes. The hungry agent commits `EatSnack` on the first tick, completes 10 ticks later, and `hunger` rises by 0.4 (from 0.3 to 0.7).

- [ ] **Step 6.7: Run all existing tests — they must still pass**

```bash
cargo test --workspace
```

Expected: all clean. The `decide` and `execute` systems run every tick now, but agents in existing tests have no smart objects to interact with → they fall back to `Idle` indefinitely. `tests/needs_decay.rs` continues to assert needs decay (decision systems don't touch needs without effects), `tests/mood.rs` continues to assert mood drift (same), `tests/determinism.rs` continues to pass (sorted type_ids + deterministic query iteration + global RNG = byte-equal snapshots).

- [ ] **Step 6.8: Verify clippy clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 6.9: Confirm commit scope**

```bash
jj st
```

Expected: 3-4 files modified (`crates/core/src/sim.rs`, `decision/{decide,execute}.rs`, `ids.rs` if it had an allow), plus 2 new test files (`crates/core/tests/decision.rs` and `crates/core/tests/common/mod.rs`), plus possibly `crates/core/Cargo.toml` if `gecko-sim-content` was added.

---

## Task 7: `CurrentActionView` wire type + `AgentSnapshot.current_action` + ts-rs regen + protocol roundtrip

**Files:**
- Modify: `crates/core/src/snapshot.rs` (add `CurrentActionView`; `AgentSnapshot` grows `current_action`)
- Modify: `crates/core/src/sim.rs` (snapshot fn projects `CurrentActionView`)
- Modify: `crates/core/src/lib.rs` (re-export `CurrentActionView`)
- Modify: `crates/protocol/tests/roundtrip.rs` (fixtures)
- Create (regen): `apps/web/src/types/sim/CurrentActionView.ts`
- Modify (regen): `apps/web/src/types/sim/AgentSnapshot.ts`

This task carries the schema change to the wire. After it lands, the host serves snapshots that include each agent's current action; the frontend will consume it in Task 9.

**Important note on transient state:** This task intentionally leaves `pnpm tsc --noEmit` failing because the frontend reducer test fixture doesn't yet include `current_action`. Task 9 closes the loop. Rust gates remain green throughout.

- [ ] **Step 7.1: Start the task commit**

```bash
jj new -m "Decision: CurrentActionView wire type; AgentSnapshot.current_action; ts-rs regen; protocol roundtrip"
```

- [ ] **Step 7.2: Add `CurrentActionView` and grow `AgentSnapshot` in `crates/core/src/snapshot.rs`**

Replace the file's contents with:

```rust
//! Snapshot type: a deterministic, by-value view of sim state at a tick.
//!
//! Agents are sorted by `AgentId` ascending so two `Sim` instances built
//! from the same seed and same calls produce byte-equal `Snapshot`s.
//! Serde derives let `Snapshot` ride directly on the wire (per ADR 0013
//! and the WS transport v0 spec — wire types live in `protocol`, but the
//! `Snapshot` shape itself is the schema-of-record from `core`).

use serde::{Deserialize, Serialize};

use crate::agent::{Mood, Needs};
use crate::ids::AgentId;

/// Lossy projection of `CommittedAction` for the wire. Carries enough for
/// the frontend to render "Alice is doing X (50%)". The full
/// `CommittedAction` lives only as an ECS component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct CurrentActionView {
    /// `Advertisement.display_name` for object-targeted actions, or
    /// `"Idle"` / `"Wait"` for self-actions.
    pub display_name: String,
    /// Progress through the action's `duration_ticks`. `0.0` at start,
    /// rises monotonically toward `1.0` at scheduled completion.
    pub fraction_complete: f32,
}

/// Full sim state at a tick boundary. `PartialEq` is required by the
/// determinism test in the test suite; serde derives let `protocol`
/// envelope this type without a parallel wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Snapshot {
    #[cfg_attr(feature = "export-ts", ts(type = "number"))]
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
}

/// Per-agent snapshot row. Holds the slice of state this pass exposes;
/// other groupings (Personality, Memory, Spatial, …) extend this type as
/// their first consumer system lands.
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
    pub current_action: Option<CurrentActionView>,
}

#[cfg(test)]
mod serde_derive_tests {
    use super::{AgentSnapshot, CurrentActionView, Snapshot};

    fn assert_serialize<T: serde::Serialize>() {}
    fn assert_deserialize<T: serde::de::DeserializeOwned>() {}

    #[test]
    fn snapshot_types_implement_serde() {
        assert_serialize::<Snapshot>();
        assert_deserialize::<Snapshot>();
        assert_serialize::<AgentSnapshot>();
        assert_deserialize::<AgentSnapshot>();
        assert_serialize::<CurrentActionView>();
        assert_deserialize::<CurrentActionView>();
    }
}
```

- [ ] **Step 7.3: Update `Sim::snapshot` to project `CurrentActionView`**

In `crates/core/src/sim.rs`, find the `snapshot` method's `filter_map` and grow it. Currently:

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

Replace with:

```rust
.filter_map(|entity_ref| {
    let identity = entity_ref.get::<Identity>()?;
    let needs = entity_ref.get::<Needs>()?;
    let mood = entity_ref.get::<Mood>()?;
    let current_action_component = entity_ref.get::<CurrentAction>();
    let current_action = current_action_component
        .and_then(|c| c.0.as_ref())
        .and_then(|action| project_current_action(action, self.tick, self));
    Some(AgentSnapshot {
        id: identity.id,
        name: identity.name.clone(),
        needs: *needs,
        mood: *mood,
        current_action,
    })
})
```

Add the projection helper at the bottom of `sim.rs` (after the `impl Sim` block):

```rust
/// Build a `CurrentActionView` from a `CommittedAction`. Looks up the
/// advertisement's `display_name` via the catalog (for object-targeted
/// actions); falls back to `"Idle"` / `"Wait"` for self-actions.
/// Returns `None` only if the catalog lookup fails for an object action,
/// which would indicate a data-flow bug — we log and produce `None`.
fn project_current_action(
    action: &crate::decision::CommittedAction,
    current_tick: u64,
    sim: &Sim,
) -> Option<crate::snapshot::CurrentActionView> {
    let duration = action
        .expected_end_tick
        .saturating_sub(action.started_tick) as f32;
    let elapsed = current_tick.saturating_sub(action.started_tick) as f32;
    let fraction_complete = if duration > 0.0 {
        (elapsed / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let display_name = match action.action {
        crate::decision::ActionRef::SelfAction(crate::decision::SelfActionKind::Idle) => {
            "Idle".to_string()
        }
        crate::decision::ActionRef::SelfAction(crate::decision::SelfActionKind::Wait) => {
            "Wait".to_string()
        }
        crate::decision::ActionRef::Object { object, ad } => {
            // Look up the smart-object instance to get its type, then
            // the catalog's advertisement display_name.
            let mut iter = sim.world.iter_entities();
            let object_entry = iter
                .find(|e| e.get::<crate::object::SmartObject>().is_some_and(|o| o.id == object))?;
            let smart_object = object_entry.get::<crate::object::SmartObject>()?;
            let object_type = sim
                .world
                .resource::<ObjectCatalog>()
                .by_id
                .get(&smart_object.type_id)?;
            let advertisement = object_type.advertisements.iter().find(|a| a.id == ad)?;
            advertisement.display_name.clone()
        }
    };
    Some(crate::snapshot::CurrentActionView {
        display_name,
        fraction_complete,
    })
}
```

The helper is a free function rather than an `impl Sim` method so it can borrow `&Sim` non-uniquely from inside the closure passed to `filter_map`.

Also add `use crate::decision::CurrentAction;` to the `sim.rs` imports if not already there (it was added in Task 2).

- [ ] **Step 7.4: Re-export `CurrentActionView` from `crates/core/src/lib.rs`**

Find the existing `pub use snapshot::{...};` line. Add `CurrentActionView`:

```rust
pub use snapshot::{AgentSnapshot, CurrentActionView, Snapshot};
```

- [ ] **Step 7.5: Update `crates/protocol/tests/roundtrip.rs` fixture**

Find the `sample_snapshot_with_agents` function. Currently:

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
            current_action: None,
        })
        .collect();
    Snapshot { tick: 7, agents }
}
```

Add a new round-trip test at the bottom of the file:

```rust
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
            current_action: Some(CurrentActionView {
                display_name: "Eat snack".to_string(),
                fraction_complete: 0.5,
            }),
        }],
    };
    roundtrip(&snap);
}
```

(The existing `Mood` import should already be there from the mood pass; if not, add `use gecko_sim_core::agent::Mood;`.)

- [ ] **Step 7.6: Run all Rust tests**

```bash
cargo test --workspace
```

Expected: all green. The integration test from Task 6 (`tests/decision.rs`) now sees `agent.current_action` populated; existing tests work because `current_action` adds an additive field.

- [ ] **Step 7.7: Regenerate the ts-rs bindings**

```bash
cargo test -p gecko-sim-core --features export-ts
cargo test -p gecko-sim-protocol --features export-ts
```

Expected: passes; writes `apps/web/src/types/sim/CurrentActionView.ts` (new) and updates `apps/web/src/types/sim/AgentSnapshot.ts`.

- [ ] **Step 7.8: Verify the typed bindings emitted correctly**

```bash
cat apps/web/src/types/sim/CurrentActionView.ts
cat apps/web/src/types/sim/AgentSnapshot.ts
```

Expected:
- `CurrentActionView.ts`: `export type CurrentActionView = { display_name: string, fraction_complete: number, };`
- `AgentSnapshot.ts`: includes `current_action: CurrentActionView | null,`.

- [ ] **Step 7.9: Note the transient pnpm tsc state**

`pnpm tsc --noEmit` will FAIL after Task 7 because `apps/web/src/lib/sim/reducer.test.ts`'s fixture builds `AgentSnapshot` literals without `current_action`. Task 9 fixes this. **Do not run pnpm tsc here.**

- [ ] **Step 7.10: Verify Rust workspace still clean**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both clean.

- [ ] **Step 7.11: Confirm commit scope**

```bash
jj st
```

Expected: 4-5 modified Rust files (snapshot.rs, sim.rs, lib.rs, protocol/tests/roundtrip.rs) + 1 modified TS (`AgentSnapshot.ts`) + 1 new TS (`CurrentActionView.ts`).

---

## Task 8: Host spawns seed instances at startup

**Files:**
- Modify: `crates/host/src/main.rs`

After `Sim::new` and the existing `spawn_test_agent` calls, spawn one of each loaded object type into `LeafAreaId::DEFAULT` so the demo client has objects to interact with.

**Note on transient state:** `pnpm tsc` is still failing after this task (Task 7 left it that way). Task 9 closes it.

- [ ] **Step 8.1: Start the task commit**

```bash
jj new -m "Decision: host spawns seed instances at startup"
```

- [ ] **Step 8.2: Update `crates/host/src/main.rs`**

Find the existing block:

```rust
    let mut sim = Sim::new(DEMO_SEED, content);
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");

    let initial = sim.snapshot();
    tracing::info!(agents = initial.agents.len(), "sim primed");
```

Replace with:

```rust
    let mut sim = Sim::new(DEMO_SEED, content);
    sim.spawn_test_agent("Alice");
    sim.spawn_test_agent("Bob");
    sim.spawn_test_agent("Charlie");
    let object_ids = sim.spawn_one_of_each_object_type(
        gecko_sim_core::ids::LeafAreaId::DEFAULT,
        gecko_sim_core::Vec2::ZERO,
    );
    tracing::info!(object_count = object_ids.len(), "seed instances spawned");

    let initial = sim.snapshot();
    tracing::info!(agents = initial.agents.len(), "sim primed");
```

(Inline-imports the path-qualified types so we don't need to grow the `use` block. If the existing imports already cover `LeafAreaId` and `Vec2`, simplify to bare names.)

- [ ] **Step 8.3: Verify the host builds and tests pass**

```bash
cargo build -p gecko-sim-host
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. `tests/ws_smoke.rs` still passes — it asserts on tick advancement and agent count, not on object-side state.

- [ ] **Step 8.4: Manual smoke (host startup logs)**

```bash
timeout 3s cargo run -p gecko-sim-host 2>&1 | head -20 || true
```

Expected output includes:

```
gecko-sim host v0.1.0
loading content path=…/geckosim/crates/host/../../content
content loaded object_types=2 accessories=2
seed instances spawned object_count=2
sim primed agents=3
ws transport listening local_addr=127.0.0.1:9001
```

- [ ] **Step 8.5: Confirm commit scope**

```bash
jj st
```

Expected: 1 file modified — `crates/host/src/main.rs`.

---

## Task 9: Frontend "Doing" column + reducer fixture + new round-trip test

**Files:**
- Modify: `apps/web/src/components/AgentList.tsx`
- Modify: `apps/web/src/lib/sim/reducer.test.ts`

After Task 7, the frontend's `AgentSnapshot` type includes `current_action`, but the reducer test fixture doesn't, so `pnpm tsc` is failing. This task closes the loop and adds the visible column.

- [ ] **Step 9.1: Start the task commit**

```bash
jj new -m "Decision: frontend AgentList grows Doing column; reducer fixture updated"
```

- [ ] **Step 9.2: Update the reducer test fixture in `apps/web/src/lib/sim/reducer.test.ts`**

Find:

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
      current_action: null,
    },
  ],
});
```

- [ ] **Step 9.3: Add a new test asserting `current_action` round-trips**

Append to the bottom of the `describe("reduce", () => {` block (just before the closing `});`):

```ts
  it("init message preserves the current_action field on the snapshot", () => {
    const snap: Snapshot = {
      tick: 10,
      agents: [
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: 0, arousal: 0, stress: 0 },
          current_action: { display_name: "Eat snack", fraction_complete: 0.5 },
        },
      ],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 10, snapshot: snap },
    });
    expect(next.snapshot?.agents[0].current_action).toEqual({
      display_name: "Eat snack",
      fraction_complete: 0.5,
    });
  });
```

- [ ] **Step 9.4: Run the reducer tests**

```bash
cd apps/web && pnpm test
```

Expected: 7+ tests pass (existing 6 from prior passes + the new one).

- [ ] **Step 9.5: Update `apps/web/src/components/AgentList.tsx`**

Replace the file's contents with:

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
          <th className="px-2 py-1">Doing</th>
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
            <td className="px-2 py-1">
              {agent.current_action
                ? `${agent.current_action.display_name} (${(
                    agent.current_action.fraction_complete * 100
                  ).toFixed(0)}%)`
                : "—"}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
```

- [ ] **Step 9.6: Run the full frontend gate**

```bash
cd apps/web && pnpm tsc --noEmit
cd apps/web && pnpm lint
cd apps/web && pnpm test
cd apps/web && pnpm build
```

Expected: all clean.

- [ ] **Step 9.7: Manual end-to-end smoke**

In one terminal:

```bash
cargo run -p gecko-sim-host
```

In another:

```bash
cd apps/web && pnpm dev
```

Open http://localhost:3000.

For an autonomous-run verification (no real browser available), the lighter-tier:

```bash
cargo run -p gecko-sim-host > /tmp/host.log 2>&1 &
HOST_PID=$!
sleep 2
( cd apps/web && pnpm dev > /tmp/web.log 2>&1 ) &
WEB_PID=$!
sleep 8
curl -s http://localhost:3000 | grep -E "Doing|gecko-sim" | head -5
kill $WEB_PID $HOST_PID 2>/dev/null
wait 2>/dev/null
```

Expected: SSR HTML includes a `Doing` table header.

For a real browser smoke (preferred when running interactively):
1. Confirm the new `Doing` column renders. Initially shows `Idle (NN%)` for all three agents (full needs → no ad qualifies → fallback to Idle for 5 ticks → re-decide → Idle again).
2. Click `64×` to accelerate. Watch hunger drop below 0.6 — the agent's `Doing` column flips to `Eat snack (0%)`, climbs through the percentages, then drops back to Idle/Sit cycling.
3. Click `Pause / Resume` — the percentage stops advancing.

- [ ] **Step 9.8: Confirm commit scope**

```bash
jj st
```

Expected: 2 files modified — `apps/web/src/components/AgentList.tsx` and `apps/web/src/lib/sim/reducer.test.ts`.

---

## Definition of done (rolled-up gate)

After Task 9 lands:

- `cargo build --workspace` clean.
- `cargo test --workspace` clean — all existing tests pass; new unit tests under `systems::decision::*` (~25 cases) and the new integration test (`tests/decision.rs`) pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test -p gecko-sim-protocol --features export-ts` regenerates types; idempotent.
- `cargo test -p gecko-sim-core --features export-ts` regenerates types; idempotent.
- `cd apps/web && pnpm install --frozen-lockfile && pnpm test && pnpm tsc --noEmit && pnpm lint && pnpm build` clean.
- Manual smoke: agents cycle through `Idle` and `Eat snack` (and `Sit` once recency penalty kicks in) in the browser. `64×` accelerates the cycling visibly within seconds.
- 9-commit chain matching the commit-strategy section of the spec.

## Notes for the implementer

- **`#[allow(dead_code)]` lifecycle.** Tasks 4 and 5 may need this attribute on `pub(crate) fn execute` and `pub(crate) fn decide` because the workspace's `warnings = "deny"` lint flags unused functions. Task 6 removes it when registering the systems in `Sim::new`.
- **Determinism.** The integration test depends on byte-equal snapshots from the same seed. Two sources of nondeterminism to watch:
  1. `HashMap::keys()` iteration order in `spawn_one_of_each_object_type` — fixed by sorting the type IDs.
  2. `bevy_ecs` query iteration order — bevy's archetype storage is insertion-ordered for our single-thread case, so this is implicit.
- **Trait imports for `bevy_ecs::Schedule`.** `add_systems(...)` and `chain()` require `IntoSystemConfigs` (older bevy) or `IntoScheduleConfigs` (newer) in scope. The `Mood` pass already pinned this; `sim.rs` should already have the right import. If not, add `use bevy_ecs::schedule::IntoScheduleConfigs;`.
- **Task 7 leaves `pnpm tsc` failing on purpose.** The frontend reducer test fixture mismatches the new `AgentSnapshot` shape until Task 9 closes the loop. This is the same pattern the mood pass used.
- **The `seed_content_bundle()` helper in `tests/common/mod.rs`** mirrors the existing seed-content loading pattern from `crates/content/tests/seed_loads.rs`. Both files reach the workspace `content/` directory via `env!("CARGO_MANIFEST_DIR")`-relative paths.
- **Task 6's integration test takes ~15 ticks.** The first tick decides; ticks 1+10=11 sees the action complete (since `started_tick=1`, `expected_end_tick=11`). The extra 4-tick slack is defensive.
- **`tracing::warn!` in unsupported effect path.** Tests confirm no panic; the warn message is checked indirectly (we don't assert on log output without test infrastructure).
