# 0003 — Scale target

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

Population scale gates almost every architectural decision — data structures, AI complexity, rendering strategy, transport bandwidth.

## Decision

**Hundreds of agents (Dwarf-Fortress-style scale).** Roughly 100–1000 fully simulated geckos for v0. Big enough that scaling pressure is real and emergent behavior shows up; small enough that every agent can be fully simulated rather than statistically aggregated.

## Rationale

- Sims-scale (≤10s of agents) is too small to get system-level emergence.
- SimCity-scale (10k+ agents) forces statistical/aggregate simulation, which kills the "specific gecko, specific story" quality (see 0005).
- DF-scale is the sweet spot for our emergence goals.

## Consequences

- Per-tick decision-making for every agent is feasible.
- We don't need agent tiering / off-screen simplification at v0 (revisit if measured otherwise).
- The macro layer (see 0006) is useful for legibility, not load-bearing for tractability.
