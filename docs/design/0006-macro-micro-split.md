# 0006 — Macro/micro split

- **Status:** Accepted (shape TBD)
- **Date:** 2026-04-26

## Context

Some phenomena (housing policy, crime rates, economic cycles, disasters, weather) operate at a scale above individual agents. Pure bottom-up simulation from agent behavior is intractable for some of these and often loses legibility.

## Decision

**Two-layer simulation.**

- **Macro layer** — coarser, cheaper simulation tracking aggregate state: population stats, crime rate, housing prices, employment, policy variables, weather/disaster state.
- **Micro layer** — agents and smart objects, simulated in full detail (see 0004).
- **Macro → Micro** — macro state pushes down to agents: modifies need-decay rates, injects available actions, alters event probabilities, gates policy-dependent behaviors.
- **Micro → Macro** — agent activity rolls up into aggregate stats; specific notable events may also be promoted upward.

## Open questions

- Exact contract between layers: which stats live in macro, what update cadence, which events promote up. Subject of a later doc.
- Whether macro is "always running" or only updates on a slower tick.
- How macro state is authored vs simulated vs scripted (e.g. is "policy" a player input, an emergent macro variable, or both?).
