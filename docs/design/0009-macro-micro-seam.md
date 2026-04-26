# 0009 — Macro/micro seam

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

0006 committed to a two-layer simulation (macro for aggregate phenomena, micro for individual agents and smart objects) but left the contract between layers TBD. This doc fixes that contract.

## Decision

### Cadence

- **Macro ticks every 60 sim-ticks (1 sim-hour).** Most macro variables don't change minute-to-minute; hour cadence matches the natural rate of change for prices, employment, weather shifts, crime rates.
- Macro ticks happen at integer-multiple sim-tick boundaries (60, 120, 180, …) — preserves determinism from 0008.
- Things needing finer granularity (active weather effects, an in-progress fire) live as micro events that aggregate into macro at each macro tick.

### Two flavors of macro state

- **Simulated state** — has its own dynamics, advanced by the macro tick, **part of the save**. Examples: weather, prices, ongoing events, policy effects propagation.
- **Derived aggregates** — pure functions of current micro state, recomputed each macro tick, **not saved separately** (always reproducible from a micro snapshot). Examples: population, employment rate, occupancy.

### Macro → micro: pull by default, push only for interrupts

- **Pull (default).** Macro state is a cheap, read-only context (`MacroContext`). Agents and smart objects reference macro variables as inputs to scoring. Keeps the seam thin and avoids propagation steps.
- **Push (interrupts only).** Forced actions (evacuate, eviction, mandatory shift) are injected as agent interrupts — same mechanism as need-threshold interrupts from 0004; the macro layer is just another source.
- **Smart-object macro gating.** Smart objects evaluate conditions against macro state at advertising time (e.g. "open if curfew not in effect"). This is how policy and events reshape the available action space without modifying agent code.

### Micro → macro: aggregation + promoted events

- **Aggregation sweep** at each macro tick. Population, employment rate, occupancy, average wage — recomputed by sweeping micro state. Cheap at hundreds-of-agents scale.
- **Promoted events** — explicit emission from agents/systems for notable specifics. Buffered between macro ticks; processed at the next macro tick boundary. May trigger macro state changes (crime spike, reputation drop, news beat).

### Promoted-event taxonomy (v0)

Small typed enum, grow as needed:

- `Crime` (subtype: theft, assault, murder, …)
- `Death`
- `Birth`
- `Fire`
- `Disaster` (subtype)
- `NotableAchievement` (catch-all for unusual positive events)

Each event carries minimal payload — involved agent IDs, location, timestamp, type-specific fields.

### Pricing & economy

**Macro-tick supply/demand model.** At each macro tick, prices for tracked goods/services adjust based on aggregated supply and demand from the previous interval. Not per-transaction emergent (too wild for v0); not fixed-table (too static).

### Macro-state visibility

**Full visibility for v0.** All agents can read all macro state during decision scoring. Acknowledged simplification — in reality a gecko doesn't know the city-wide crime rate, only a personal estimate. A per-agent perception layer can be added later if it matters; for v0 the noise term and personality weighting in scoring (0004) cover most of the realism gap.

### Player policy as first-class macro state

Policy variables (tax rate, zoning per district, welfare level) are macro state the player directly mutates. Changes propagate through the normal macro-tick mechanisms — same code path as "a hurricane arrived." Player-driven micro interventions go through the agent interrupt channel.

### Determinism

- Macro ticks at deterministic sim-tick boundaries (consistent with 0008).
- Macro RNG is a seeded sub-stream of the world seed.
- Derived aggregates are deterministic functions of micro snapshots — not saved, always reproducible.

## v0 macro variable set (~20)

Intentionally tight. Each variable comes with a cost: somewhere, agent scoring or smart-object gating must react to it.

**Per district** (5 districts × 5 variables):

- Population
- Crime rate
- Housing price (avg)
- Vacancy rate
- Ambient reputation / vibe

**City-wide:**

- Employment rate
- Average wage
- Cost-of-living index
- Current weather

**Active events** (variable-length list):

- Ongoing disasters, festivals, public events

**Policy (player inputs):**

- Tax rate
- Zoning per district (small enum)
- Welfare level

## Macro tick — order of operations

1. Recompute derived aggregates from current micro snapshot.
2. Ingest buffered promoted events; update simulated state accordingly.
3. Advance simulated dynamics (weather model step, supply/demand pricing step, ongoing-event progression).
4. Apply pending player policy changes.
5. Publish updated `MacroContext` for micro reads in the next 60 ticks.

Revisit ordering if cycles appear.

## Consequences

- Agents have read access to a `MacroContext` during decision scoring.
- Smart-object advertising functions can read `MacroContext` when evaluating preconditions.
- A `PromotedEvent` type and emission channel must exist before macro-affecting micro systems can be added.
- Save format includes simulated macro state; excludes derived aggregates.
- Macro tick is a phase in the sim loop: micro ticks; at each 60th micro tick, macro tick runs.

## Open questions

- **Per-agent perception.** Deferred. Add only if "all agents know everything" produces noticeably wrong behavior.
- **Pricing model granularity.** Which goods/services are tracked? At minimum housing, food, labor — finalized in the systems-inventory doc.
- **Player intervention UX.** How players actually edit policy (UI) is out of scope here; the contract above only requires policy live in macro state.
