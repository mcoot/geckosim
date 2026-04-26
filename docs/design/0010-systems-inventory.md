# 0010 — Systems inventory (v0)

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

0005 asserted that DF-style emergence comes from many small systems multiplying and committed to ~8–12 systems for v0. This doc fixes the v0 list with brief responsibilities and key couplings. Concrete data shapes belong in the schema doc that follows.

## Decision

**11 systems for v0**, grouped by what they touch.

### Inner life

1. **Needs** — physiological / psychological drives that decay continuously and motivate baseline action selection. v0 set: **hunger, sleep, social, hygiene, fun, comfort**. Couples with: decisions (input to scoring), mood, smart objects (effect targets).

2. **Personality** — static trait vector that biases scoring across the agent's lifetime. The main differentiator between two agents in the same situation. Couples with: decisions, mood, relationships, memory (which events feel notable).

3. **Mood** — short-term emotional state, modeled as a **low-dimensional vector: (valence, arousal, stress)**. Reactive to needs, events, weather, social interactions. Couples with: decisions (score modifier), memory (mood-tinted recall), promoted events (extreme states may trigger).

4. **Memory** — episodic record of notable events (typed, timestamped, participants, location, valence). Bounded; lower-importance / older memories decay. Couples with: relationships (memories about specific others), decisions (recency-weighted bias), promoted events.

### Social

5. **Relationships** — directed pairwise edges between agents: affinity, trust, familiarity, last-interaction. Sparse graph (only non-trivial pairs stored). **Reputation is per-observer affinity, not a separate system.** Couples with: memory, decisions (who to socialize with), crime (witnesses, victims, perpetrators).

6. **Skills** — abilities that improve with use; gate jobs and certain actions. Long-term per-agent progression vector. Couples with: jobs (eligibility, performance), smart objects (action gating), memory (skill milestones).

### Material life

7. **Money & personal economy** — wealth + bounded transaction history per agent. Couples with: jobs (income), housing (rent/mortgage), smart objects (priced actions), macro pricing (0009).

8. **Housing & households** — per-agent residence; per-building occupancy; household groupings. Couples with: macro housing prices, money (rent/mortgage), relationships (household members), needs (home-only actions).

9. **Jobs & employment** — roles, schedules, hiring/firing, performance, pay. Per-agent job reference; per-building employer slots. Couples with: skills (eligibility), money (income), schedules (forcing functions on decisions), macro employment rate.

### Embodiment

10. **Health** — illness, injury, recovery, death. Condition list per agent (each with severity + recovery dynamics). The system that ends agent life cycles. Couples with: needs (illness affects decay rates), jobs (sick days), promoted events (death), smart objects (medical actions).

### Antisocial

11. **Crime & justice** — illegal actions, witnesses, consequences, enforcement. Incident log + per-agent criminal record + pending consequences. Couples with: relationships (witnesses, victims), money (fines, theft), housing (incarceration), promoted events (crimes), macro crime rate.

## Cross-cutting

- **Inventory.** Each agent has a **lightweight typed inventory** — a small slot list of typed items (food, work materials, stolen goods, gifts). Enables theft mechanics, gift-giving, and contraband cheaply. Not a generalized item economy — items are typed enums plus optional metadata, not parametric objects. Not a "system" with its own dynamics; a data feature consumed by other systems.

## Explicitly out of scope for v0 (deferred)

- **Family, reproduction, romance.** Pregnancy / parenting / child-rearing / romantic relationships as full systems are post-v0. **Population renewal at v0 happens entirely via macro-driven migration** — fully-formed adult geckos arrive with procedurally generated state (see 0011's "Agent generation"). `Birth` is *not* a v0 promoted event; it returns when reproduction lands.
- **Education.** Schools, formal learning, child progression — post-v0.
- **Transit & vehicles.** Walking only (per 0007).
- **News / gossip propagation.** Promoted events surface to macro directly; agent-to-agent rumor mill is post-v0.
- **Per-agent macro perception.** Agents read macro state directly (per 0009).
- **Generalized item economy.** Inventory is a typed slot list; no parametric item descriptions.

## How emergence is supposed to happen

Couplings, not depth. Examples:

- **Vendetta loop:** bad mood × memory of a slight × opportunity (smart object advertises "confront") → fight → injury → memory → grudge → vendetta.
- **Downward spiral:** lost job → no income → can't pay rent → forced housing change → household relationship strain → mood drop → poor performance at next job.
- **Policy backfire:** crime spike (macro) → curfew policy → smart objects gate evening actions → bored agents (fun unmet) → underground social scene → more crime.

Each system is small. The product is what's interesting.

## Consequences

- The schema doc (next) defines concrete data shapes for per-agent state across these 11 systems plus inventory.
- Each system needs at minimum: per-tick or event-driven update logic, integration with the decision-scoring loop (0004), and coupling specifications to other systems.
- v0 will explicitly *not* try to be deep in any one system. If a system feels thin individually, that's correct — depth comes later.
