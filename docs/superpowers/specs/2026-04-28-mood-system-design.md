# Mood system v0 — system #2 of 11, plus `bevy_ecs::Schedule` ceremony

- **Date:** 2026-04-28
- **Status:** Draft
- **Scope:** Sixth implementation pass. Lands the second sim system (`mood`, system #3 of 11 from ADR 0010), introduces the `bevy_ecs::Schedule` ceremony in `Sim`, propagates `mood` to the wire (snapshot + ts-rs regen), and renders the three mood components in the frontend's agent list.
- **Predecessors:**
  - [`2026-04-26-live-runtime-v0-design.md`](2026-04-26-live-runtime-v0-design.md) — `Sim::tick` calls `systems::needs::decay(&mut world)` directly. No `bevy_ecs::Schedule` yet.
  - [`2026-04-27-ws-transport-v0-design.md`](2026-04-27-ws-transport-v0-design.md) — explicitly defers Schedule introduction: "`bevy_ecs::Schedule` ceremony in `core::sim::tick` — System #2 lands". This pass is system #2.
  - [`2026-04-27-frontend-scaffold-design.md`](2026-04-27-frontend-scaffold-design.md) — `<AgentList>` renders a Tailwind table over `Snapshot.agents`; `ts-rs` regen wired and committed bindings.

## Goal

End state:

1. `Mood` is an ECS component (`#[derive(bevy_ecs::component::Component)]` added to the existing schema struct in `core::agent`). Spawned agents start at `Mood::neutral() = (0, 0, 0)`.
2. `systems::mood::update` runs every tick after `systems::needs::decay`, drifting each agent's mood toward a needs-derived target with inertia (placeholder formula; retunable).
3. `Sim` owns a `bevy_ecs::schedule::Schedule` that holds both systems registered in order via `(needs::decay, mood::update).chain()`. `Sim::tick` becomes `self.schedule.run(&mut self.world); self.tick += 1;`.
4. Both system functions are refactored from `fn(&mut World)` into idiomatic bevy systems (`fn(query: Query<&mut T>)`).
5. `AgentSnapshot` grows a `pub mood: Mood` field after `needs`. The wire shape changes; `protocol/tests/roundtrip.rs` fixtures update.
6. `ts-rs` regenerates `apps/web/src/types/sim/{Mood,AgentSnapshot}.ts`. Committed.
7. Frontend `<AgentList>` grows three columns — Valence, Arousal, Stress — each formatted to 2 decimal places.
8. Manual smoke shows mood values drifting visibly in the browser after ~1–2 sim-minutes of runtime.

This is the smallest "second sim system" slice — establishes the Schedule pattern, adds one observable coupling between two existing systems, and locks in the wire-evolution workflow (Rust schema change → ts-rs regen → frontend column added).

## Non-goals (deferred)

- **No event coupling.** ADR 0010 says mood is reactive to events, weather, social interactions. None of those systems exist yet; the v0 mood update reads `Needs` only. Event-driven mood spikes land alongside the events system.
- **No `PromotedEvent` emission on mood thresholds.** Stress crossing 1.0 is not promoted today. Lands when events does.
- **No personality coupling.** ADR 0011's `Personality` is sketched but no system reads or writes it yet. Personality biasing of mood-target lands when the personality system does.
- **No mood-driven action scoring.** ADR 0011's `SituationalModifier::MoodWeight` exists in the advertisement schema but no decision system consumes it. Lands with the decision-runtime pass.
- **No memory tinting.** ADR 0010 mentions "mood-tinted recall"; defer to memory system.
- **No tunable mood constants per gecko.** The α drift rate and need-pressure formula are static. Per-personality tuning is a future polish.
- **No `bevy_ecs` parallel scheduling.** Single-threaded sim per ADR 0012. The Schedule is used for ordering, not parallelism.

## Architecture

### Crate dep graph (unchanged)

```
host ──▶ content ──▶ core
   │                  ▲
   └──▶ protocol ─────┘
```

All changes are inside `crates/core` (system + Sim driver), `crates/protocol` (snapshot fixtures in tests), and `apps/web` (frontend column + reducer test).

### `Mood` as ECS component

In `crates/core/src/agent/mod.rs`, add `bevy_ecs::component::Component` to the `Mood` struct's derive list (lazy-shard pattern; same single-type-for-schema-and-component as `Needs`):

```rust
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
    pub valence: f32,
    pub arousal: f32,
    pub stress: f32,
}

impl Mood {
    /// Neutral mood: all components at zero.
    #[must_use]
    pub fn neutral() -> Self {
        Self { valence: 0.0, arousal: 0.0, stress: 0.0 }
    }
}
```

The `Component` derive plus the `ts-rs` derives both gate via existing repo conventions. `Mood::neutral()` is the constructor used by `Sim::spawn_test_agent`.

### Value ranges

| Field | Range | Semantic |
|---|---|---|
| `valence` | `[-1, 1]` | negative = unhappy, positive = happy |
| `arousal` | `[0, 1]` | 0 = calm, 1 = alert / excited |
| `stress` | `[0, 1]` | 0 = none, 1 = max stress |

`systems::mood::update` clamps to these ranges after every tick.

### Update formula (placeholder; retunable)

Per tick, for each agent with both `Needs` and `Mood`:

```
mean_need = (hunger + sleep + social + hygiene + fun + comfort) / 6
min_need  = min(hunger, sleep, social, hygiene, fun, comfort)

valence_target = 2 · mean_need - 1                      // ∈ [-1, 1]
arousal_target = (1 - mean_need).clamp(0, 1)            // empty needs → alert
stress_target  = ((0.5 - min_need) · 2).clamp(0, 1)     // kicks in when worst need < 0.5

mood.valence += (valence_target - mood.valence) · α
mood.arousal += (arousal_target - mood.arousal) · α
mood.stress  += (stress_target  - mood.stress ) · α

mood.valence = mood.valence.clamp(-1.0, 1.0)
mood.arousal = mood.arousal.clamp( 0.0, 1.0)
mood.stress  = mood.stress .clamp( 0.0, 1.0)
```

with `α = 0.01` per tick. Mood reaches ~63% of target in ~100 ticks (≈ 1.67 sim-hours). All constants land as `pub const` in `systems::mood` mirroring `*_DECAY_PER_TICK` in `systems::needs`:

```rust
pub const MOOD_DRIFT_RATE_PER_TICK: f32 = 0.01;
// no STRESS_THRESHOLD const yet — formula inlines it; promote to const when a
// second consumer (event emission) wants the same threshold.
```

The formula is **deliberately simple** — six floats in, three floats out, no RNG. Determinism preserved.

### `bevy_ecs::Schedule` introduction

Currently `Sim` has fields `world`, `tick`, `rng`, `next_agent_id`. After this pass, add `schedule: bevy_ecs::schedule::Schedule`:

```rust
pub struct Sim {
    world: World,
    schedule: Schedule,
    tick: u64,
    #[expect(dead_code, reason = "consumed by first RNG-using system in a later pass")]
    rng: PrngState,
    next_agent_id: u64,
}
```

In `Sim::new`, build the schedule once:

```rust
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};

let mut schedule = Schedule::default();
schedule.add_systems((systems::needs::decay, systems::mood::update).chain());
```

`.chain()` enforces sequential ordering: `mood::update` runs strictly after `needs::decay` so mood reads the post-decay needs values for the same tick.

`Sim::tick` becomes:

```rust
pub fn tick(&mut self) -> TickReport {
    self.schedule.run(&mut self.world);
    self.tick += 1;
    TickReport
}
```

The schedule is built once at `Sim::new` and runs every tick. No per-tick allocation.

### System refactor: `Query`-parameter shape

Both systems become idiomatic bevy systems. `systems::needs::decay` is rewritten:

```rust
// Before: pub(crate) fn decay(world: &mut World) { ... world.query::<&mut Needs>() ... }
// After:
pub(crate) fn decay(mut needs: Query<&mut Needs>) {
    for mut n in needs.iter_mut() {
        n.hunger  = (n.hunger  - HUNGER_DECAY_PER_TICK ).max(0.0);
        n.sleep   = (n.sleep   - SLEEP_DECAY_PER_TICK  ).max(0.0);
        n.social  = (n.social  - SOCIAL_DECAY_PER_TICK ).max(0.0);
        n.hygiene = (n.hygiene - HYGIENE_DECAY_PER_TICK).max(0.0);
        n.fun     = (n.fun     - FUN_DECAY_PER_TICK    ).max(0.0);
        n.comfort = (n.comfort - COMFORT_DECAY_PER_TICK).max(0.0);
    }
}
```

`systems::mood::update`:

```rust
pub(crate) fn update(mut q: Query<(&Needs, &mut Mood)>) {
    for (needs, mut mood) in q.iter_mut() {
        let mean_need = mean(needs);
        let min_need  = min(needs);

        let valence_target = 2.0 * mean_need - 1.0;
        let arousal_target = (1.0 - mean_need).clamp(0.0, 1.0);
        let stress_target  = ((0.5 - min_need) * 2.0).clamp(0.0, 1.0);

        mood.valence = (mood.valence + (valence_target - mood.valence) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(-1.0, 1.0);
        mood.arousal = (mood.arousal + (arousal_target - mood.arousal) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(0.0, 1.0);
        mood.stress  = (mood.stress  + (stress_target  - mood.stress ) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(0.0, 1.0);
    }
}

fn mean(n: &Needs) -> f32 {
    (n.hunger + n.sleep + n.social + n.hygiene + n.fun + n.comfort) / 6.0
}

fn min(n: &Needs) -> f32 {
    n.hunger.min(n.sleep).min(n.social).min(n.hygiene).min(n.fun).min(n.comfort)
}
```

Bevy injects the `Query<...>` parameters via system-parameter resolution. The system runs once per `schedule.run(...)`.

### Agent spawning

`Sim::spawn_test_agent` grows a `Mood::neutral()` component:

```rust
self.world.spawn((
    Identity { id, name: name.to_string() },
    Needs::full(),
    Mood::neutral(),
));
```

### Wire shape: `AgentSnapshot` grows `mood`

In `crates/core/src/snapshot.rs`:

```rust
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
    pub mood: Mood,
}
```

Field order (id → name → needs → mood) matches ADR 0011's schema field order (system #1 = needs, system #3 = mood) so future readers can predict the shape.

`Sim::snapshot` updates to read both components:

```rust
let mut agents: Vec<AgentSnapshot> = self
    .world
    .iter_entities()
    .filter_map(|e| {
        let identity = e.get::<Identity>()?;
        let needs = e.get::<Needs>()?;
        let mood = e.get::<Mood>()?;
        Some(AgentSnapshot {
            id: identity.id,
            name: identity.name.clone(),
            needs: *needs,
            mood: *mood,
        })
    })
    .collect();
```

### Wire-format propagation

- `cargo test -p gecko-sim-protocol --features export-ts` regenerates `apps/web/src/types/sim/Mood.ts` and rewrites `AgentSnapshot.ts` to include the new field.
- `protocol/tests/roundtrip.rs` fixtures grow `mood: Mood::neutral()` (or specific values) on every `AgentSnapshot` construction so the JSON roundtrip continues to lock the wire shape.
- `host/tests/ws_smoke.rs` continues to work — it asserts agent count, name, and tick advancement, not specific need values. Adding mood to the wire is additive.

### Frontend changes

In `apps/web/src/components/AgentList.tsx`, add three columns alongside the six need columns:

```tsx
const NEED_KEYS = ["hunger", "sleep", "social", "hygiene", "fun", "comfort"] as const;
const MOOD_KEYS = ["valence", "arousal", "stress"] as const;

// ...inside <thead><tr>:
{NEED_KEYS.map((k) => (<th key={k} className="px-2 py-1 capitalize">{k}</th>))}
{MOOD_KEYS.map((k) => (
  <th key={k} className="px-2 py-1 capitalize text-neutral-500">{k}</th>
))}

// ...inside <tbody><tr>:
{NEED_KEYS.map((k) => (
  <td key={k} className="px-2 py-1 font-mono">{agent.needs[k].toFixed(2)}</td>
))}
{MOOD_KEYS.map((k) => (
  <td key={k} className="px-2 py-1 font-mono text-neutral-500">{agent.mood[k].toFixed(2)}</td>
))}
```

The mood columns use `text-neutral-500` to visually separate "primary inputs" (needs) from "derived state" (mood).

The reducer in `apps/web/src/lib/sim/reducer.ts` needs no change — it copies `Snapshot` whole.

### Determinism

`systems::mood::update` reads `Needs` and writes `Mood`; consumes no randomness. The schedule's `chain()` makes the order deterministic: `needs::decay` → `mood::update`. Two `Sim` instances built from the same seed and same calls produce byte-equal `Snapshot`s. The existing `crates/core/tests/determinism.rs` continues to pass with snapshot-shape updates (mood values added to assertions).

## Module changes by crate

### `gecko-sim-core`

- **Modified:**
  - `src/agent/mod.rs` — `Mood` gains `bevy_ecs::component::Component` derive + `ts-rs` cfg_attr derives. Adds `Mood::neutral()` constructor.
  - `src/snapshot.rs` — `AgentSnapshot` grows `pub mood: Mood`.
  - `src/sim.rs` — `Sim` gains `schedule: Schedule` field, builds it in `new`, uses it in `tick`; `spawn_test_agent` adds `Mood::neutral()` to the spawn tuple; `snapshot` reads `Mood`.
  - `src/systems/mod.rs` — declare `pub mod mood;`.
  - `src/systems/needs.rs` — refactor `decay(&mut World)` → `decay(mut needs: Query<&mut Needs>)`.
- **New:**
  - `src/systems/mood.rs` — system #3 of 11. Houses `pub(crate) fn update(...)`, the `MOOD_DRIFT_RATE_PER_TICK` const, and the `mean`/`min` helpers.
- **Tests modified:**
  - `tests/snapshot.rs` — assertions on `AgentSnapshot` shape grow `mood` field.
  - `tests/determinism.rs` — snapshot equality already covers; no logical change.
  - `tests/needs_decay.rs` — system body changed; if the test calls `systems::needs::decay(&mut world)` directly, replace with `Sim::tick`-driven exercise (or a freshly-built `Schedule` with just `decay` registered; preferred).
  - `tests/catalogs.rs` — `Sim::new` signature unchanged; tests untouched.
- **Tests new:**
  - `crates/core/src/systems/mood.rs` `#[cfg(test)] mod tests` — unit tests on the `update` system using a hand-built `World`/`Schedule`. Cases:
    1. Full needs (`Needs::full()`): valence_target = 1, arousal_target = 0, stress_target = 0. After 1 tick from neutral, valence ≈ 0.01.
    2. Empty needs (all 0): valence_target = -1, arousal_target = 1, stress_target = 1. After 1 tick from neutral, valence ≈ -0.01.
    3. Worst-need = 0.6 (above stress threshold): stress_target = 0; mood.stress stays at 0.
    4. Worst-need = 0.2: stress_target ≈ 0.6; after 1 tick mood.stress ≈ 0.006.
    5. After 1000 ticks of empty needs, mood values approach (-1, 1, 1) within ε = 0.01 (saturation toward target).
  - `crates/core/tests/mood.rs` — integration test through `Sim`: spawn an agent, drop its needs to zero (via direct world manipulation in test scaffolding), run 500 ticks, assert valence < -0.5 and stress > 0.5.

### `gecko-sim-protocol`

- **Modified:**
  - `tests/roundtrip.rs` — every `AgentSnapshot` literal grows `mood: Mood::neutral()` (or a specific value). New `Mood` import. The JSON-roundtrip suite continues to lock the wire format.
- `Cargo.toml`: untouched.

### `gecko-sim-host`

- **Untouched.** `tests/ws_smoke.rs` works as-is — it asserts on tick advancement and agent count, not specific snapshot field values.

### `apps/web`

- **Modified:**
  - `src/components/AgentList.tsx` — three new columns.
  - `src/types/sim/{Mood,AgentSnapshot}.ts` — auto-regenerated by `pnpm gen-types`.
  - `src/lib/sim/reducer.test.ts` — fixture grows `mood` on the agent in `fixtureSnapshot`. Add one assertion that confirms the mood field round-trips through the reducer.
- **Untouched:** `connection.tsx`, `reducer.ts`, `ConnectionStatus.tsx`, `Controls.tsx`, `page.tsx`.

### Top-level

- Workspace `Cargo.toml`: untouched.
- `.cargo/config.toml`: untouched.

## Tests

### Rust unit tests (in `src/systems/mood.rs`)

Five cases listed above. Each builds a `World` with one entity carrying `(Needs, Mood)`, runs a `Schedule` with just `mood::update`, asserts mood deltas. Pure functions; no `Sim` overhead.

### Rust integration test (`crates/core/tests/mood.rs`)

```rust
//! Integration: mood drifts toward needs-derived target through Sim::tick.

use gecko_sim_core::{ContentBundle, Sim};

#[test]
fn mood_drifts_negative_under_starvation() {
    let mut sim = Sim::new(0, ContentBundle::default());
    let _id = sim.spawn_test_agent("Hungry");

    // Drop the agent's needs to zero by replacing the component.
    // (Test-only access; production has no API for this.)
    sim.set_needs_for_test_at(0, gecko_sim_core::Needs {
        hunger: 0.0, sleep: 0.0, social: 0.0,
        hygiene: 0.0, fun: 0.0, comfort: 0.0,
    });

    for _ in 0..500 {
        sim.tick();
    }

    let snap = sim.snapshot();
    let agent = &snap.agents[0];
    assert!(agent.mood.valence < -0.5, "valence={}", agent.mood.valence);
    assert!(agent.mood.stress  >  0.5, "stress={}",  agent.mood.stress);
}
```

This requires a small test-only public method `Sim::set_needs_for_test_at(agent_idx, needs)`. Marked `#[cfg(test)]` or `#[doc(hidden)]` per the existing `spawn_test_agent` convention. Lives next to `spawn_test_agent` in `sim.rs`.

### Frontend unit test (Vitest)

Update `reducer.test.ts`'s `fixtureSnapshot` helper to include `mood` on every agent. Add one new test case that constructs a `Snapshot` with non-neutral mood, dispatches `init`, and asserts `state.snapshot.agents[0].mood` matches the fixture.

### Determinism test

`crates/core/tests/determinism.rs` checks `Sim::snapshot()` equality across two seeded runs. With `mood::update` adding deterministic state to the snapshot, the test continues to pass. The assertion uses `PartialEq` over `Snapshot`, which is structural — no test changes required beyond the ones forced by the `AgentSnapshot` shape (which the determinism test doesn't construct manually; it only runs `tick` and compares snapshots).

### Manual smoke

After all commits land:

1. `cargo run -p gecko-sim-host`
2. `cd apps/web && pnpm dev`, open http://localhost:3000
3. Confirm: Valence/Arousal/Stress columns appear with values around `0.00` initially. Over ~200 ticks (~7 seconds at 30 Hz tick), valence drifts negative and arousal drifts positive as needs decay.
4. Click `64×` — drift accelerates.

## Definition of done

- `cargo build --workspace` clean.
- `cargo test --workspace` clean (unit + integration + determinism + smoke + needs_decay + snapshot tests all pass; new `mood` unit and integration tests pass).
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test -p gecko-sim-protocol --features export-ts` regenerates types; re-run is no-op (idempotent).
- `cd apps/web && pnpm test && pnpm tsc --noEmit && pnpm lint && pnpm build` — all clean.
- Manual smoke (above) passes; mood values drift visibly in the browser.
- Atomic jj commit chain matching the commit-strategy section below.

## Commit strategy

Six commits, each independently green:

1. `Mood: ECS Component derive + Mood::neutral() constructor`
2. `Mood: introduce bevy_ecs::Schedule in Sim; refactor needs::decay to Query parameter`
3. `Mood: systems::mood::update + unit tests`
4. `Mood: register mood::update in schedule; spawn_test_agent attaches Mood::neutral`
5. `Mood: AgentSnapshot grows mood; ts-rs regen; protocol roundtrip fixtures updated`
6. `Mood: frontend AgentList grows valence/arousal/stress columns; reducer test fixture updated`

Plan-author may merge commits 2 and 4 if the schedule wiring is small enough; the spec doesn't require strict atomicity beyond keeping each commit green.

## Trace to ADRs

- **ADR 0008 (time):** mood updates per tick (one sim-minute). `α = 0.01` chosen so mood reaches steady-state on a sim-hours timescale, matching the human-emotional intuition that mood doesn't flip on a sim-minute.
- **ADR 0010 (systems):** system #3 lands. Per-system update logic is tick-clean (no mid-tick mutation across systems — bevy `Schedule` enforces by design). Mood couples to needs (system #1) per ADR 0010's coupling list. Other listed couplings (events, weather, social) deferred.
- **ADR 0011 (schema):** `Mood { valence, arousal, stress }` field order and types unchanged. The `Mood::neutral()` constructor is implementation polish; not a schema change.
- **ADR 0012 (architecture):** introduces the `bevy_ecs::Schedule` pattern that ADR 0012 anticipates. `Sim::tick` becomes "run schedule"; further systems land via `schedule.add_systems(...)` rather than hand-edits in `tick()`.
- **ADR 0013 (transport):** `Snapshot` shape grows. JSON-over-WS continues; bandwidth grows by 12 bytes per agent per snapshot (3 × f32). Negligible.

## Deferred items

| Item | Triggers landing | Lives in |
|---|---|---|
| Mood spike → `PromotedEvent` | events system (system tbd) | `systems::mood` (emit) + `events::promoted` (channel) |
| Personality biases mood targets | personality system | `systems::mood` reads `Personality` query |
| Weather → mood (cold → comfort drop → mood drop) | weather macro var | `systems::mood` reads `Res<MacroState>` |
| Social interaction → mood delta | relationships system | new mood-effect entry from advertisement effects |
| Mood as decision-scoring modifier | decision-runtime pass | `decision::scoring` reads `Mood` query |
| Per-personality drift rate | first balancing pass | `systems::mood` reads personality |
| Active-speed echo from server | unrelated polish | protocol Hello/Init |

## What this pass enables next

- **First decision-runtime pass.** The decision system can now read `Mood` (via `Query<(&Needs, &Personality, &Mood, …)>`) when scoring advertisements. Mood is the first non-needs input the scorer expects.
- **Third sim system pass.** Adding `personality::system` is now mechanically the same shape — define an idiomatic bevy system, register it via `schedule.add_systems(personality::update.after(needs::decay))` or wherever it belongs in the dependency graph.
- **Frontend polish pass.** With three numeric columns visible per agent, a future UI pass might collapse them into a single mood emoji or color-coded badge.
