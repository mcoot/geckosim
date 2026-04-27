# Decision-runtime v0 — utility AI scoring + commit + execute, no spatial, no interrupts

- **Date:** 2026-04-28
- **Status:** Draft
- **Scope:** Seventh implementation pass. Brings the seed catalog to life — agents pick advertisements from `Res<ObjectCatalog>`, score them via the v0 utility-AI formula, commit to a winning action, and apply its effects when its `duration_ticks` elapse. Establishes the action lifecycle (commit → execute → complete) without spatial walking and without interrupt handling.
- **Predecessors:**
  - [`2026-04-26-live-runtime-v0-design.md`](2026-04-26-live-runtime-v0-design.md) — `Sim` + tick + snapshot.
  - [`2026-04-27-ron-content-loading-design.md`](2026-04-27-ron-content-loading-design.md) — `ObjectCatalog` / `AccessoryCatalog` resources; seed `fridge` (with `EatSnack` ad) + `chair` (with `Sit` ad).
  - [`2026-04-27-frontend-scaffold-design.md`](2026-04-27-frontend-scaffold-design.md) — `<AgentList>` Tailwind table; ts-rs regen pipeline; reducer + Vitest harness.
  - [`2026-04-28-mood-system-design.md`](2026-04-28-mood-system-design.md) — `bevy_ecs::Schedule` ceremony; `(needs::decay, mood::update).chain()` ordering pattern.

## Goal

End state:

1. At sim startup, `host` spawns one instance per object type from the loaded catalog (today: 1 fridge, 1 chair) into a stub default leaf area.
2. Every agent carries `Personality::default()`, an `Option<CommittedAction>` (initially `None`), and a `RecentActionsRing` (initially empty, capacity 16).
3. Two new systems run every tick after needs+mood:
   - `decision::execute` — for each agent with `current_action = Some(...)`, advance the phase tick (no walking at v0 → straight to `Performing`); if the action's `expected_end_tick` is reached, apply effects atomically, push a `RecentActionEntry` into the ring (FIFO eviction at 16), set `current_action = None`.
   - `decision::decide` — for each agent with `current_action = None`, evaluate every advertisement of every smart-object instance against the agent's state, score the survivors, pick weighted-random from top-3, and commit. If no ads survive: commit `SelfAction(Idle)` with `duration_ticks = 5`.
4. `AgentSnapshot` grows `current_action: Option<CurrentActionView>` with `display_name` + `fraction_complete`; ts-rs regenerates the bindings; the frontend table grows a "Doing" column.
5. Manual smoke: open the browser, watch agents alternate between `Eat snack` and `Sit` (and idle when their needs are too high to qualify) as their needs decay.

This is the **smallest end-to-end vertical slice** of the decision runtime. Real spatial walking, need-threshold interrupts, macro gating, per-agent RNG, personality system, and content-driven instance spawning are all explicit follow-up passes.

## Non-goals (deferred)

- **No spatial walking.** Actions fire instantaneously after their `duration_ticks` regardless of agent or object position. `Predicate::Spatial(_)` always evaluates true. `LeafAreaId::DEFAULT` is a constant; agents have no `position` or `current_leaf` ECS component. ADR 0007's hierarchical world + 0.5m grid is its own pass.
- **No interrupts.** `Interrupt` and `InterruptSource` exist as schema types but no system fires them. `Predicate::AgentNeed` evaluates only at decision time; once an action is committed, no need-threshold crossing interrupts it. (Pass B from the brainstorming options.)
- **No per-agent RNG.** Score noise and weighted pick-from-top-N draw from the global `Sim::rng`. ADR 0008's per-agent RNG sub-streams land later.
- **No real Personality system.** Every agent gets `Personality::default()` (all zeros). The score formula's `personality_modifier` runs against zero-vectors and degenerates to `1.0` (no bias). Real personality lands as system #2 of 11 from ADR 0010.
- **No macro gating, no time-of-day.** `Predicate::MacroState` and `Predicate::TimeOfDay` always evaluate **false** (filter the ad out). `SituationalModifier::MacroVarWeight`, `TimeOfDayWeight`, `RelationshipWithTarget` are no-ops.
- **No content-driven object spawning.** `Sim::spawn_test_object(type_id, leaf, position)` is a test/host helper — same convention as `spawn_test_agent_with_needs`. World-seed scenarios that spawn instances declaratively land later.
- **No `PromotedEvent` emission.** Effect variants `PromotedEvent` log a `tracing::warn!` no-op. The promoted-event ring lands with the events system.
- **No skill/money/inventory/memory/relationship/health effects.** Those Effect variants log a `tracing::warn!` no-op. The seed catalog uses only `AgentNeedDelta` (and we extend to `AgentMoodDelta` for forward-compat).
- **No `Delta` wire type.** Snapshot-only — same as the current state.

## Architecture

### Module organization

```
crates/core/src/
├── decision/                       (already exists — schema types only)
│   └── mod.rs                      (CommittedAction, Phase, ActionRef, Interrupt, RecentActionEntry)
└── systems/
    ├── needs.rs                    (system #1)
    ├── mood.rs                     (system #3)
    └── decision/                   ← new this pass
        ├── mod.rs                  (re-exports + module declarations)
        ├── scoring.rs              (pure scoring helpers + unit tests)
        ├── predicates.rs           (pure predicate evaluators + unit tests)
        ├── effects.rs              (pure effect-application helper + unit tests)
        ├── decide.rs               (the decide system function + unit tests)
        └── execute.rs              (the execute system function + unit tests)
```

The schema types (`CommittedAction` etc.) stay in `core::decision`. The systems live under `core::systems::decision`. Two `decision` paths is mildly awkward but matches the existing `core::agent` / `core::systems::*` separation.

### New ECS components

#### On agents

In `core::agent::mod.rs`:

```rust
// Personality already exists as a schema struct. Add Component derive.
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq,
         Serialize, Deserialize, /* ts-rs cfg_attr */)]
pub struct Personality { … }            // unchanged shape

impl Personality {
    pub fn default() -> Self { Self { openness: 0.0, ..others_zero } }
}
```

In `core::decision::mod.rs`:

```rust
/// Wrapper around the optional committed action so it can be an ECS component.
/// `None` ↔ idle / awaiting decision.
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq, Default,
         Serialize, Deserialize)]
pub struct CurrentAction(pub Option<CommittedAction>);

/// Bounded ring of recent action templates, FIFO at 16 entries (per ADR 0011).
/// Used by the recency penalty in scoring.
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq, Default,
         Serialize, Deserialize)]
pub struct RecentActionsRing {
    pub entries: VecDeque<RecentActionEntry>,
}

impl RecentActionsRing {
    pub const CAPACITY: usize = 16;
    pub fn push(&mut self, entry: RecentActionEntry) {
        if self.entries.len() == Self::CAPACITY { self.entries.pop_front(); }
        self.entries.push_back(entry);
    }
}
```

#### On smart-object instances

In `core::object::mod.rs`:

```rust
// SmartObject already exists as a schema struct. Add Component derive.
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq,
         Serialize, Deserialize)]
pub struct SmartObject {
    pub id: ObjectId,
    pub type_id: ObjectTypeId,
    pub location: LeafAreaId,
    pub position: Vec2,
    pub owner: Option<OwnerRef>,
    pub state: StateMap,
}
```

#### Resources

```rust
// In core::time (or a new core::sim::tick_resource module)
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Copy, Default)]
pub struct CurrentTick(pub u64);
```

`CurrentTick` exposes the sim tick to systems (today: needed by `decide` to set `started_tick` and by `execute` to check completion). Inserted at `Sim::new`, updated at the start of every `Sim::tick` call.

### Tick semantics

Today: `Sim::tick` is `schedule.run(&mut world); self.tick += 1;`. Systems can't read the tick.

After this pass: `self.tick += 1` runs **before** `schedule.run`, and `*world.resource_mut::<CurrentTick>() = CurrentTick(self.tick);` syncs the resource. So:

- `Sim::current_tick() == 0` at construction; `== 1` after one `tick()` call (unchanged).
- During the Nth `schedule.run`, every system sees `Res<CurrentTick>` = `N`.
- An action committed during the Nth tick has `started_tick = N`. With `duration_ticks = D`, completion fires when `current_tick = N + D` (i.e., `D` ticks after commit, during what an outside observer would call "tick N+D").

This shift doesn't change any external behavior — it just makes the tick visible inside systems.

### `LeafAreaId::DEFAULT` stub

In `core::ids`:

```rust
impl LeafAreaId {
    /// v0 stub: every agent and every smart-object instance lives in this
    /// single implicit leaf area until the spatial pass introduces a real
    /// world graph (ADR 0007).
    pub const DEFAULT: Self = Self::new(0);
}
```

`spawn_test_agent_with_needs` and `spawn_test_object` both stamp this. The decision-runtime's spatial predicate evaluator returns `true` for all `Predicate::Spatial(_)` variants at v0; this is documented as "every entity is in `LeafAreaId::DEFAULT` so `SameLeafArea` is trivially true; `AdjacentArea` and `KnownPlace` likewise — the stub leaf area is its own neighborhood."

### Sim API additions

```rust
impl Sim {
    /// Spawn a smart-object instance of the given type. Test-only entry
    /// point until content-driven instance spawning lands.
    pub fn spawn_test_object(
        &mut self,
        type_id: ObjectTypeId,
        location: LeafAreaId,
        position: Vec2,
    ) -> ObjectId;

    /// Reads the smart-object catalog through the existing accessor and
    /// spawns one instance of every loaded `ObjectType`. Convenience for
    /// the host's seed-instance spawn at startup.
    pub fn spawn_one_of_each_object_type(
        &mut self,
        location: LeafAreaId,
        position: Vec2,
    ) -> Vec<ObjectId>;
}
```

`spawn_test_object` allocates an `ObjectId` from a new `Sim::next_object_id` counter (mirroring `next_agent_id`), reads `default_state` from the catalog by `type_id`, and spawns the entity with `(SmartObject { … })` as a single-component bundle. (No spatial component yet — `location` and `position` live inside the `SmartObject` struct.)

### Score formula

Per ADR 0011, scaled to v0. `score_template.need_weights` is `Vec<(Need, f32)>`; `personality_weights` is a `Personality` struct (5 floats); `situational_modifiers` is `Vec<SituationalModifier>`. A small helper `need_value(&Needs, Need) -> f32` and `mood_value(&Mood, MoodDim) -> f32` map enum keys to struct fields.

```
base_utility = Σ over (need, factor) in ad.score_template.need_weights:
                 factor * (1.0 - need_value(&agent.needs, need))    // pressure rises as need drops

personality_modifier = (1.0 + 0.5 * dot(agent.personality, ad.score_template.personality_weights)).max(0.1)
                       // sensitivity = 0.5 (constant from ADR 0011)
                       // dot(p, q) = p.openness*q.openness + p.conscientiousness*q.conscientiousness + …

mood_modifier = product over each SituationalModifier::MoodWeight { dim, weight } in ad.score_template.situational_modifiers:
                  (1.0 + weight * mood_value(&agent.mood, dim)).max(0.1)
                ; default 1.0 when no MoodWeight modifiers.
                       // MacroVarWeight, TimeOfDayWeight,
                       // RelationshipWithTarget contribute 1.0 at v0.

recency_penalty = if agent.recent_actions.entries.iter()
                     .any(|e| e.ad_template == (object_type_id, ad.id)) { 0.5 } else { 0.0 }
                       // recent_actions matches by (ObjectTypeId, AdvertisementId) per ADR 0011

noise = sim.rng.gen::<f32>() * 0.1                          // uniform [0, 0.1)

score = base_utility * personality_modifier * mood_modifier * (1.0 - recency_penalty) + noise
```

All factors stay non-negative (clamps + `max(0.1, ...)`). At v0 personality is zero-vector → `personality_modifier = 1.0` always.

### Predicate evaluator (v0)

```rust
fn evaluate(predicate: &Predicate, ctx: &EvalContext) -> bool {
    match predicate {
        Predicate::AgentNeed(need, op, threshold) =>
            apply_op(ctx.needs[*need], *op, *threshold),

        Predicate::ObjectState(key, op, value) =>
            ctx.object_state.get(key).is_some_and(|v| apply_op_state(v, *op, value)),

        Predicate::Spatial(_) => true,                  // v0: always pass

        Predicate::AgentSkill(_, _, _) | Predicate::AgentInventory(_, _, _) |
        Predicate::AgentRelationship(_, _, _, _) | Predicate::MacroState(_, _, _) |
        Predicate::TimeOfDay(_)
            => false,                                   // v0: missing systems → predicate fails
    }
}
```

`EvalContext` carries the agent's `&Needs`, the smart object's `&StateMap`, and (for completeness; unused at v0) `agent.recent_actions` and current tick.

`apply_op` is the obvious six-way comparison; `apply_op_state` matches on `StateValue` variant equality + ordering for numeric variants.

A whole advertisement passes preconditions iff **every** predicate in `ad.preconditions` evaluates true (conjunctive AND).

### Effect application (v0)

At end-tick, on the agent's entity:

```rust
fn apply(effect: &Effect, agent: &mut AgentEffectTarget) {
    match effect {
        Effect::AgentNeedDelta(need, delta) => {
            let v = &mut agent.needs[*need];
            *v = (*v + delta).clamp(0.0, 1.0);
        }
        Effect::AgentMoodDelta(dim, delta) => {
            let v = &mut agent.mood[*dim];
            *v = (*v + delta).clamp(min_for(*dim), 1.0);
        }

        Effect::AgentSkillDelta(_, _) | Effect::MoneyDelta(_) |
        Effect::InventoryDelta(_, _) | Effect::MemoryGenerate { .. } |
        Effect::RelationshipDelta(_, _, _) | Effect::HealthConditionChange(_) |
        Effect::PromotedEvent(_, _)
            => {
                tracing::warn!("decision::execute: effect kind not yet implemented: {effect:?}");
                // no-op
            }
    }
}
```

`AgentEffectTarget` is a small struct of `&mut Needs, &mut Mood` borrowed from the entity. Other components (Skills, etc.) absent → corresponding effect variants are no-ops by routing through the warn!.

`min_for(MoodDim::Valence) = -1.0`; the rest = 0.0. Same as the mood update system's clamps.

### `decide` system

```rust
fn decide(
    catalog: Res<ObjectCatalog>,
    current_tick: Res<CurrentTick>,
    mut sim_rng: ResMut<SimRngResource>,                    // see "RNG plumbing" below
    objects: Query<&SmartObject>,
    mut agents: Query<(
        &Needs, &Mood, &Personality, &RecentActionsRing,
        &mut CurrentAction,
    )>,
);
```

For each agent whose `CurrentAction.0` is `None`:

1. Build `Vec<(ObjectId, &Advertisement, &StateMap)>` by iterating `objects` and looking up each one's catalog entry, flattening the per-type advertisements.
2. Filter by predicate evaluation.
3. Score each survivor.
4. Sort scores descending; truncate to top 3; weighted-pick.
5. Emit `CommittedAction` with `started_tick = current_tick.0`, `expected_end_tick = current_tick.0 + ad.duration_ticks as u64`, `phase = Phase::Performing` (skip Walking at v0), `target_position = None`.
6. If no candidates survive: emit `SelfAction(SelfActionKind::Idle)` with `duration_ticks = IDLE_DURATION_TICKS = 5`, `phase = Phase::Performing`.

**Iteration order:** the agent query and object query both iterate in deterministic order (bevy's archetype storage is insertion-ordered for our single-thread case). The RNG draws happen within a single thread in deterministic sequence.

### `execute` system

```rust
fn execute(
    catalog: Res<ObjectCatalog>,
    current_tick: Res<CurrentTick>,
    objects: Query<&SmartObject>,
    mut agents: Query<(
        &mut Needs, &mut Mood, &mut RecentActionsRing,
        &mut CurrentAction,
    )>,
);
```

For each agent with `CurrentAction.0 = Some(action)`:

1. If `current_tick.0 >= action.expected_end_tick`:
   a. Look up the advertisement (if `ActionRef::Object { object, ad }`):
      - `objects.get(...)` to find the smart-object instance.
      - `catalog.by_id.get(type_id)` to find the type.
      - Among `type.advertisements`, find the matching `ad` id.
   b. For each `Effect` in `ad.effects`, call the effect applicator.
   c. Compute the `RecentActionEntry { ad_template: (type_id, ad_id), completed_tick: current_tick.0 }` and `recent_actions.push(entry)`.
   d. Set `CurrentAction.0 = None`.
2. (`SelfAction(Idle)`: same path but no effects to apply, no ring entry; just clear the action.)

**No phase advancement** at v0 because we skip Walking. The `Phase::Performing` stays `Performing` until completion.

### Schedule order

```rust
schedule.add_systems(
    (
        systems::needs::decay,
        systems::mood::update,
        systems::decision::execute,
        systems::decision::decide,
    ).chain(),
);
```

`execute` runs before `decide` so that an agent whose action just completed has `current_action = None` visible to `decide` in the same tick (no idle gap).

### RNG plumbing

The global `Sim::rng` (a `PrngState` field) needs to be visible to systems. v0 wraps it in a Resource:

```rust
#[derive(bevy_ecs::prelude::Resource, Debug)]
pub struct SimRngResource(pub PrngState);
```

`Sim::new` inserts `SimRngResource(PrngState::from_seed(seed))` instead of holding `rng` as a struct field. The current `#[expect(dead_code)]` on `rng` goes away. `decide` takes `ResMut<SimRngResource>`. `Sim::current_tick`, etc. unchanged.

Determinism: a single thread, deterministic system order, deterministic query iteration → byte-equal snapshots from the same seed. The existing determinism test continues to pass.

### Wire shape: `AgentSnapshot.current_action`

```rust
// New in core::snapshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, /* ts-rs cfg_attr */)]
pub struct CurrentActionView {
    pub display_name: String,
    pub fraction_complete: f32,    // 0.0 at start, 1.0 at scheduled completion
}

pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
    pub mood: Mood,
    pub current_action: Option<CurrentActionView>,    // null when idle
}
```

`Sim::snapshot` projects `CommittedAction → CurrentActionView` by:
- Looking up `display_name` via the catalog (for `ActionRef::Object`) or hardcoding `"Idle"` / `"Wait"` (for `SelfAction`).
- Computing `fraction_complete = (current_tick - started_tick) / duration_ticks`.

The full `CommittedAction` stays inside the ECS world; only the projection ships on the wire.

### Frontend "Doing" column

`<AgentList>` grows one column after the mood trio:

```tsx
<th className="px-2 py-1">Doing</th>
// ...
<td className="px-2 py-1">
  {agent.current_action
    ? `${agent.current_action.display_name} (${(agent.current_action.fraction_complete * 100).toFixed(0)}%)`
    : "—"}
</td>
```

Reducer test fixture grows `current_action: null` by default; one new test asserts a non-null `current_action` round-trips through the reducer.

## Module changes by crate

### `gecko-sim-core`

- **Modified:**
  - `src/agent/mod.rs` — `Personality` gains `Component` derive + `Personality::default()` constructor (`all-zeros`).
  - `src/object/mod.rs` — `SmartObject` gains `Component` derive.
  - `src/ids.rs` — `LeafAreaId::DEFAULT` const.
  - `src/decision/mod.rs` — adds `CurrentAction` and `RecentActionsRing` Component types; adds `IDLE_DURATION_TICKS` const = 5.
  - `src/sim.rs` — adds `next_object_id` field; adds `spawn_test_object` and `spawn_one_of_each_object_type` methods; replaces `rng` field with `SimRngResource` insertion; adds `CurrentTick` resource insertion + per-tick update; updates `spawn_test_agent_with_needs` to attach `Personality::default()`, `CurrentAction(None)`, `RecentActionsRing::default()`; updates `Sim::snapshot` to project `current_action`.
  - `src/snapshot.rs` — adds `CurrentActionView`; `AgentSnapshot` grows `current_action: Option<CurrentActionView>`.
  - `src/systems/mod.rs` — declares `pub mod decision;`.
  - `src/lib.rs` — re-exports any types needed by the wire (`CurrentActionView`).
- **New:**
  - `src/systems/decision/mod.rs` — module declarations.
  - `src/systems/decision/scoring.rs` — pure score helpers + unit tests.
  - `src/systems/decision/predicates.rs` — pure predicate evaluator + unit tests.
  - `src/systems/decision/effects.rs` — pure effect applicator + unit tests.
  - `src/systems/decision/execute.rs` — `pub(crate) fn execute(...)` system + unit tests.
  - `src/systems/decision/decide.rs` — `pub(crate) fn decide(...)` system + unit tests.
- **Cargo.toml:** unchanged.

### `gecko-sim-protocol`

- **Modified:** `tests/roundtrip.rs` — fixtures grow `current_action: None` (default) on each `AgentSnapshot`. New variant fixture for `current_action: Some(CurrentActionView { ... })`.

### `gecko-sim-host`

- **Modified:** `src/main.rs` — after `Sim::new`, call `sim.spawn_one_of_each_object_type(LeafAreaId::DEFAULT, Vec2::ZERO)`. Update the existing `tracing::info!` to include the spawned object count.

### `apps/web`

- **Modified:**
  - `src/types/sim/{AgentSnapshot,CurrentActionView}.ts` — auto-regenerated by `pnpm gen-types`.
  - `src/components/AgentList.tsx` — new `Doing` column.
  - `src/lib/sim/reducer.test.ts` — fixture grows `current_action: null`; new test asserts non-null `current_action` round-trips.

## Tests

### Rust unit tests

- `systems::decision::scoring::tests`:
  - `base_utility_zero_when_need_full` — agent at full needs, ad with `need_weights = [(Hunger, 1.0)]` → 0 pressure → 0 base utility.
  - `base_utility_max_when_need_empty` — agent at zero needs → pressure 1.0 → base = sum of weights.
  - `personality_modifier_one_at_zero_personality` — default personality → modifier = 1.0.
  - `personality_modifier_clamped_at_floor` — extreme negative dot product → clamped to 0.1.
  - `mood_modifier_compounds_multiplicatively` — two MoodWeight entries → product.
  - `recency_penalty_halves_score_when_recent` — ad in ring → score scaled by 0.5.
  - `weighted_pick_selects_highest_score_in_expectation` — deterministic seed, run 100 picks, assert highest-scoring ad picked > 50% of the time.
  - `weighted_pick_falls_back_to_uniform_on_zero_total` — all scores zero → uniform random over top-N.

- `systems::decision::predicates::tests`:
  - `agent_need_op_lt_passes_when_below_threshold`.
  - `agent_need_op_lt_fails_when_above_threshold`.
  - `object_state_eq_bool_passes` and `object_state_missing_key_fails`.
  - `spatial_always_passes` (any variant returns true).
  - `unsupported_predicates_fail` (AgentSkill, MacroState, TimeOfDay → false).

- `systems::decision::effects::tests`:
  - `agent_need_delta_applies_and_clamps`.
  - `agent_mood_delta_applies_and_clamps`.
  - `unsupported_effects_no_op` (just don't crash; can't check for tracing log without infrastructure).

- `systems::decision::execute::tests`:
  - Build a tiny World with one agent + one fridge instance + an active CommittedAction whose `expected_end_tick` matches the resource's CurrentTick → after one schedule run, `current_action == None` and hunger increased by 0.4 (the fridge's `EatSnack` ad effect).
  - Same setup but `expected_end_tick > current_tick` → action stays active, no effects applied.
  - Idle action completes → no effects, no ring entry, action cleared.

- `systems::decision::decide::tests`:
  - World with 1 hungry agent (`hunger = 0.3`) + 1 fridge → `decide` picks the `EatSnack` ad (only qualifying one), commits with correct `expected_end_tick`.
  - World with 1 full-needs agent + 1 fridge → `EatSnack` filtered out (`AgentNeed(Hunger, Lt, 0.6)` fails) → falls back to `SelfAction(Idle)`.

### Rust integration test

`crates/core/tests/decision.rs`:

```rust
#[test]
fn agent_eats_from_fridge_when_hungry() {
    let mut sim = Sim::new(0, seed_content_bundle());   // load real RON catalog
    sim.spawn_test_agent_with_needs("Hungry", Needs { hunger: 0.3, ..Needs::full() });
    sim.spawn_one_of_each_object_type(LeafAreaId::DEFAULT, Vec2::ZERO);

    // The fridge ad takes 10 ticks to complete + 1 tick to commit.
    for _ in 0..15 { sim.tick(); }

    let snap = sim.snapshot();
    let agent = &snap.agents[0];
    assert!(agent.needs.hunger > 0.6, "hunger restored: {}", agent.needs.hunger);
    // Either currently doing nothing (just completed) or queued for next decision:
    // simply assert the action's effects took.
}
```

Helper `seed_content_bundle()` invokes `gecko_sim_content::load_from_dir` over the workspace `content/` directory, mirroring the existing `seed_loads.rs` test pattern. Lives in `crates/core/tests/common/mod.rs` (new) so other integration tests can share it.

### Frontend Vitest

Update existing `apps/web/src/lib/sim/reducer.test.ts`:

- `fixtureSnapshot` grows `current_action: null`.
- New test: `init message preserves the current_action field on the snapshot` — sends a Snapshot with `current_action: { display_name: "Eat snack", fraction_complete: 0.5 }`, asserts the reducer copies it through.

### Manual smoke

After all commits land:

1. `cargo run -p gecko-sim-host` — observe two new tracing lines: `loading content`, `content loaded object_types=2 accessories=2`, **then** `seed instances spawned object_count=2`.
2. `cd apps/web && pnpm dev`.
3. Browser: agent list shows three rows. Initially `Doing = "—"` (full needs → `EatSnack` filtered out; agent idles for 5 ticks then re-decides). After ~480 ticks (8 sim-hours), hunger drops below 0.6 → `EatSnack` qualifies → agent shows `"Eat snack (NN%)"` cycling 0% → 100% → "—" briefly → cycles again. The `Sit` ad has empty preconditions so it qualifies always; after the recency penalty kicks in and other ads also qualify, the agent's `Doing` column fills with alternating `Eat snack` / `Sit` actions.
4. Click `64×` to accelerate; behavior cycles through visibly within seconds.

## Definition of done

- `cargo build --workspace` clean.
- `cargo test --workspace` clean — all existing tests + new unit/integration tests pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test -p gecko-sim-protocol --features export-ts` regenerates types; idempotent.
- `cargo test -p gecko-sim-core --features export-ts` regenerates types; idempotent.
- `cd apps/web && pnpm test && pnpm tsc --noEmit && pnpm lint && pnpm build` clean.
- Manual smoke: agents cycle through actions visibly in the browser at 64× speed.
- 9-commit chain matching the commit-strategy section below.

## Commit strategy

Nine commits:

1. `Decision: SmartObject Component derive + LeafAreaId::DEFAULT + ts-rs derive on SmartObject`
2. `Decision: Personality / CurrentAction / RecentActionsRing components; CurrentTick resource; spawn_test_agent_with_needs attaches them`
3. `Decision: scoring / predicates / effects pure helpers + unit tests`
4. `Decision: execute system + unit tests`
5. `Decision: decide system + unit tests`
6. `Decision: register systems in schedule; spawn_test_object + spawn_one_of_each_object_type; integration test`
7. `Decision: CurrentActionView wire type + AgentSnapshot.current_action; ts-rs regen; protocol roundtrip fixtures updated`
8. `Decision: host spawns seed instances at startup; manual smoke commands`
9. `Decision: frontend AgentList grows Doing column; reducer fixture + new round-trip test`

Plan-author may merge commits 4 and 5 if both implementations stay small; merging 6 and 8 is also reasonable. The spec doesn't require strict atomicity beyond keeping each commit green.

## Trace to ADRs

- **ADR 0004 (decision model):** smart-object advertisements + utility scoring + weighted-random from top-N + recency penalty all land. Per-agent personality biasing is in the formula (zero-vector at v0). Memory-tinted recall, schedules-as-virtual-need, and macro forcing functions are all explicitly deferred.
- **ADR 0008 (time):** `CurrentTick` resource exposes the canonical clock to systems; `started_tick` and `expected_end_tick` use the same N-after-N-ticks semantic.
- **ADR 0009 (macro/micro seam):** `Predicate::MacroState` is implemented as "always false" at v0 — when macro state lands, the predicate evaluator picks it up via a new `Res<MacroState>` parameter and the v0 ads with macro predicates start qualifying.
- **ADR 0010 (systems inventory):** systems #1 (needs), #3 (mood), and the decision-runtime cluster all run together in one schedule. Personality (system #2) is a static read-only ECS component this pass; its dynamics (if any) are still deferred.
- **ADR 0011 (schema):** `CommittedAction`, `Phase`, `ActionRef`, `RecentActionEntry`, the `Predicate`/`Effect` enums, and the score formula all match the spec verbatim. `Phase` is fixed at `Performing` for non-self actions at v0; `Walking` and `Completing` lie unused until spatial.
- **ADR 0013 (transport):** `AgentSnapshot.current_action` is additive; bandwidth grows ~30 bytes per agent per snapshot worst case (display_name string + f32). Negligible.

## Deferred items

| Item | Triggers landing | Lives in |
|---|---|---|
| Spatial walking (Walking phase, real LeafArea graph, agent positions) | Spatial pass (ADR 0007 plumbing) | `core::world` + `Phase::Walking` activation |
| Need-threshold interrupts (pro-rata effect application) | Interrupt pass | `core::systems::decision::interrupts` |
| Macro-precondition gating + re-check at macro tick | Macro pass | predicate evaluator extension + macro tick |
| Per-agent RNG sub-streams | RNG pass | `Sim::new` + `Components::AgentRng` |
| Real Personality system | Personality pass | `core::systems::personality` |
| Action chaining (`next: Option<...>`) | Multi-step actions | `core::decision::CommittedAction` |
| `PromotedEvent` emission from effects | Events pass | `core::events::ring` + `Effect::PromotedEvent` |
| Skills/Money/Inventory/Memory/Relationships/Health Effect variants | Each system's pass | `core::systems::*` |
| Content-driven instance spawning (replaces `spawn_test_object`) | World-seed/scenario pass | new `core::sim::scenario` |
| Active-action-id + remaining-tick on the wire | Frontend richer "what is X doing" | `CurrentActionView` extension |
| Inspection UI (click agent → see why it picked this ad) | Inspection PlayerInput | `protocol::messages` + frontend panel |

## What this pass enables next

- **Spatial pass.** With actions firing on time-only, the natural extension is `Phase::Walking` taking real travel time across a leaf area. Drags in ADR 0007.
- **Interrupt pass.** Hooks need-threshold crossings into the decision runtime. Pro-rata effect application (per ADR 0011) lights up.
- **Personality pass.** Once an agent's personality is non-zero, the score formula's biasing has actual effect — different personalities pick different actions in the same situation.
- **Macro pass.** First system to introduce `Res<MacroState>`; advertisements with macro preconditions start qualifying.
- **Events pass.** First consumer of `Effect::PromotedEvent`; the events ring becomes non-empty.
