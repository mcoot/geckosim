# 0005 — Emergence style

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

"Emergence" can mean several things, and they bias the architecture differently:

- **Story emergence** (Dwarf Fortress) — specific agents with specific histories accumulate into legends.
- **Situational emergence** (The Sims) — comedic / unexpected moments from the combinatorics of object × need × context.
- **System emergence** (SimCity) — macro patterns (traffic, economic cycles) from micro rules.

## Decision

**Balance of DF-style story emergence and Sims-style situational emergence.** System emergence is welcome but not the primary goal.

## Consequences

- Persistent per-agent memory of specific events and individuals is a first-class system, not just a stat (drives story emergence).
- Smart-object catalog and combinatorial action space are first-class (drives situational emergence).
- Many small interacting systems (target ~8–12 for v0; subject of a future systems-inventory doc) — emergence is a product of system count multiplied together, not depth of any single system.
- Rare events with permanent consequences are valuable. Most ticks will be boring; that's correct.
