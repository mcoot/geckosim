//! Effect application per ADR 0011's "Effect application" section.
//!
//! v0: only `AgentNeedDelta` and `AgentMoodDelta` are wired. Other variants
//! log a `tracing::warn!` no-op so unsupported content can flow through
//! the loader without crashing.

use crate::agent::{Memory, MemoryEntry, Mood, MoodDim, Need, Needs};
use crate::ids::{AgentId, LeafAreaId};
use crate::object::Effect;
use crate::systems::memory::{push_memory, resolve_memory_participants, MemoryIdAllocator};

/// Mutable references to the agent's effect-targeted components. v0 covers
/// `Needs` and `Mood`; other components (`Skills`, `Money`, `Inventory`,
/// `Memory`, …) join when their systems land.
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

/// Apply one effect to the agent's target components. Unsupported variants
/// log a `tracing::warn!` and return without modifying state, so the loader
/// can ship content with future-system effects without crashing the sim.
pub fn apply(effect: &Effect, target: &mut EffectTarget<'_>) {
    match effect {
        Effect::AgentNeedDelta(need, delta) => {
            let value = need_field_mut(target.needs, *need);
            *value = (*value + delta).clamp(0.0, 1.0);
        }
        Effect::AgentMoodDelta(dim, delta) => {
            let (value, lo, hi) = mood_field_mut(target.mood, *dim);
            *value = (*value + delta).clamp(lo, hi);
        }
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
                    participants: resolve_memory_participants(*participants, memory_target.actor),
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
        // v0: not yet implemented.
        Effect::AgentSkillDelta(_, _)
        | Effect::MoneyDelta(_)
        | Effect::InventoryDelta(_, _)
        | Effect::RelationshipDelta(_, _, _)
        | Effect::HealthConditionChange(_)
        | Effect::PromotedEvent(_, _) => {
            tracing::warn!(
                ?effect,
                "decision::effects::apply: effect kind not yet implemented; no-op",
            );
        }
    }
}

fn need_field_mut(needs: &mut Needs, kind: Need) -> &mut f32 {
    match kind {
        Need::Hunger => &mut needs.hunger,
        Need::Sleep => &mut needs.sleep,
        Need::Social => &mut needs.social,
        Need::Hygiene => &mut needs.hygiene,
        Need::Fun => &mut needs.fun,
        Need::Comfort => &mut needs.comfort,
    }
}

/// Returns `(field, lower_bound, upper_bound)` for the mood dimension.
fn mood_field_mut(mood: &mut Mood, dim: MoodDim) -> (&mut f32, f32, f32) {
    match dim {
        MoodDim::Valence => (&mut mood.valence, -1.0, 1.0),
        MoodDim::Arousal => (&mut mood.arousal, 0.0, 1.0),
        MoodDim::Stress => (&mut mood.stress, 0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::{Memory, MemoryKind, Mood, MoodDim, Need, Needs, TargetSpec};
    use crate::ids::{AgentId, LeafAreaId, MemoryEntryId};
    use crate::object::Effect;
    use crate::systems::decision::effects::{apply, EffectTarget, MemoryEffectTarget};
    use crate::systems::memory::MemoryIdAllocator;

    #[test]
    fn agent_need_delta_applies() {
        let mut needs = Needs {
            hunger: 0.3,
            ..Needs::full()
        };
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
            memory: None,
        };
        apply(&Effect::AgentNeedDelta(Need::Hunger, 0.4), &mut target);
        assert!((needs.hunger - 0.7).abs() < 1e-6, "hunger={}", needs.hunger);
    }

    #[test]
    fn agent_need_delta_clamps_at_one() {
        let mut needs = Needs {
            hunger: 0.9,
            ..Needs::full()
        };
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
            memory: None,
        };
        apply(&Effect::AgentNeedDelta(Need::Hunger, 0.5), &mut target);
        assert!((needs.hunger - 1.0).abs() < 1e-6);
    }

    #[test]
    fn agent_need_delta_clamps_at_zero() {
        let mut needs = Needs {
            hunger: 0.1,
            ..Needs::full()
        };
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
            memory: None,
        };
        apply(&Effect::AgentNeedDelta(Need::Hunger, -0.5), &mut target);
        assert!(needs.hunger.abs() < 1e-6);
    }

    #[test]
    fn agent_mood_delta_applies_to_valence() {
        let mut needs = Needs::full();
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
            memory: None,
        };
        apply(&Effect::AgentMoodDelta(MoodDim::Valence, 0.5), &mut target);
        assert!((mood.valence - 0.5).abs() < 1e-6);
    }

    #[test]
    fn unsupported_effect_does_not_panic() {
        let mut needs = Needs::full();
        let mut mood = Mood::neutral();
        let mut target = EffectTarget {
            needs: &mut needs,
            mood: &mut mood,
            memory: None,
        };
        // MoneyDelta is not yet implemented; should warn and no-op.
        apply(&Effect::MoneyDelta(100), &mut target);
        // No assertion — just confirm we didn't panic.
    }

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
}
