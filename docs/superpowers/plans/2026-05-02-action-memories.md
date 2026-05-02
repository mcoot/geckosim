# Action Memories Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Completed smart-object actions can append deterministic per-agent `MemoryEntry` records through `Effect::MemoryGenerate`.

**Architecture:** Add a lazy-sharded `Memory` ECS component and a small `systems::memory` helper module for bounded append, clamping, participant resolution, and deterministic eviction. Extend the existing `decision::execute` completion path so `MemoryGenerate` effects allocate IDs and append memories atomically alongside current need/mood effects, then add one seed fridge memory effect for a content-driven smoke path.

**Tech Stack:** Rust 2021, `bevy_ecs 0.16`, serde/RON content, Cargo tests, jj for VCS.

**Spec:** `docs/superpowers/specs/2026-05-02-action-memories-design.md`

**VCS note:** This repository uses jj/jujutsu. Do not use raw git. Each task starts in its own jj change with `jj new -m "..."`, except Task 1 may use the current empty planning child change if it is still empty.

---

## File Structure

**Create:**
- `crates/core/src/systems/memory.rs` — memory cap, ID allocator resource, participant resolution, clamping, append, eviction, unit tests.

**Modify:**
- `crates/core/src/agent/mod.rs` — add `Memory` ECS component and component unit tests.
- `crates/core/src/systems/mod.rs` — expose `pub mod memory;`.
- `crates/core/src/systems/decision/effects.rs` — add optional memory effect target and implement `Effect::MemoryGenerate`.
- `crates/core/src/systems/decision/execute.rs` — query identity/position/memory and pass memory context into effect application.
- `crates/core/src/sim.rs` — insert `MemoryIdAllocator`, spawn agents with `Memory::default()`, expose `agent_memory` test/helper API.
- `crates/core/src/lib.rs` — optionally re-export `Memory`, `MemoryEntry`, `MemoryKind` if integration tests would otherwise get noisy. Prefer using `gecko_sim_core::agent::...` first.
- `crates/core/tests/decision.rs` — assert the seed fridge action creates one routine memory.
- `crates/core/tests/determinism.rs` — add a memory determinism integration test using seed content.
- `content/object_types/fridge.ron` — add one `MemoryGenerate` effect to "Eat snack".

**Do not touch:**
- `apps/web/**`
- `crates/protocol/**`
- `apps/web/src/types/sim/**`
- snapshot wire shape

## Chunk 1: Core Action Memories

### Task 1: Memory Component

**Files:**
- Modify: `crates/core/src/agent/mod.rs`

- [ ] **Step 1.1: Start the task change**

If the current jj change is still empty and described as `WIP: plan action memories implementation`, reuse it. Otherwise run:

```bash
jj new -m "Memory: ECS component + bounded append helpers"
```

Expected: working copy is on a fresh empty change.

- [ ] **Step 1.2: Write the failing component tests**

In `crates/core/src/agent/mod.rs`, after `memory_tests` would naturally belong near `MemoryEntry`, add:

```rust
#[cfg(test)]
mod memory_component_tests {
    use super::{Memory, MemoryEntry};
    use bevy_ecs::world::World;

    #[test]
    fn memory_default_is_empty() {
        let memory = Memory::default();
        assert!(memory.entries.is_empty());
    }

    #[test]
    fn memory_can_be_inserted_as_component() {
        let mut world = World::new();
        let entity = world.spawn(Memory::default()).id();
        let memory = world.get::<Memory>(entity).expect("Memory component present");
        assert!(memory.entries.is_empty());
    }

    fn assert_memory_entry_clone<T: Clone>() {}

    #[test]
    fn memory_entry_remains_cloneable() {
        assert_memory_entry_clone::<MemoryEntry>();
    }
}
```

- [ ] **Step 1.3: Run the focused failing test**

Run:

```bash
cargo test -p gecko-sim-core memory_default_is_empty
```

Expected: compile failure mentioning missing type `Memory`.

- [ ] **Step 1.4: Add the `Memory` component**

In `crates/core/src/agent/mod.rs`, directly after `MemoryEntry`, add:

```rust
/// Bounded episodic memory ring for an agent.
///
/// The runtime cap is enforced by `systems::memory::push_memory`; the
/// component stores a plain `Vec` to match ADR 0011's v0 bounded-ring alias.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Memory {
    pub entries: Vec<MemoryEntry>,
}
```

- [ ] **Step 1.5: Run the component tests**

Run:

```bash
cargo test -p gecko-sim-core memory_component_tests
```

Expected: all tests in `memory_component_tests` pass.

### Task 2: Memory Helper Module

**Files:**
- Create: `crates/core/src/systems/memory.rs`
- Modify: `crates/core/src/systems/mod.rs`

- [ ] **Step 2.1: Write failing tests and module skeleton**

Create `crates/core/src/systems/memory.rs` with tests first:

```rust
//! Memory helpers for ADR 0010 system #4.
//!
//! This pass does not schedule a per-tick memory system. Memory mutation
//! happens when action effects complete.

use crate::agent::{Memory, MemoryEntry};
use crate::ids::{AgentId, LeafAreaId, MemoryEntryId};

pub const MEMORY_CAP: usize = 500;
pub const MEMORY_RECENCY_HALF_LIFE_TICKS: f32 = 60.0 * 24.0 * 30.0;

#[cfg(test)]
mod tests {
    use super::{
        push_memory, resolve_memory_participants, MemoryIdAllocator, MEMORY_CAP,
    };
    use crate::agent::{Memory, MemoryEntry, MemoryKind, TargetSpec};
    use crate::ids::{AgentId, LeafAreaId, MemoryEntryId};

    fn entry(id: u64, tick: u64, importance: f32) -> MemoryEntry {
        MemoryEntry {
            id: MemoryEntryId::new(id),
            kind: MemoryKind::Routine,
            tick,
            participants: vec![],
            location: LeafAreaId::new(7),
            valence: 0.0,
            importance,
        }
    }

    #[test]
    fn allocator_returns_monotonic_memory_ids() {
        let mut ids = MemoryIdAllocator::default();
        assert_eq!(ids.allocate(), MemoryEntryId::new(0));
        assert_eq!(ids.allocate(), MemoryEntryId::new(1));
    }

    #[test]
    fn push_memory_appends_below_cap() {
        let mut memory = Memory::default();
        push_memory(&mut memory, entry(0, 10, 0.5), 10);
        assert_eq!(memory.entries.len(), 1);
        assert_eq!(memory.entries[0].id, MemoryEntryId::new(0));
    }

    #[test]
    fn push_memory_clamps_importance_and_valence() {
        let mut memory = Memory::default();
        let mut e = entry(0, 10, 2.0);
        e.valence = -2.0;
        push_memory(&mut memory, e, 10);
        assert_eq!(memory.entries[0].importance, 1.0);
        assert_eq!(memory.entries[0].valence, -1.0);
    }

    #[test]
    fn push_memory_evicts_lowest_scored_entry() {
        let mut memory = Memory::default();
        for idx in 0..MEMORY_CAP {
            push_memory(&mut memory, entry(idx as u64, 100, 0.9), 100);
        }
        push_memory(&mut memory, entry(999, 100, 0.1), 100);

        assert_eq!(memory.entries.len(), MEMORY_CAP);
        assert!(!memory.entries.iter().any(|e| e.id == MemoryEntryId::new(999)));
    }

    #[test]
    fn push_memory_tie_breaks_by_older_tick_then_lower_id() {
        let mut memory = Memory::default();
        for idx in 0..MEMORY_CAP {
            push_memory(&mut memory, entry(idx as u64, 100, 0.5), 100);
        }
        push_memory(&mut memory, entry(999, 100, 0.5), 100);

        assert_eq!(memory.entries.len(), MEMORY_CAP);
        assert!(!memory.entries.iter().any(|e| e.id == MemoryEntryId::new(0)));
    }

    #[test]
    fn self_target_resolves_to_actor_participant() {
        assert_eq!(
            resolve_memory_participants(TargetSpec::Self_, AgentId::new(42)),
            vec![AgentId::new(42)]
        );
    }

    #[test]
    fn unsupported_targets_resolve_empty_at_v0() {
        assert!(resolve_memory_participants(
            TargetSpec::OwnerOfObject,
            AgentId::new(42)
        )
        .is_empty());
    }
}
```

In `crates/core/src/systems/mod.rs`, add:

```rust
pub mod memory;
```

- [ ] **Step 2.2: Run the focused failing tests**

Run:

```bash
cargo test -p gecko-sim-core systems::memory
```

Expected: compile failures for missing `push_memory`, `resolve_memory_participants`, and `MemoryIdAllocator`.

- [ ] **Step 2.3: Implement allocator, participant resolution, append, and eviction**

In `crates/core/src/systems/memory.rs`, above the test module, add:

```rust
use bevy_ecs::prelude::Resource;

use crate::agent::TargetSpec;

#[derive(Resource, Debug, Default)]
pub struct MemoryIdAllocator {
    next: u64,
}

impl MemoryIdAllocator {
    pub fn allocate(&mut self) -> MemoryEntryId {
        let id = MemoryEntryId::new(self.next);
        self.next += 1;
        id
    }
}

pub fn resolve_memory_participants(target: TargetSpec, actor: AgentId) -> Vec<AgentId> {
    match target {
        TargetSpec::Self_ => vec![actor],
        TargetSpec::OwnerOfObject
        | TargetSpec::OtherAgent { .. }
        | TargetSpec::NearbyAgent { .. } => vec![],
    }
}

pub fn push_memory(memory: &mut Memory, mut entry: MemoryEntry, current_tick: u64) {
    entry.importance = entry.importance.clamp(0.0, 1.0);
    entry.valence = entry.valence.clamp(-1.0, 1.0);
    memory.entries.push(entry);
    if memory.entries.len() > MEMORY_CAP {
        let idx = eviction_index(&memory.entries, current_tick)
            .expect("entries is non-empty after push");
        memory.entries.remove(idx);
    }
}

fn eviction_index(entries: &[MemoryEntry], current_tick: u64) -> Option<usize> {
    entries
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| compare_for_eviction(a, b, current_tick))
        .map(|(idx, _)| idx)
}

fn compare_for_eviction(
    a: &MemoryEntry,
    b: &MemoryEntry,
    current_tick: u64,
) -> std::cmp::Ordering {
    let score_order = eviction_score(a, current_tick).total_cmp(&eviction_score(b, current_tick));
    score_order
        .then_with(|| a.tick.cmp(&b.tick))
        .then_with(|| a.id.cmp(&b.id))
}

fn eviction_score(entry: &MemoryEntry, current_tick: u64) -> f32 {
    let age_ticks = current_tick.saturating_sub(entry.tick) as f32;
    let recency = 1.0 / (1.0 + age_ticks / MEMORY_RECENCY_HALF_LIFE_TICKS);
    entry.importance.clamp(0.0, 1.0) * recency
}
```

Keep imports tidy:

```rust
use bevy_ecs::prelude::Resource;

use crate::agent::{Memory, MemoryEntry, TargetSpec};
use crate::ids::{AgentId, MemoryEntryId};
```

Remove unused `LeafAreaId` from non-test imports if clippy reports it.

- [ ] **Step 2.4: Run helper tests**

Run:

```bash
cargo test -p gecko-sim-core systems::memory
```

Expected: all memory helper tests pass.

- [ ] **Step 2.5: Run clippy for the new module**

Run:

```bash
cargo clippy -p gecko-sim-core --lib -- -D warnings
```

Expected: clean. If clippy objects to `as f32`, add a narrow `#[allow(clippy::cast_precision_loss, reason = "...")]` on `eviction_score`; use wording about memory ages being well below problematic precision for v0.

- [ ] **Step 2.6: Record the task change**

Run:

```bash
jj st
jj desc -m "Memory: ECS component + bounded append helpers"
jj new -m "Memory: execute MemoryGenerate effects"
```

Expected: first memory change described; working copy advances to a fresh child change.

### Task 3: Spawn Agents With Memory And Expose Test Helper

**Files:**
- Modify: `crates/core/src/sim.rs`

- [ ] **Step 3.1: Write failing integration-facing helper test**

In `crates/core/tests/snapshot.rs`, extend `snapshot_contains_spawned_agents_sorted_by_id` after the existing action assertion:

```rust
    let alice_memory = sim.agent_memory(alice).expect("Alice has memory component");
    assert!(alice_memory.is_empty());
```

- [ ] **Step 3.2: Run the focused failing test**

Run:

```bash
cargo test -p gecko-sim-core --test snapshot snapshot_contains_spawned_agents_sorted_by_id
```

Expected: compile failure that `Sim::agent_memory` does not exist, or runtime failure if the helper exists but agents do not yet have memory.

- [ ] **Step 3.3: Wire memory into `Sim`**

In `crates/core/src/sim.rs`, update imports:

```rust
use crate::agent::{
    Accessory, AccessoryCatalog, Facing, Identity, Memory, MemoryEntry, Mood, Needs, Personality,
    Position,
};
use crate::systems::memory::MemoryIdAllocator;
```

In `Sim::new`, after inserting `CurrentTick`, insert:

```rust
        world.insert_resource(MemoryIdAllocator::default());
```

In `spawn_test_agent_with_needs`, add `Memory::default()` to the spawned component tuple:

```rust
            Memory::default(),
```

Add this public helper near `snapshot()` or near the catalog/world accessors:

```rust
    #[must_use]
    pub fn agent_memory(&self, agent_id: AgentId) -> Option<&[MemoryEntry]> {
        self.world.iter_entities().find_map(|entity_ref| {
            let identity = entity_ref.get::<Identity>()?;
            if identity.id != agent_id {
                return None;
            }
            entity_ref
                .get::<Memory>()
                .map(|memory| memory.entries.as_slice())
        })
    }
```

- [ ] **Step 3.4: Run the focused test**

Run:

```bash
cargo test -p gecko-sim-core --test snapshot snapshot_contains_spawned_agents_sorted_by_id
```

Expected: pass.

- [ ] **Step 3.5: Run core lib tests**

Run:

```bash
cargo test -p gecko-sim-core --lib
```

Expected: pass.

### Task 4: Apply MemoryGenerate During Execute

**Files:**
- Modify: `crates/core/src/systems/decision/effects.rs`
- Modify: `crates/core/src/systems/decision/execute.rs`

- [ ] **Step 4.1: Add failing `effects.rs` unit test for `MemoryGenerate`**

In `crates/core/src/systems/decision/effects.rs` tests, add imports:

```rust
use crate::agent::{Memory, MemoryKind, TargetSpec};
use crate::ids::{AgentId, LeafAreaId, MemoryEntryId};
use crate::systems::decision::effects::{MemoryEffectTarget, ...};
use crate::systems::memory::MemoryIdAllocator;
```

Add the test:

```rust
    #[test]
    fn memory_generate_appends_memory() {
        let mut needs = Needs::full();
        let mut mood = Mood::neutral();
        let mut memory = Memory::default();
        let mut ids = MemoryIdAllocator::default();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
            memory: Some(MemoryEffectTarget {
                actor: AgentId::new(7),
                location: LeafAreaId::new(3),
                memory: &mut memory,
                memory_ids: &mut ids,
                current_tick: 12,
            }),
        };

        apply(
            &Effect::MemoryGenerate {
                kind: MemoryKind::Routine,
                importance: 2.0,
                valence: -2.0,
                participants: TargetSpec::Self_,
            },
            &mut target,
        );

        assert_eq!(memory.entries.len(), 1);
        let entry = &memory.entries[0];
        assert_eq!(entry.id, MemoryEntryId::new(0));
        assert_eq!(entry.kind, MemoryKind::Routine);
        assert_eq!(entry.tick, 12);
        assert_eq!(entry.location, LeafAreaId::new(3));
        assert_eq!(entry.participants, vec![AgentId::new(7)]);
        assert_eq!(entry.importance, 1.0);
        assert_eq!(entry.valence, -1.0);
    }
```

Update existing `EffectTarget` initializers in tests to include `memory: None`.

- [ ] **Step 4.2: Run the focused failing test**

Run:

```bash
cargo test -p gecko-sim-core memory_generate_appends_memory
```

Expected: compile failure for missing `MemoryEffectTarget` / `EffectTarget.memory`.

- [ ] **Step 4.3: Implement memory effect target**

In `crates/core/src/systems/decision/effects.rs`, update imports:

```rust
use crate::agent::{Memory, MemoryEntry, Mood, MoodDim, Need, Needs};
use crate::ids::{AgentId, LeafAreaId};
use crate::systems::memory::{push_memory, resolve_memory_participants, MemoryIdAllocator};
```

Replace `EffectTarget` with:

```rust
pub struct MemoryEffectTarget<'a> {
    pub actor: AgentId,
    pub location: LeafAreaId,
    pub memory: &'a mut Memory,
    pub memory_ids: &'a mut MemoryIdAllocator,
    pub current_tick: u64,
}

pub struct EffectTarget<'a> {
    pub needs: &'a mut Needs,
    pub mood: &'a mut Mood,
    pub memory: Option<MemoryEffectTarget<'a>>,
}
```

In `apply`, replace the unsupported `Effect::MemoryGenerate { .. }` match arm with:

```rust
        Effect::MemoryGenerate {
            kind,
            importance,
            valence,
            participants,
        } => {
            if let Some(memory_target) = target.memory.as_mut() {
                let entry = MemoryEntry {
                    id: memory_target.memory_ids.allocate(),
                    kind: *kind,
                    tick: memory_target.current_tick,
                    participants: resolve_memory_participants(
                        *participants,
                        memory_target.actor,
                    ),
                    location: memory_target.location,
                    valence: *valence,
                    importance: *importance,
                };
                push_memory(memory_target.memory, entry, memory_target.current_tick);
            } else {
                tracing::warn!(
                    ?kind,
                    "decision::effects::apply: memory target missing; MemoryGenerate no-op",
                );
            }
        }
```

Keep these variants in the unsupported block:

```rust
        Effect::AgentSkillDelta(_, _)
        | Effect::MoneyDelta(_)
        | Effect::InventoryDelta(_, _)
        | Effect::RelationshipDelta(_, _, _)
        | Effect::HealthConditionChange(_)
        | Effect::PromotedEvent(_, _) => { ... }
```

- [ ] **Step 4.4: Run effects tests**

Run:

```bash
cargo test -p gecko-sim-core systems::decision::effects
```

Expected: pass.

- [ ] **Step 4.5: Add failing execute-system test**

In `crates/core/src/systems/decision/execute.rs` tests:

1. Add imports:

```rust
use crate::agent::{Identity, Memory, MemoryKind, Position, TargetSpec};
use crate::ids::AgentId;
use crate::systems::memory::MemoryIdAllocator;
```

2. Insert `MemoryIdAllocator::default()` into `build_world` resources:

```rust
        world.insert_resource(MemoryIdAllocator::default());
```

3. Add `Identity`, `Position`, and `Memory::default()` to the spawned agent tuple:

```rust
                Identity {
                    id: AgentId::new(0),
                    name: "Tester".to_string(),
                },
                Position {
                    leaf: LeafAreaId::new(0),
                    pos: Vec2::ZERO,
                },
                Memory::default(),
```

4. Add a second ad or extend `fridge_object_type()` with a `MemoryGenerate` effect. Prefer extending the existing ad:

```rust
                effects: vec![
                    Effect::AgentNeedDelta(Need::Hunger, 0.4),
                    Effect::MemoryGenerate {
                        kind: MemoryKind::Routine,
                        importance: 0.2,
                        valence: 0.35,
                        participants: TargetSpec::Self_,
                    },
                ],
```

5. Add assertion to `completed_action_applies_effects_and_clears_current_action`:

```rust
        let memory = world.get::<Memory>(agent).unwrap();
        assert_eq!(memory.entries.len(), 1);
        assert_eq!(memory.entries[0].kind, MemoryKind::Routine);
        assert_eq!(memory.entries[0].participants, vec![AgentId::new(0)]);
        assert_eq!(memory.entries[0].location, LeafAreaId::new(0));
```

6. Add to `in_progress_action_does_not_complete`:

```rust
        let memory = world.get::<Memory>(agent).unwrap();
        assert!(memory.entries.is_empty());
```

7. Add to `idle_self_action_clears_without_effects`:

```rust
        let memory = world.get::<Memory>(agent).unwrap();
        assert!(memory.entries.is_empty());
```

- [ ] **Step 4.6: Run the focused failing execute test**

Run:

```bash
cargo test -p gecko-sim-core systems::decision::execute::tests::completed_action_applies_effects_and_clears_current_action
```

Expected: compile failure because `execute` does not yet query/pass memory context.

- [ ] **Step 4.7: Wire execute to pass memory context**

In `crates/core/src/systems/decision/execute.rs`, update imports:

```rust
use bevy_ecs::system::{Query, Res, ResMut};

use crate::agent::{Identity, Memory, Mood, Needs, Position};
use crate::systems::decision::effects::{
    apply as apply_effect, EffectTarget, MemoryEffectTarget,
};
use crate::systems::memory::MemoryIdAllocator;
```

Update `execute` signature:

```rust
pub(crate) fn execute(
    catalog: Res<ObjectCatalog>,
    current_tick: Res<CurrentTick>,
    mut memory_ids: ResMut<MemoryIdAllocator>,
    objects: Query<&SmartObject>,
    mut agents: Query<(
        Option<&Identity>,
        Option<&Position>,
        &mut Needs,
        &mut Mood,
        Option<&mut Memory>,
        &mut RecentActionsRing,
        &mut CurrentAction,
    )>,
)
```

Update the loop binding:

```rust
    for (identity, position, mut needs, mut mood, mut memory, mut ring, mut current) in &mut agents {
```

When building the target before applying effects:

```rust
                    let memory_target = match (identity, position, memory.as_deref_mut()) {
                        (Some(identity), Some(position), Some(memory)) => {
                            Some(MemoryEffectTarget {
                                actor: identity.id,
                                location: position.leaf,
                                memory,
                                memory_ids: &mut memory_ids,
                                current_tick: current_tick.0,
                            })
                        }
                        _ => None,
                    };
                    let mut target = EffectTarget {
                        needs: &mut needs,
                        mood: &mut mood,
                        memory: memory_target,
                    };
```

If `memory.as_deref_mut()` is awkward with Bevy's `Mut<Memory>`, use this explicit form instead:

```rust
                    let memory_target = match (identity, position, memory.as_mut()) {
                        (Some(identity), Some(position), Some(memory)) => {
                            Some(MemoryEffectTarget {
                                actor: identity.id,
                                location: position.leaf,
                                memory: &mut *memory,
                                memory_ids: &mut memory_ids,
                                current_tick: current_tick.0,
                            })
                        }
                        _ => None,
                    };
```

- [ ] **Step 4.8: Run execute tests**

Run:

```bash
cargo test -p gecko-sim-core systems::decision::execute
```

Expected: pass.

- [ ] **Step 4.9: Run core tests**

Run:

```bash
cargo test -p gecko-sim-core
```

Expected: pass.

- [ ] **Step 4.10: Record the task change**

Run:

```bash
jj st
jj desc -m "Memory: execute MemoryGenerate effects"
jj new -m "Memory: seed fridge action emits a routine memory"
```

Expected: execute integration is recorded; working copy advances to a fresh child change.

### Task 5: Seed Content Memory And Integration Tests

**Files:**
- Modify: `content/object_types/fridge.ron`
- Modify: `crates/core/tests/decision.rs`
- Modify: `crates/core/tests/determinism.rs`

- [ ] **Step 5.1: Add failing integration assertion before changing content**

In `crates/core/tests/decision.rs`, change:

```rust
    sim.spawn_test_agent_with_needs(
```

to capture the ID:

```rust
    let agent_id = sim.spawn_test_agent_with_needs(
```

After the hunger assertion, add:

```rust
    let memories = sim.agent_memory(agent_id).expect("agent has memory");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].kind, gecko_sim_core::agent::MemoryKind::Routine);
    assert_eq!(memories[0].participants, vec![agent_id]);
```

- [ ] **Step 5.2: Run the focused failing integration test**

Run:

```bash
cargo test -p gecko-sim-core --test decision agent_eats_from_fridge_when_hungry
```

Expected: test fails because the seed fridge content does not yet emit memory.

- [ ] **Step 5.3: Update seed fridge content**

In `content/object_types/fridge.ron`, update the `effects` list:

```ron
            effects: [
                AgentNeedDelta(Hunger, 0.4),
                MemoryGenerate(
                    kind: Routine,
                    importance: 0.2,
                    valence: 0.35,
                    participants: Self_,
                ),
            ],
```

- [ ] **Step 5.4: Run seed content load test**

Run:

```bash
cargo test -p gecko-sim-content --test seed_loads
```

Expected: pass. If RON syntax fails, adjust only syntax, not Rust shapes. The expected struct-variant syntax is `MemoryGenerate(kind: Routine, importance: 0.2, valence: 0.35, participants: Self_)`.

- [ ] **Step 5.5: Run the action-memory integration test**

Run:

```bash
cargo test -p gecko-sim-core --test decision agent_eats_from_fridge_when_hungry
```

Expected: pass.

- [ ] **Step 5.6: Add memory determinism test**

In `crates/core/tests/determinism.rs`, add `mod common;` at the top and import `MemoryEntry`:

```rust
mod common;

use gecko_sim_core::agent::{MemoryEntry, Needs};
use gecko_sim_core::{ContentBundle, Sim, Snapshot, Vec2};
```

Keep the existing `run()` snapshot helper. Add:

```rust
fn run_hungry_agent_memories(seed: u64, ticks: u64) -> Vec<MemoryEntry> {
    let mut sim = Sim::new(seed, common::seed_content_bundle());
    let agent = sim.spawn_test_agent_with_needs(
        "Hungry",
        Needs {
            hunger: 0.3,
            ..Needs::full()
        },
    );
    let leaf = sim.world_graph().default_spawn_leaf;
    sim.spawn_one_of_each_object_type(leaf, Vec2::ZERO);
    for _ in 0..ticks {
        sim.tick();
    }
    sim.agent_memory(agent)
        .expect("agent has memory")
        .to_vec()
}

#[test]
fn same_seed_same_action_memories_after_fridge_action() {
    assert_eq!(
        run_hungry_agent_memories(42, 15),
        run_hungry_agent_memories(42, 15)
    );
}
```

- [ ] **Step 5.7: Run determinism tests**

Run:

```bash
cargo test -p gecko-sim-core --test determinism
```

Expected: pass.

- [ ] **Step 5.8: Run content validation tests**

Run:

```bash
cargo test -p gecko-sim-content
```

Expected: pass.

- [ ] **Step 5.9: Record the task change**

Run:

```bash
jj st
jj desc -m "Memory: seed fridge action emits a routine memory"
jj new -m "Memory: final verification"
```

Expected: seed content + integration tests are recorded; working copy advances to a fresh child change for any verification-only fixes.

### Task 6: Final Verification

**Files:**
- No planned edits. Only make fixes if verification reveals an issue.

- [ ] **Step 6.1: Run workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: all pass, except the sandbox may deny the websocket smoke test's localhost bind.

- [ ] **Step 6.2: If websocket smoke fails from sandbox bind, rerun just that test with permission**

Run:

```bash
cargo test -p gecko-sim-host --test ws_smoke
```

Expected: pass when localhost binding is allowed.

- [ ] **Step 6.3: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: pass.

- [ ] **Step 6.4: Check jj status and log**

Run:

```bash
jj st
jj log -r 'ancestors(@, 8)' --no-graph
```

Expected: no unintended files. The recent chain should show:

- `Action memories: design`
- `Action memories: implementation plan`
- memory implementation changes described above

- [ ] **Step 6.5: If the final verification change is empty, abandon or describe appropriately**

If no files changed in the final verification change, it can remain empty as the active workspace change or be abandoned by the integrator. Do not use destructive commands. If fixes were made, describe it:

```bash
jj desc -m "Memory: final verification fixes"
```

Expected: all implementation changes are described and ready for the next integration step.
