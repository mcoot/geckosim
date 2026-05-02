//! Memory helpers for ADR 0010 system #4.
//!
//! This pass does not schedule a per-tick memory system. Memory mutation
//! happens when action effects complete.

use bevy_ecs::prelude::Resource;

use crate::agent::{Memory, MemoryEntry, TargetSpec};
use crate::ids::{AgentId, MemoryEntryId};

pub const MEMORY_CAP: usize = 500;
pub const MEMORY_RECENCY_HALF_LIFE_TICKS: f32 = 60.0 * 24.0 * 30.0;

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
        let idx =
            eviction_index(&memory.entries, current_tick).expect("entries is non-empty after push");
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

#[allow(
    clippy::cast_precision_loss,
    reason = "memory ages for v0 sim runs remain well below problematic f32 precision"
)]
fn eviction_score(entry: &MemoryEntry, current_tick: u64) -> f32 {
    let age_ticks = current_tick.saturating_sub(entry.tick) as f32;
    let recency = 1.0 / (1.0 + age_ticks / MEMORY_RECENCY_HALF_LIFE_TICKS);
    entry.importance.clamp(0.0, 1.0) * recency
}

#[cfg(test)]
mod tests {
    use super::{push_memory, resolve_memory_participants, MemoryIdAllocator, MEMORY_CAP};
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
        assert!(!memory
            .entries
            .iter()
            .any(|e| e.id == MemoryEntryId::new(999)));
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
        assert!(resolve_memory_participants(TargetSpec::OwnerOfObject, AgentId::new(42))
            .is_empty());
    }
}
