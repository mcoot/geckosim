# 0008 — Time model

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

The sim must support visually smooth observation, fast time-skip for emergent storytelling, deterministic replay, and cheap per-tick decision-making for hundreds of agents (0003, 0004).

## Decision

**Hybrid coarse-tick simulation with renderer interpolation.**

- **One sim tick = 1 sim-minute.** Coarse enough for cheap event-driven decisions; fine enough for meaningful action durations.
- **Renderer interpolates** agent positions and animations between sim snapshots for visual smoothness, independent of sim tick rate.
- **Speed multipliers:** 0× (paused), 1×, 4×, 16×, 64×. Higher speeds run more sim ticks per real second.
- **At 1×, one sim tick per real second** → one sim-day ≈ 24 real minutes. Tunable.
- **At 64×**, one sim-day ≈ 22.5 real seconds — supports rapid time-skip.

### Determinism

- **Snapshot per tick boundary** — the entire sim state is well-defined at integer tick numbers.
- **Seeded RNG** — global seed plus per-agent and per-system sub-seeds derived from it.
- **Replay** = same seed + same player inputs applied at the same tick numbers → identical outcome.
- Saves are full state at a tick boundary; no mid-tick saves.

### Action durations & decisions

- Actions have integer-tick durations.
- Agents commit to an action and only re-decide on **action completion**, **interrupt**, or a **need crossing a threshold** (the event-driven model from 0004). Most agents are not making a decision on most ticks.
- Long actions (e.g. "sleep for 8 hours" = 480 ticks) are one committed action, not 480 micro-decisions.

## Consequences

- The sim ↔ frontend protocol sends state per tick (or deltas); the renderer handles visual smoothing.
- Player inputs are tick-stamped to preserve replay determinism.
- Off-screen / fast-forwarded geckos still tick in full — no separate "abstracted" mode at v0 scale.

## Open questions

- Whether sim ticks are wall-clock-driven (cleaner for renderer interpolation) or run-as-fast-as-possible at high speeds (better for headless time-skip). Likely both modes.
- Whether the **macro layer (0006)** ticks on the same clock or a coarser one (e.g. once per sim-hour). Subject of the macro/micro seam doc.
