# 0002 — Tech stack

- **Status:** Accepted
- **Date:** 2026-04-26

## Context

The sim needs to run hundreds of agents at high tick rates with deterministic behavior, and present a 3D view that supports exploration and interaction.

## Decision

- **Simulation core:** Rust. Performance, determinism, mature ecosystem for simulation/ECS work.
- **Frontend:** Next.js + React.
- **3D view:** Three.js (likely via react-three-fiber).
- **Sim ↔ frontend transport:** TBD — likely WebSocket with snapshot + delta protocol.

## Rationale

Splitting the authoritative sim from the renderer keeps the renderer a pure view of seeded, deterministic state. The Rust core can run as a separate native process, be embedded in a desktop shell, or eventually compile to WASM for browser-only deployment.

## Open questions

- Sim host: separate native process (with IPC) vs WASM in browser vs both.
- Three.js at city scale needs aggressive instancing/LOD; acceptable for hundreds of agents but worth re-checking once visual fidelity is decided.
