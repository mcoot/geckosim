# Action Memories Design

- **Status:** Approved
- **Date:** 2026-05-02
- **Workspace:** `/Users/joseph/src/geckosim/.workspaces/nonrenderer-pass-planning`

## Context

The renderer pass is already active in a separate jj workspace, so this pass should avoid frontend renderer files, TypeScript bindings, and snapshot/protocol churn where possible.

The existing sim already has the schema pieces for memories:

- `MemoryEntry` and `MemoryKind` in `crates/core/src/agent/mod.rs`
- `Effect::MemoryGenerate` in `crates/core/src/object/mod.rs`
- `MemoryEntryId` in `crates/core/src/ids.rs`
- ADR 0010's memory system goal: episodic records of notable events
- ADR 0011's cap and eviction shape: `importance * recency_factor`

Runtime memory generation is still missing. `decision::effects::apply` currently treats `Effect::MemoryGenerate` as an unsupported no-op.

## Goal

Land the first memory-system slice: completed smart-object actions can append deterministic per-agent `MemoryEntry` records through `Effect::MemoryGenerate`.

This is intentionally a foundation pass. Memories are recorded but do not yet affect decision scoring, relationships, promoted events, or frontend display.

## Non-goals

- No memory influence on action scoring.
- No relationship updates from memories.
- No memory panel or snapshot/protocol exposure.
- No multi-agent or nearby-agent target resolution beyond the minimal v0 behavior.
- No save/load format work beyond using the existing serializable schema types.
- No memory decay tick system. Eviction uses recency when appending past the cap; stored entries do not mutate over time.

## Recommended approach

Implement core memory generation plus one minimal seed-content hook.

1. Add an ECS `Memory` component wrapping a `Vec<MemoryEntry>`.
2. Add a deterministic memory ID allocator resource inserted by `Sim::new`.
3. Attach `Memory::default()` when spawning test agents.
4. Teach effect application to handle `Effect::MemoryGenerate`.
5. Update the seed fridge "Eat snack" advertisement to generate a small `Routine` memory.
6. Add core tests that inspect memory directly through test helpers, not through the wire.

This keeps the work mostly inside `crates/core/src/agent/mod.rs`, `crates/core/src/systems/memory.rs`, `crates/core/src/systems/decision/{effects,execute}.rs`, `crates/core/src/sim.rs`, and one content file.

## Architecture

`Memory` is a lazy-sharded ECS component, matching the style already used for `Needs`, `Mood`, `Personality`, `Position`, and `RecentActionsRing`.

```rust
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Memory {
    pub entries: Vec<MemoryEntry>,
}
```

Memory mutation lives in a focused `crates/core/src/systems/memory.rs` module. That module owns:

- `MEMORY_CAP: usize = 500`
- `push_memory(memory, entry, current_tick)`
- clamping helpers for `importance` and `valence`
- eviction scoring and deterministic tie-breaks

ID allocation lives in a small ECS resource inserted by `Sim::new`:

```rust
#[derive(bevy_ecs::prelude::Resource, Debug, Default)]
pub struct MemoryIdAllocator {
    next: u64,
}
```

The allocator exposes `allocate() -> MemoryEntryId`. This keeps action completion deterministic because `decision::execute` runs in schedule order and allocates IDs only when completed actions apply effects normally.

## Data flow

The action-completion path stays the single source of truth:

1. `decision::execute` finds an object-targeted committed action whose `Phase::Performing` has reached `expected_end_tick`.
2. It resolves the smart-object advertisement.
3. It applies each effect atomically to the acting agent.
4. For `Effect::MemoryGenerate`, effect application appends a `MemoryEntry` to that agent's `Memory` component.
5. The recent-actions ring is updated as it is today.
6. `CurrentAction` is cleared.

The generated memory fields are:

- `id`: next `MemoryEntryId` from `MemoryIdAllocator`
- `kind`: from `Effect::MemoryGenerate.kind`
- `tick`: current tick
- `participants`: resolved from `Effect::MemoryGenerate.participants`
- `location`: actor's current `Position.leaf`
- `valence`: effect value clamped to `[-1.0, 1.0]`
- `importance`: effect value clamped to `[0.0, 1.0]`

Participant resolution is deliberately small at v0:

- `TargetSpec::Self_` resolves to the acting agent's `AgentId`.
- Other targets resolve to an empty list for this pass.

That lets content start emitting self/action memories now while leaving relationship-aware and nearby-agent target resolution to the relationship/social pass.

## Seed content

The fridge `Eat snack` advertisement gets one memory effect:

```ron
MemoryGenerate(
    kind: Routine,
    importance: 0.2,
    valence: 0.35,
    participants: Self_,
)
```

This gives the integration test a real content-driven memory without inventing a new object type or touching renderer-facing data.

## Eviction

The memory cap is 500 entries per agent, matching ADR 0011. Appending the 501st entry evicts exactly one entry.

Eviction score:

```text
importance * recency_factor(entry.tick, current_tick)
```

For v0, use a simple monotonic recency factor that preserves the intended shape:

```text
recency_factor = 1.0 / (1.0 + age_ticks / MEMORY_RECENCY_HALF_LIFE_TICKS)
```

with a named constant such as `MEMORY_RECENCY_HALF_LIFE_TICKS = 60 * 24 * 30` (roughly one sim-month if one tick is one sim-minute). The exact curve is balancing data later; the contract for this pass is deterministic old/low-importance eviction while allowing old high-importance memories to survive.

Tie-breaks must be deterministic. If two entries have equal eviction score, evict the older tick; if still tied, evict the lower `MemoryEntryId`.

## Error handling

Memory generation should not make action execution fragile:

- Clamp invalid `importance` and `valence` values instead of rejecting the effect.
- Unsupported participant targets produce an empty participant list.
- Missing `Memory` or `Position` components should be impossible for spawned agents, but if a hand-built test entity omits them, the effect logs a warning and no-ops for memory only.
- Failed memory generation must not prevent other effects in the same advertisement from applying.

## Testing

Core tests:

- `Memory::default()` can be inserted as an ECS component.
- `push_memory` appends entries below the cap.
- `push_memory` clamps `importance` and `valence`.
- eviction removes the lowest `importance * recency_factor`, with deterministic tie-breaks.
- `TargetSpec::Self_` produces a participant list with the actor ID.
- unsupported targets produce an empty participant list.

Decision execution tests:

- a completed action with `Effect::MemoryGenerate` appends a memory.
- an in-progress action does not append a memory.
- a self-action does not append a memory.
- existing need/mood/recent-action behavior remains unchanged.

Integration tests:

- a hungry agent eating from the seed fridge ends with restored hunger and one `Routine` memory.
- determinism remains stable for two sims with the same seed and same tick sequence. If snapshots stay memory-free, expose a core-only helper such as `Sim::agent_memory(agent_id) -> Option<&[MemoryEntry]>` for tests.

Verification commands:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p gecko-sim-host --test ws_smoke
```

The websocket smoke test may need permission to bind localhost in this sandbox.

## Commit strategy

Three jj changes:

1. `Memory: ECS component + bounded append/eviction helpers`
2. `Memory: execute MemoryGenerate effects on completed actions`
3. `Memory: seed fridge action emits a routine memory`

The plan may merge 1 and 2 if the implementation stays small, but keeping seed content separate gives a clean boundary between runtime behavior and authored content.

## Trace to ADRs

- **ADR 0004:** persistent memory begins to exist as action-generated state, but recall does not yet bias utility scoring.
- **ADR 0010:** system #4 lands as a bounded episodic record. Couplings to relationships, decisions, and promoted events remain deferred.
- **ADR 0011:** `MemoryEntry`, `Effect::MemoryGenerate`, `TargetSpec`, and the 500-entry cap are implemented as described. Multi-participant memories remain structurally supported but not generated by this pass.
- **ADR 0013:** untouched; memories stay out of snapshots and the frontend for now.

## Deferred items

| Item | Trigger | Likely files |
|---|---|---|
| Memory-based decision scoring | Once memories should bias repeated/avoidant behavior | `systems/decision/scoring.rs`, `systems/decision/decide.rs` |
| Relationship-affecting memories | Relationships pass | `systems/relationships.rs`, target resolution helpers |
| Frontend memory inspection | Agent detail/inspection UI pass | protocol messages, `AgentSnapshot`, web components |
| Multi-agent target resolution | Social interactions / nearby-agent actions | target resolver module + ECS queries |
| Memory decay/balancing | Long-running sim tuning | `systems/memory.rs`, content/balancing docs |
| Save/load memory validation | Save-system pass | `crates/core/src/save` |
