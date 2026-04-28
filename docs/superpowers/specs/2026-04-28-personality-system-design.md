# Personality system v0 — sample at spawn, propagate to wire, bias chair scoring

- **Date:** 2026-04-28
- **Status:** Draft
- **Scope:** Eighth implementation pass. Replaces the all-zeros `Personality::default()` placeholder with a real per-agent `Personality::sample(rng)` at spawn time. Propagates personality to the wire and the frontend. Lightly tweaks the chair seed content so the score formula's `personality_modifier` produces visibly different action choices across agents.
- **Predecessors:**
  - [`2026-04-28-mood-system-design.md`](2026-04-28-mood-system-design.md) — established the `bevy_ecs::Schedule` pattern, the lazy-shard ECS-component-on-schema-struct convention, and the wire-shape extension flow (snapshot grows → ts-rs regen → frontend column).
  - [`2026-04-28-decision-runtime-v0-design.md`](2026-04-28-decision-runtime-v0-design.md) — added `Personality` as an ECS component with `Default` derive (all zeros) and wired `personality_modifier` into the score formula. Today the modifier is always `1.0` because every agent has zero personality.

## Goal

End state:

1. `Personality::sample(rng: &mut impl Rng) -> Self` returns a uniformly-sampled Big Five vector with each component in `[-1, 1]`.
2. `Sim::spawn_test_agent_with_needs` calls `Personality::sample(...)` against the world's `SimRngResource` instead of stamping `Personality::default()`. Newly spawned agents have non-zero, agent-specific personalities.
3. `AgentSnapshot` grows a `personality: Personality` field. `ts-rs` regenerates `Personality.ts` and updates `AgentSnapshot.ts`.
4. The `chair.ron` "Sit" ad's `personality_weights` becomes `Personality { extraversion: -0.2, ... rest 0 }`, biasing introverts toward sitting and extraverts away. With the score formula's sensitivity `= 0.5`, an `extraversion = +1.0` agent's chair score is multiplied by `0.9`; an `extraversion = -1.0` agent's by `1.1` — a 20% spread.
5. Frontend `<AgentList>` grows five new columns (O / C / E / A / N) after the mood trio, displaying each Big Five component to 2 decimal places.
6. Manual smoke shows three agents with visibly different personality vectors, picking actions at meaningfully different rates over many ticks.

This is the smallest pass that flips Personality from "structurally present but functionally inert" to "first non-needs input the scorer responds to" while keeping the system static (no per-tick dynamics, no schedule additions, no Schedule reordering).

## Non-goals (deferred)

- **No `personality::update` system.** ADR 0010 says Personality is "static." There's nothing to update per tick. The pass adds zero entries to `Sim`'s schedule.
- **No personality dynamics from events / mood / experiences.** Trauma-shifts-personality, optimism-from-happiness — all post-v0.
- **No content-driven personality priors.** v0 uses uniform `[-1, 1]` per component, hard-coded in `Personality::sample`. ADR 0011 mentions "configured prior distribution"; that configuration plumbing lands when we have a content reason for it.
- **No client-side caching of immutable per-agent state.** Personality, name, age, gender, intrinsic appearance — all immutable per agent — re-ship on every snapshot at v0. Optimization is its own pass when bandwidth bites (per ADR 0013's threshold).
- **No frontend tooltips or polished personality panel** (Big Five bar charts, gauge widgets). Plain numeric grid in v0; that's a UI polish pass.
- **No personality biasing on systems other than scoring.** Memory's "what feels notable", relationships' "compatibility", mood's "personality-tuned drift rate" — all deferred to those systems' future passes.
- **No content tweaks to fridge.ron.** Food is a need, not a preference. Leave `EatSnack`'s `personality_weights` at all-zero.
- **No retiring `spawn_test_agent` / `spawn_test_agent_with_needs`.** Same placeholder convention as agent-generation; replaced when content-driven spawning lands.

## Architecture

### `Personality::sample`

In `crates/core/src/agent/mod.rs`, on the existing `Personality` struct:

```rust
impl Personality {
    /// Sample a Big Five personality from the uniform distribution on
    /// `[-1, 1]^5` per ADR 0011's "roughly uniform, centered on zero"
    /// default. Five draws from the supplied RNG; deterministic for
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

(rand 0.9 uses `random_range`, not the deprecated `gen_range`. Same convention adopted in the decision-runtime pass.)

`Default` derive stays — useful for unit tests that build hand-crafted worlds and want zero personality. `sample` is the production path.

### `Sim::spawn_test_agent_with_needs` updates

Currently:

```rust
pub fn spawn_test_agent_with_needs(&mut self, name: &str, needs: Needs) -> AgentId {
    let id = AgentId::new(self.next_agent_id);
    self.next_agent_id += 1;
    self.world.spawn((
        Identity { id, name: name.to_string() },
        needs,
        Mood::neutral(),
        Personality::default(),
        CurrentAction::default(),
        RecentActionsRing::default(),
    ));
    id
}
```

After this pass:

```rust
pub fn spawn_test_agent_with_needs(&mut self, name: &str, needs: Needs) -> AgentId {
    let id = AgentId::new(self.next_agent_id);
    self.next_agent_id += 1;
    let personality = {
        let mut rng = self.world.resource_mut::<SimRngResource>();
        Personality::sample(&mut rng.0.0)
    };
    self.world.spawn((
        Identity { id, name: name.to_string() },
        needs,
        Mood::neutral(),
        personality,
        CurrentAction::default(),
        RecentActionsRing::default(),
    ));
    id
}
```

The borrow of `SimRngResource` is scoped to a block so it's dropped before the subsequent `world.spawn`. Five RNG draws happen per spawn. Determinism: spawn order + RNG sub-stream is deterministic from `Sim::new(seed, ...)`.

### Wire shape: `AgentSnapshot.personality`

In `crates/core/src/snapshot.rs`:

```rust
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
    pub mood: Mood,
    pub personality: Personality,                  // ← new
    pub current_action: Option<CurrentActionView>,
}
```

Field order matches ADR 0011's per-system ordering: 1 (needs) → 3 (mood) → 2 (personality) → decision-runtime view. Slight inversion vs. ADR-numbering but groups "live state" before "static profile" before "in-flight action" — readable on the wire.

`Personality` already has the `cfg_attr(feature = "export-ts", ...)` derives wired in the decision-runtime pass (well, actually it doesn't — `Personality` was added as a `Component` only, no ts-rs). **Add the ts-rs derives** in this pass. The two `cfg_attr` lines are identical to those on `Mood`, `Needs`, etc.

`Sim::snapshot` projects personality the same way it projects mood — direct read of the ECS component:

```rust
let personality = entity_ref.get::<Personality>().copied().unwrap_or_default();
```

(`Personality` is `Copy + Default`. The unwrap_or_default is defensive — in practice all agents have personality from spawn.)

### Seed-content tweak: chair.ron

Change `content/object_types/chair.ron`'s `Sit` ad from:

```ron
score_template: ScoreTemplate(
    need_weights: [(Comfort, 1.0)],
    personality_weights: Personality(
        openness: 0.0, conscientiousness: 0.0,
        extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
    ),
    situational_modifiers: [],
),
```

to:

```ron
score_template: ScoreTemplate(
    need_weights: [(Comfort, 1.0)],
    personality_weights: Personality(
        openness: 0.0, conscientiousness: 0.0,
        extraversion: -0.2, agreeableness: 0.0, neuroticism: 0.0,
    ),
    situational_modifiers: [],
),
```

The single change: `extraversion: 0.0` → `extraversion: -0.2`. Negative-weighted means introverted agents (negative extraversion) get a positive modifier, extraverted agents a negative one. With `sensitivity = 0.5`, the score multiplier is `(1.0 + 0.5 * extraversion * (-0.2)) = (1.0 - 0.1 * extraversion)`. That's a 10% spread per unit of extraversion — at the [-1, +1] extremes, modifiers run from `0.9` to `1.1`. Modest but observable across many ticks.

`fridge.ron` is unchanged — food is a need, not a personality preference.

### `seed_loads.rs` continues to pass

The seed-loads test asserts only catalog counts (2 object types, 2 accessories), not specific advertisement field values. Safe.

### Frontend: O / C / E / A / N columns

In `apps/web/src/components/AgentList.tsx`, after the existing mood column block:

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

// in <thead><tr>, after the mood headers:
{PERSONALITY_KEYS.map((k) => (
  <th key={k} className="px-2 py-1 text-neutral-500" title={k}>
    {PERSONALITY_LABELS[k]}
  </th>
))}

// in <tbody><tr>, after the mood cells:
{PERSONALITY_KEYS.map((k) => (
  <td key={k} className="px-2 py-1 font-mono text-neutral-500">
    {agent.personality[k].toFixed(2)}
  </td>
))}
```

The header `title={k}` gives a hover tooltip with the full word. Same `text-neutral-500` styling as mood — visually marks personality as "static" alongside mood's "derived" rather than active needs.

The `<Doing>` column stays at the right edge after personality.

### Determinism

Single source of randomness: `SimRngResource(PrngState)`. Spawn order is sequential (host calls `spawn_test_agent("Alice")`, then `("Bob")`, then `("Charlie")`). RNG draws are deterministic. `tests/determinism.rs` continues to pass — same seed produces byte-equal snapshots.

### Reducer test fixture growth

`apps/web/src/lib/sim/reducer.test.ts`'s `fixtureSnapshot` helper grows a `personality: { openness: 0, conscientiousness: 0, extraversion: 0, agreeableness: 0, neuroticism: 0 }` field. New round-trip test asserts a non-zero personality flows through the reducer.

## Module changes by crate

### `gecko-sim-core`

- **Modified:**
  - `src/agent/mod.rs` — `Personality` gets `cfg_attr(feature = "export-ts", derive(ts_rs::TS))` + `cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))` derives. `impl Personality { pub fn sample(rng: &mut R) -> Self }` added.
  - `src/sim.rs` — `spawn_test_agent_with_needs` calls `Personality::sample(&mut sim_rng.0.0)` instead of `Personality::default()`. (Imports `Personality` already in scope.)
  - `src/snapshot.rs` — `AgentSnapshot` grows `pub personality: Personality`. Add `Personality` to the existing `crate::agent::{Mood, Needs}` import.
- **New:** none.
- **Tests modified:**
  - `tests/decision.rs::agent_eats_from_fridge_when_hungry` — should still pass; the test seeds hunger to 0.3 and personality_modifier on EatSnack remains `1.0` (its personality_weights are zero). Verify but expect green.
- **Tests new:**
  - `crates/core/src/agent/mod.rs` `#[cfg(test)] mod tests` (or extend an existing block) — unit tests on `Personality::sample`:
    1. All five components in `[-1, 1]` after one sample.
    2. Same seed → same Personality (determinism).
    3. Two consecutive samples differ (independent draws).

### `gecko-sim-protocol`

- **Modified:**
  - `tests/roundtrip.rs` — fixture grows `personality: Personality::default()` on every `AgentSnapshot` literal. New `agent_snapshot_with_personality_roundtrips` test with non-default values.

### `gecko-sim-host`

- **Untouched.** The host's `cargo run -p gecko-sim-host` already prints `sim primed agents=3`; the personality propagates to snapshots automatically.

### `apps/web`

- **Modified:**
  - `src/types/sim/{Personality,AgentSnapshot}.ts` — auto-regenerated by `pnpm gen-types`.
  - `src/components/AgentList.tsx` — five new columns (O/C/E/A/N).
  - `src/lib/sim/reducer.test.ts` — fixture grows `personality`; new round-trip test.

### `content/`

- **Modified:** `content/object_types/chair.ron` — `extraversion: -0.2` on the `Sit` ad's `personality_weights`.

## Tests

### Rust unit tests (in `agent/mod.rs::tests` or a new submodule)

Three cases:

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
                p.openness, p.conscientiousness, p.extraversion,
                p.agreeableness, p.neuroticism,
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

### Rust integration test

`tests/decision.rs::agent_eats_from_fridge_when_hungry` — re-run as-is, expect green. Rationale: the EatSnack ad has all-zero `personality_weights`, so the personality_modifier stays at `1.0` regardless of the agent's sampled personality. Hunger pressure dominates → EatSnack still wins → hunger restores by 0.4. No code change needed.

### Frontend Vitest test

Update `apps/web/src/lib/sim/reducer.test.ts`:

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

New test:

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

### Manual smoke

After all commits land:

1. `cargo run -p gecko-sim-host` — host boots, logs `seed instances spawned object_count=2`.
2. `cd apps/web && pnpm dev` — open http://localhost:3000.
3. Confirm five new columns (O/C/E/A/N) populated with non-zero, distinct values for Alice/Bob/Charlie.
4. Click `64×`. Watch over a few minutes: introverted agents (extraversion negative) trend toward picking `Sit` more often than extraverted agents. Subtle but observable across many tick cycles.

## Definition of done

- `cargo build --workspace` clean.
- `cargo test --workspace` clean — existing tests + 3 new personality unit tests + protocol roundtrip with personality fixture pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test -p gecko-sim-protocol --features export-ts` regenerates types; idempotent.
- `cargo test -p gecko-sim-core --features export-ts` regenerates types; idempotent.
- `cd apps/web && pnpm test && pnpm tsc --noEmit && pnpm lint && pnpm build` clean.
- `crates/content/tests/seed_loads.rs` continues to pass with the chair.ron tweak.
- Manual smoke: 5 personality columns visible, non-zero, distinct per agent. At 64× speed, agents' action distributions diverge over time.
- 5-commit chain matching the commit-strategy section below.

## Commit strategy

5 commits:

1. `Personality: ts-rs derives + Personality::sample(rng) + unit tests`
2. `Personality: spawn_test_agent_with_needs samples Personality from SimRngResource`
3. `Personality: chair.ron Sit ad gets extraversion: -0.2 weight (introverts prefer sitting)`
4. `Personality: AgentSnapshot grows personality; ts-rs regen; protocol roundtrip fixtures updated`
5. `Personality: frontend AgentList grows OCEAN columns; reducer fixture + round-trip test`

Plan-author may merge commits 1 and 2 (both touch core agent/sim plumbing) but they're cleanly separable: commit 1 is pure type addition, commit 2 wires the spawn helper. Keeping them separate documents intent.

## Trace to ADRs

- **ADR 0010 (systems inventory):** system #2 (`personality`) lands as a "static" system with no per-tick dynamics. The "system" is just sampling at spawn; no schedule entry. Couples to decision-runtime via the score formula's `personality_modifier`, which the decision-runtime pass already wired.
- **ADR 0011 (schema):** Big Five `(openness, conscientiousness, extraversion, agreeableness, neuroticism)` each in `[-1, 1]`, sampled "from a configured prior distribution (default roughly uniform, centered on zero)". This pass uses uniform `[-1, 1]` per component as the v0 default. Configurable priors deferred.
- **ADR 0013 (transport):** wire shape grows additively. `Snapshot` bandwidth grows by 20 bytes per agent (5 × f32). Negligible per the spec's "comfortable on local" margin.
- **ADR 0008 (time):** sampling happens at `spawn_test_agent` time (pre-tick); no per-tick implications.

## Deferred items

| Item | Triggers landing | Lives in |
|---|---|---|
| Configurable personality priors (per-population, per-district, age-stratified) | Migration/scenario pass | content RON + `Personality::sample_from_prior(...)` |
| Personality dynamics (life-event shifts, trauma) | Memory or events pass | new `systems::personality::*` if dynamics arrive |
| Personality biases on memory importance | Memory system pass | `systems::memory::generate` reads `Personality` |
| Personality biases on relationship compatibility | Relationships pass | `systems::relationships::*` |
| Personality-tuned mood drift rates | Mood polish pass | `systems::mood::update` reads `Personality` |
| Client-side caching of immutable per-agent state (personality, name, age, intrinsic appearance) | Bandwidth optimization pass | New `Init`-only payload + `Snapshot` slimming |
| Polished personality panel (gauges, bar charts, tooltips) | Frontend polish pass | `apps/web/src/components/PersonalityPanel.tsx` |
| Non-zero personality_weights on more advertisements | Each ad's content tuning | seed RON files |

## What this pass enables next

- **Decision-runtime polish.** With personality non-zero, the scoring formula's full breadth becomes observable. Tuning personality_weights on ads becomes a real lever for emergent behavior.
- **Memory system pass** — once memories carry `importance` derived from personality, agent-specific story arcs become possible.
- **Personality dynamics pass** — if/when life events shift personality, that becomes a real `personality::update` system entry in the schedule.
- **Frontend polish pass** — the OCEAN columns at v0 are utilitarian. A `<PersonalityPanel>` with bar charts or radar plots would make agent profiles visually scannable.
