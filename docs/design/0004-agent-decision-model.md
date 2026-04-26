# 0004 — Agent decision model

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

We need an AI architecture that produces emergent, surprising behavior at hundreds-of-agents scale (see 0003), while staying cheap enough to tick frequently and supporting the emergence goals in 0005.

## Decision

**Utility AI + smart objects + persistent personality/memory.**

1. **Smart objects.** Entities in the world advertise possible actions (e.g. fridge → "eat: +60 hunger, requires kitchen access"). Agents query the environment for offered actions; agents do not hard-code knowledge of object types.
2. **Per-agent state.** Needs (continuous decay), personality (static traits weighting scoring), memory (events, relationships, grudges, debts).
3. **Scoring.** Each candidate action scores roughly as `need_pressure × personality_weight × situational_context`, plus a small noise term and a recency penalty. Pick weighted-random from top-N rather than strict argmax.
4. **Schedules and goals.** Sit on top as forcing functions. A job adds "be at office 9-5" as a virtual need; long-term goals decay slowly and bias decisions.
5. **Macro events.** From the macro layer (0006) — modify need-decay rates, inject new advertised actions, or alter personality weights.

## Rationale

- Smart objects mean adding a new object/situation creates new behavior with no agent-code changes — the single biggest lever for emergence.
- Utility AI is cheap, naturally emergent, and reactive.
- Memory + personality is what creates *specific* gecko stories rather than interchangeable ones.
- Hybrid avoids the failure modes of single approaches: behavior trees too scripted, GOAP too expensive, pure utility AI too repetitive.

## Consequences

- We need a data-driven format for advertised actions (likely RON or similar, loaded by Rust).
- Decisions are event-driven, not per-tick — re-decide on action completion, interrupt, or need crossing a threshold.
- We may eventually need a thin GOAP-like layer for long-horizon plans (e.g. "save up for a house"). Defer until needed.
- Determinism requires per-agent seeded RNG.
