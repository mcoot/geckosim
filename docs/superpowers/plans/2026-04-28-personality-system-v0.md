# Personality system v0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `Personality::default()` with `Personality::sample(rng)` at agent spawn so every agent has a distinct Big Five profile, propagate `personality` to the wire (`AgentSnapshot.personality`), and tweak the chair seed content so the score formula's `personality_modifier` produces visibly different action choices across agents.

**Architecture:** Five small commits. (1) `Personality::sample` constructor + ts-rs derives + unit tests. (2) `Sim::spawn_test_agent_with_needs` borrows `SimRngResource` and calls `Personality::sample`. (3) chair.ron's "Sit" ad gets `extraversion: -0.2` (introverts prefer sitting). (4) `AgentSnapshot.personality` field + ts-rs regen + protocol roundtrip. (5) Frontend `<AgentList>` gets five OCEAN columns + reducer fixture growth.

**Tech Stack:** Rust 2021, `bevy_ecs 0.16`, `ts-rs 10` (already wired), Next.js 16 + React 19 + Tailwind v4 + Vitest.

**Reference:** Spec at [`docs/superpowers/specs/2026-04-28-personality-system-design.md`](../specs/2026-04-28-personality-system-design.md). ADR 0010 (systems inventory; Personality is system #2 of 11), ADR 0011 (schema; Big Five sampled from "configured prior distribution, default roughly uniform, centered on zero"), ADR 0013 (transport).

**VCS note:** This repo uses `jj` (jujutsu), colocated with git. **Never use raw `git`.** Each task starts with `jj new -m "<task title>"`. There is no separate "commit" command.

---

## File Structure

**New files:** none. Pure additive changes to existing files.

**Modified files (Rust):**
- `crates/core/src/agent/mod.rs` — Task 1 (`Personality::sample` + ts-rs derives + unit tests)
- `crates/core/src/sim.rs` — Task 2 (`spawn_test_agent_with_needs` calls `Personality::sample`)
- `crates/core/src/snapshot.rs` — Task 4 (`AgentSnapshot.personality`)
- `crates/protocol/tests/roundtrip.rs` — Task 4 (fixture grows `personality`; new round-trip test)

**Modified files (content):**
- `content/object_types/chair.ron` — Task 3 (`extraversion: -0.2` on Sit ad)

**Modified files (frontend):**
- `apps/web/src/types/sim/Personality.ts` — Task 4 (auto-regen, new file)
- `apps/web/src/types/sim/AgentSnapshot.ts` — Task 4 (auto-regen, modified)
- `apps/web/src/components/AgentList.tsx` — Task 5 (5 OCEAN columns)
- `apps/web/src/lib/sim/reducer.test.ts` — Task 5 (fixture grows `personality`; new round-trip test)

**Existing tests untouched (work as-is):**
- `crates/core/tests/{snapshot,determinism,needs_decay,catalogs,mood,decision}.rs` — all use `Sim` public API; the additive `personality` ECS component and `AgentSnapshot.personality` field don't break any existing assertions. The `decision.rs` integration test seeds `hunger=0.3` and asserts hunger restoration; sampled personality doesn't change which ad wins because EatSnack's `personality_weights` stays all-zero.
- `crates/core/src/systems/decision/{decide,execute,scoring,predicates,effects}.rs` unit tests — all build worlds manually with `Personality::default()`, unaffected by the spawn-helper change.
- `crates/host/tests/ws_smoke.rs` — asserts on tick + agent count, not field values.
- `crates/content/tests/{loader_smoke,validation,seed_loads}.rs` — `seed_loads` asserts catalog counts (2 ObjectTypes, 2 Accessories), not specific advertisement field values; chair.ron tweak is invisible to it.

---

## Task 1: `Personality::sample` constructor + ts-rs derives + unit tests

**Files:**
- Modify: `crates/core/src/agent/mod.rs`

This task adds the ts-rs derives that Task 4 will need (so `Personality.ts` can be generated), and the `sample` constructor + unit tests. Pure additive — no consumers fire yet.

- [ ] **Step 1.1: Start the task commit**

```bash
jj new -m "Personality: ts-rs derives + Personality::sample(rng) + unit tests"
```

- [ ] **Step 1.2: Add ts-rs derives to `Personality`**

In `crates/core/src/agent/mod.rs`, find the `Personality` struct (around line 162):

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

Replace with:

```rust
/// Big Five personality components; each in `[-1, 1]`. Per ADR 0011.
/// Doubles as the ECS component (lazy sharding). Sampled at agent spawn
/// via `Personality::sample(rng)`; `Default` is retained for unit tests
/// that build hand-crafted worlds and want zero personality.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize,
)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Personality {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
}
```

(Two new `cfg_attr` lines and a doc-comment refresh.)

- [ ] **Step 1.3: Write the failing unit tests**

Append to `crates/core/src/agent/mod.rs` at the end of the file (or after the `Personality` struct if there's a logical spot):

```rust
#[cfg(test)]
mod personality_tests {
    use super::Personality;
    use rand::SeedableRng;

    #[test]
    fn sample_components_in_range() {
        let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(0);
        for _ in 0..1000 {
            let p = Personality::sample(&mut rng);
            for c in [
                p.openness,
                p.conscientiousness,
                p.extraversion,
                p.agreeableness,
                p.neuroticism,
            ] {
                assert!((-1.0..=1.0).contains(&c), "out of range: {c}");
            }
        }
    }

    #[test]
    fn same_seed_same_personality() {
        let mut a = rand_pcg::Pcg64Mcg::seed_from_u64(42);
        let mut b = rand_pcg::Pcg64Mcg::seed_from_u64(42);
        assert_eq!(Personality::sample(&mut a), Personality::sample(&mut b));
    }

    #[test]
    fn consecutive_samples_differ() {
        let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(0);
        let p1 = Personality::sample(&mut rng);
        let p2 = Personality::sample(&mut rng);
        assert_ne!(p1, p2);
    }
}
```

- [ ] **Step 1.4: Run the failing test**

```bash
cargo test -p gecko-sim-core agent::personality_tests
```

Expected: compile error — `Personality::sample` doesn't exist.

- [ ] **Step 1.5: Implement `Personality::sample`**

Add an `impl Personality` block immediately below the `Personality` struct (above the `#[cfg(test)]` block):

```rust
impl Personality {
    /// Sample a Big Five personality from the uniform distribution on
    /// `[-1, 1]^5` per ADR 0011's "roughly uniform, centered on zero"
    /// default. Five draws from the supplied RNG; deterministic for a
    /// fixed seed.
    pub fn sample<R: rand::Rng + ?Sized>(rng: &mut R) -> Self {
        Self {
            openness: rng.random_range(-1.0..=1.0),
            conscientiousness: rng.random_range(-1.0..=1.0),
            extraversion: rng.random_range(-1.0..=1.0),
            agreeableness: rng.random_range(-1.0..=1.0),
            neuroticism: rng.random_range(-1.0..=1.0),
        }
    }
}
```

- [ ] **Step 1.6: Run the tests to verify pass**

```bash
cargo test -p gecko-sim-core agent::personality_tests
```

Expected: 3 tests pass.

- [ ] **Step 1.7: Verify the workspace still builds clean**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all clean. The new `cfg_attr(feature = "export-ts", ...)` lines compile away at default features. With `--features export-ts`, ts-rs would emit `Personality.ts` — but Task 4 is the actual regen step.

- [ ] **Step 1.8: Verify `--features export-ts` build clean (no regen yet)**

```bash
cargo build -p gecko-sim-core --features export-ts
```

Expected: clean.

If clippy fires `dead_code` on `Personality::sample` because it's not yet called outside tests, add `#[allow(dead_code, reason = "called by spawn_test_agent_with_needs in Task 2")]` on the `impl` method. Drop the allow in Task 2.

- [ ] **Step 1.9: Confirm commit scope**

```bash
jj st
```

Expected: 1 file modified — `crates/core/src/agent/mod.rs`.

---

## Task 2: `spawn_test_agent_with_needs` samples Personality from `SimRngResource`

**Files:**
- Modify: `crates/core/src/sim.rs`
- Modify: `crates/core/src/agent/mod.rs` (drop dead_code allow on `Personality::sample` if Task 1 added one)

After this task, every agent spawned via `spawn_test_agent_with_needs` (and therefore via the public `spawn_test_agent`) has a uniformly-sampled personality. Determinism: spawn order is sequential and the global `SimRngResource` advances deterministically.

- [ ] **Step 2.1: Start the task commit**

```bash
jj new -m "Personality: spawn_test_agent_with_needs samples Personality from SimRngResource"
```

- [ ] **Step 2.2: Drop the dead_code allow on `Personality::sample` (if any)**

```bash
grep 'dead_code.*Personality::sample\|Task 2' crates/core/src/agent/mod.rs
```

If a `#[allow(dead_code, reason = "called by spawn_test_agent_with_needs in Task 2")]` line is present on the `impl Personality { pub fn sample ... }` block, delete that attribute line. (Skip this step if Task 1 didn't add one.)

- [ ] **Step 2.3: Update `spawn_test_agent_with_needs` in `crates/core/src/sim.rs`**

Find the existing method:

```rust
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
```

Replace with:

```rust
    /// Spawn a fresh agent with explicit initial needs, neutral mood,
    /// a sampled personality, and decision-runtime components (no current
    /// action, empty recent-actions ring). Test-only entry point.
    ///
    /// Personality is sampled from the world's `SimRngResource` so spawn
    /// order is deterministic from the seed (per ADR 0008).
    pub fn spawn_test_agent_with_needs(&mut self, name: &str, needs: Needs) -> AgentId {
        let id = AgentId::new(self.next_agent_id);
        self.next_agent_id += 1;
        let personality = {
            let mut rng = self.world.resource_mut::<SimRngResource>();
            Personality::sample(&mut rng.0.0)
        };
        self.world.spawn((
            Identity {
                id,
                name: name.to_string(),
            },
            needs,
            Mood::neutral(),
            personality,
            CurrentAction::default(),
            RecentActionsRing::default(),
        ));
        id
    }
```

The borrow of `SimRngResource` is scoped to a block so it's dropped before `world.spawn` (which also needs `&mut world`).

`SimRngResource` and `Personality` should already be in scope from prior passes; if `Personality` isn't imported, the existing `use crate::agent::{...}` line at the top of `sim.rs` already includes it via `Personality`.

- [ ] **Step 2.4: Run the workspace tests**

```bash
cargo test --workspace
```

Expected: all clean. The behavior change: existing test agents now have non-zero personalities, but no test asserts on personality values. The integration test `crates/core/tests/decision.rs::agent_eats_from_fridge_when_hungry` still passes — EatSnack's `personality_weights` is all-zero, so `personality_modifier == 1.0` regardless of sampled personality. The hungry agent still picks EatSnack, hunger still restores by 0.4.

If `tests/determinism.rs` or `tests/snapshot.rs` fails because of changed snapshots: revisit. Both tests build agents via `spawn_test_agent` (which now samples personality). Both compare snapshots; same seed → same sampled personalities → byte-equal snapshots. Should pass.

- [ ] **Step 2.5: Verify clippy clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean. The dead_code on `Personality::sample` is gone (`spawn_test_agent_with_needs` is the caller).

- [ ] **Step 2.6: Confirm commit scope**

```bash
jj st
```

Expected: 1 file modified (`crates/core/src/sim.rs`) + possibly 1 more if Task 1 added the dead_code allow that this task drops (`crates/core/src/agent/mod.rs`).

---

## Task 3: chair.ron `Sit` ad gets `extraversion: -0.2`

**Files:**
- Modify: `content/object_types/chair.ron`

Smallest possible content tweak that makes personality visible in scoring: introverts prefer sitting more than extraverts. With `sensitivity = 0.5`, the modifier is `(1.0 - 0.1 · extraversion)`, ranging `0.9` (extreme extravert) to `1.1` (extreme introvert).

- [ ] **Step 3.1: Start the task commit**

```bash
jj new -m "Personality: chair.ron Sit ad gets extraversion: -0.2 weight (introverts prefer sitting)"
```

- [ ] **Step 3.2: Edit `content/object_types/chair.ron`**

Find the `personality_weights:` block (around line 18):

```ron
                personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
```

Replace with:

```ron
                personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: -0.2, agreeableness: 0.0, neuroticism: 0.0,
                ),
```

(Single-character change: `0.0` → `-0.2` on the `extraversion:` field.)

- [ ] **Step 3.3: Verify the seed-loads test still passes**

```bash
cargo test -p gecko-sim-content --test seed_loads
```

Expected: passes. The test asserts catalog counts and ID presence, not specific advertisement field values. The chair tweak is invisible to it.

- [ ] **Step 3.4: Verify the host's seed-content load still works**

```bash
cargo test --workspace
```

Expected: all clean. The decision-runtime integration test (`tests/decision.rs::agent_eats_from_fridge_when_hungry`) still passes because the agent's hunger=0.3 makes EatSnack's `base_utility` dominate any chair-Sit competition; the modest 10% personality bias on chair doesn't flip the winner.

- [ ] **Step 3.5: Verify clippy clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 3.6: Confirm commit scope**

```bash
jj st
```

Expected: 1 file modified — `content/object_types/chair.ron`.

---

## Task 4: `AgentSnapshot.personality` + ts-rs regen + protocol roundtrip

**Files:**
- Modify: `crates/core/src/snapshot.rs`
- Modify: `crates/protocol/tests/roundtrip.rs`
- Create (regen): `apps/web/src/types/sim/Personality.ts`
- Modify (regen): `apps/web/src/types/sim/AgentSnapshot.ts`

After this task, the host serves snapshots that include each agent's personality; the frontend's reducer accepts it (covered in Task 5).

**Important note on transient state:** This task INTENTIONALLY leaves `pnpm tsc --noEmit` failing because `apps/web/src/lib/sim/reducer.test.ts`'s fixture builds `AgentSnapshot` literals without `personality`. Task 5 closes the loop. Rust gates remain green throughout.

- [ ] **Step 4.1: Start the task commit**

```bash
jj new -m "Personality: AgentSnapshot grows personality; ts-rs regen; protocol roundtrip fixtures updated"
```

- [ ] **Step 4.2: Add `personality` to `AgentSnapshot` in `crates/core/src/snapshot.rs`**

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
    pub mood: Mood,
    pub current_action: Option<CurrentActionView>,
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
    pub personality: Personality,
    pub current_action: Option<CurrentActionView>,
}
```

(`personality: Personality` placed between `mood` and `current_action`.)

Add `Personality` to the existing import at the top of `snapshot.rs`. The current line is:

```rust
use crate::agent::{Mood, Needs};
```

Change to:

```rust
use crate::agent::{Mood, Needs, Personality};
```

Update the serde-derive smoke test if it lists types explicitly:

```rust
#[cfg(test)]
mod serde_derive_tests {
    use super::{AgentSnapshot, CurrentActionView, Snapshot};
    // ... no changes needed; assert_*::<Personality> calls aren't there
    // because Personality lives in core::agent.
}
```

(No serde test changes needed; the existing tests cover the wire types added by this snapshot module.)

- [ ] **Step 4.3: Update `Sim::snapshot` in `crates/core/src/sim.rs` to project `Personality`**

Find the `snapshot` method's `filter_map` block. Currently:

```rust
.filter_map(|entity_ref| {
    let identity = entity_ref.get::<Identity>()?;
    let needs = entity_ref.get::<Needs>()?;
    let mood = entity_ref.get::<Mood>()?;
    let current_action = entity_ref
        .get::<CurrentAction>()
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

Replace with:

```rust
.filter_map(|entity_ref| {
    let identity = entity_ref.get::<Identity>()?;
    let needs = entity_ref.get::<Needs>()?;
    let mood = entity_ref.get::<Mood>()?;
    let personality = entity_ref.get::<Personality>().copied().unwrap_or_default();
    let current_action = entity_ref
        .get::<CurrentAction>()
        .and_then(|c| c.0.as_ref())
        .and_then(|action| project_current_action(action, self.tick, self));
    Some(AgentSnapshot {
        id: identity.id,
        name: identity.name.clone(),
        needs: *needs,
        mood: *mood,
        personality,
        current_action,
    })
})
```

(Adds the `let personality = ...` line and the `personality,` field. `Personality` is `Copy + Default`. The `unwrap_or_default()` is defensive — in practice every agent has a Personality from spawn.)

`Personality` is already imported in `sim.rs` from prior passes (used by `spawn_test_agent_with_needs`).

- [ ] **Step 4.4: Update `crates/protocol/tests/roundtrip.rs` fixture**

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
            current_action: None,
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
            personality: Personality::default(),
            current_action: None,
        })
        .collect();
    Snapshot { tick: 7, agents }
}
```

Add `Personality` to the existing imports at the top of the file. The current line:

```rust
use gecko_sim_core::agent::{Mood, Needs};
```

Change to:

```rust
use gecko_sim_core::agent::{Mood, Needs, Personality};
```

Add a new round-trip test at the bottom of the file:

```rust
#[test]
fn agent_snapshot_with_personality_roundtrips() {
    let snap = Snapshot {
        tick: 11,
        agents: vec![AgentSnapshot {
            id: AgentId::new(0),
            name: "Alice".to_string(),
            needs: Needs::full(),
            mood: Mood::neutral(),
            personality: Personality {
                openness: 0.4,
                conscientiousness: -0.3,
                extraversion: 0.7,
                agreeableness: -0.5,
                neuroticism: 0.1,
            },
            current_action: None,
        }],
    };
    roundtrip(&snap);
}
```

- [ ] **Step 4.5: Run all Rust tests**

```bash
cargo test --workspace
```

Expected: all green. The new `agent_snapshot_with_personality_roundtrips` test passes; existing tests still pass because `personality` is additive.

- [ ] **Step 4.6: Regenerate the ts-rs bindings**

```bash
cargo test -p gecko-sim-core --features export-ts
cargo test -p gecko-sim-protocol --features export-ts
```

Expected: passes; writes `apps/web/src/types/sim/Personality.ts` (new) and updates `apps/web/src/types/sim/AgentSnapshot.ts` to include the new field.

- [ ] **Step 4.7: Verify the typed bindings emitted correctly**

```bash
cat apps/web/src/types/sim/Personality.ts
cat apps/web/src/types/sim/AgentSnapshot.ts
```

Expected:
- `Personality.ts`: type with `openness`, `conscientiousness`, `extraversion`, `agreeableness`, `neuroticism` — all `number`.
- `AgentSnapshot.ts`: includes `personality: Personality` between `mood` and `current_action`.

- [ ] **Step 4.8: Note the transient pnpm tsc state**

`pnpm tsc --noEmit` will FAIL after Task 4 because `apps/web/src/lib/sim/reducer.test.ts`'s fixture builds `AgentSnapshot` literals without `personality`. Task 5 fixes this. **Do not run pnpm tsc here.**

- [ ] **Step 4.9: Verify Rust workspace still clean**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both clean.

- [ ] **Step 4.10: Confirm commit scope**

```bash
jj st
```

Expected: 3 modified Rust files (`crates/core/src/{snapshot,sim}.rs`, `crates/protocol/tests/roundtrip.rs`) + 1 modified TS (`AgentSnapshot.ts`) + 1 new TS (`Personality.ts`).

---

## Task 5: Frontend `<AgentList>` OCEAN columns + reducer fixture + round-trip test

**Files:**
- Modify: `apps/web/src/components/AgentList.tsx`
- Modify: `apps/web/src/lib/sim/reducer.test.ts`

After Task 4, the frontend's `AgentSnapshot` type includes `personality`, but `reducer.test.ts`'s fixture doesn't, so `pnpm tsc` is failing. This task closes the loop and adds the visible columns.

- [ ] **Step 5.1: Start the task commit**

```bash
jj new -m "Personality: frontend AgentList grows OCEAN columns; reducer fixture + round-trip test"
```

- [ ] **Step 5.2: Update the reducer test fixture in `apps/web/src/lib/sim/reducer.test.ts`**

Find the existing helper:

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
      personality: {
        openness: 0,
        conscientiousness: 0,
        extraversion: 0,
        agreeableness: 0,
        neuroticism: 0,
      },
      current_action: null,
    },
  ],
});
```

- [ ] **Step 5.3: Update other `AgentSnapshot` literals in the same file**

In `reducer.test.ts`, find every other `AgentSnapshot` literal (the prior round-trip tests for mood and current_action). Each must grow `personality: { openness: 0, ... }` to satisfy the new type shape. The "init message preserves the mood field" test:

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
          current_action: null,
        },
      ],
    };
```

Replace the agent literal with:

```ts
      agents: [
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: -0.5, arousal: 0.7, stress: 0.3 },
          personality: {
            openness: 0,
            conscientiousness: 0,
            extraversion: 0,
            agreeableness: 0,
            neuroticism: 0,
          },
          current_action: null,
        },
      ],
```

Same for the "init message preserves the current_action field" test. Find:

```ts
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: 0, arousal: 0, stress: 0 },
          current_action: { display_name: "Eat snack", fraction_complete: 0.5 },
        },
```

Replace with:

```ts
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: 0, arousal: 0, stress: 0 },
          personality: {
            openness: 0,
            conscientiousness: 0,
            extraversion: 0,
            agreeableness: 0,
            neuroticism: 0,
          },
          current_action: { display_name: "Eat snack", fraction_complete: 0.5 },
        },
```

- [ ] **Step 5.4: Add the new `personality` round-trip test**

Append to the bottom of the `describe("reduce", () => {` block (just before the closing `});`):

```ts
  it("init message preserves the personality field on the snapshot", () => {
    const snap: Snapshot = {
      tick: 5,
      agents: [
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: 0, arousal: 0, stress: 0 },
          personality: {
            openness: 0.4,
            conscientiousness: -0.3,
            extraversion: 0.7,
            agreeableness: -0.5,
            neuroticism: 0.1,
          },
          current_action: null,
        },
      ],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 5, snapshot: snap },
    });
    expect(next.snapshot?.agents[0].personality).toEqual({
      openness: 0.4,
      conscientiousness: -0.3,
      extraversion: 0.7,
      agreeableness: -0.5,
      neuroticism: 0.1,
    });
  });
```

- [ ] **Step 5.5: Run the reducer tests**

```bash
cd apps/web && pnpm test
```

Expected: 8 tests pass (existing 7 + the new one).

- [ ] **Step 5.6: Update `apps/web/src/components/AgentList.tsx`**

Find the existing `MOOD_KEYS` block:

```tsx
const MOOD_KEYS = ["valence", "arousal", "stress"] as const;
```

After it, add:

```tsx
const PERSONALITY_KEYS = [
  "openness",
  "conscientiousness",
  "extraversion",
  "agreeableness",
  "neuroticism",
] as const;

const PERSONALITY_LABELS = {
  openness: "O",
  conscientiousness: "C",
  extraversion: "E",
  agreeableness: "A",
  neuroticism: "N",
} as const;
```

Find the `<thead>` block:

```tsx
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
```

Replace with:

```tsx
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
          {PERSONALITY_KEYS.map((k) => (
            <th key={k} className="px-2 py-1 text-neutral-500" title={k}>
              {PERSONALITY_LABELS[k]}
            </th>
          ))}
          <th className="px-2 py-1">Doing</th>
        </tr>
```

Find the `<tbody>` row block:

```tsx
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
```

Replace with:

```tsx
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
            {PERSONALITY_KEYS.map((k) => (
              <td key={k} className="px-2 py-1 font-mono text-neutral-500">
                {agent.personality[k].toFixed(2)}
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
```

The personality columns sit between mood and Doing — same `text-neutral-500` styling as mood marks them as "not active needs."

- [ ] **Step 5.7: Run the full frontend gate**

```bash
cd apps/web && pnpm tsc --noEmit
cd apps/web && pnpm lint
cd apps/web && pnpm test
cd apps/web && pnpm build
```

Expected: all clean. `pnpm test` reports 8 passing (existing 7 + new personality round-trip).

- [ ] **Step 5.8: Manual end-to-end smoke**

For an autonomous-run verification:

```bash
cargo run -p gecko-sim-host > /tmp/host.log 2>&1 &
HOST_PID=$!
sleep 2
( cd apps/web && pnpm dev > /tmp/web.log 2>&1 ) &
WEB_PID=$!
sleep 8
curl -s http://localhost:3000 | grep -E "Doing|gecko-sim|Openness|Extraversion" | head -10
kill $WEB_PID $HOST_PID 2>/dev/null
wait 2>/dev/null
```

Expected: `curl` output includes the page shell. The OCEAN headers (single letters in `<th>` elements) are rendered, not the long names — they're inside `title` attributes which `grep` won't surface. A real browser visit confirms the columns populate with non-zero values per agent.

For a real browser smoke (preferred when interactive):
1. Five new columns appear after Stress and before Doing, each labeled with a single capital letter (O / C / E / A / N).
2. Hover over a header to see the full word in a tooltip.
3. Each agent (Alice / Bob / Charlie) shows distinct, non-zero values per Big Five component.
4. At `64×` speed, watch over a few minutes — agents with high `extraversion` (positive) trend toward `Sit` less than agents with negative extraversion.

- [ ] **Step 5.9: Confirm commit scope**

```bash
jj st
```

Expected: 2 files modified — `apps/web/src/components/AgentList.tsx` and `apps/web/src/lib/sim/reducer.test.ts`.

---

## Definition of done (rolled-up gate)

After Task 5 lands:

- `cargo build --workspace` clean.
- `cargo test --workspace` clean — all existing tests pass; new `agent::personality_tests` (3 cases) and `agent_snapshot_with_personality_roundtrips` test pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test -p gecko-sim-protocol --features export-ts` regenerates types; idempotent.
- `cargo test -p gecko-sim-core --features export-ts` regenerates types; idempotent.
- `cd apps/web && pnpm install --frozen-lockfile && pnpm test && pnpm tsc --noEmit && pnpm lint && pnpm build` clean.
- Manual smoke: 5 OCEAN columns visible, distinct non-zero values per agent.
- 5-commit chain matching the commit-strategy section of the spec.

## Notes for the implementer

- **rand 0.9 API.** Use `random_range(-1.0..=1.0)` (the inclusive range), NOT `gen_range(...)` (deprecated). Same convention as the decision-runtime pass.
- **`Personality::sample` generic over `Rng + ?Sized`.** Same shape as `weighted_pick`. Lets `&mut Pcg64Mcg` work directly without wrapping.
- **`SimRngResource(PrngState)` and `PrngState(Pcg64Mcg)` are double-wrapped.** Reach the inner RNG via `&mut sim_rng.0.0`. This is the same pattern the decision-runtime pass uses.
- **Task 4 leaves `pnpm tsc` failing on purpose.** The frontend reducer test fixture mismatches the new shape until Task 5 closes the loop. Same pattern as the decision-runtime pass's Tasks 7 → 9.
- **Three reducer-test fixtures need `personality` added.** The shared `fixtureSnapshot` helper plus two inline `Snapshot` literals in the mood and current_action round-trip tests. All three must be updated for `pnpm tsc` to pass.
- **Personality columns use single-letter labels** to keep the table from getting too wide. The full word is in the `title` attribute for hover tooltip. A future polish pass can switch to a dedicated personality panel.
- **Determinism.** `same_seed_same_personality` test in Task 1 and `tests/determinism.rs` together lock in deterministic sampling. If either fails, suspect a non-deterministic RNG path or HashMap iteration somewhere.
