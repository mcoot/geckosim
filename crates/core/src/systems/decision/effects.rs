//! Effect application per ADR 0011's "Effect application" section.
//!
//! v0: only `AgentNeedDelta` and `AgentMoodDelta` are wired. Other variants
//! log a `tracing::warn!` no-op so unsupported content can flow through
//! the loader without crashing.

use crate::agent::{Mood, MoodDim, Need, Needs};
use crate::object::Effect;

/// Mutable references to the agent's effect-targeted components. v0 covers
/// `Needs` and `Mood`; other components (`Skills`, `Money`, `Inventory`,
/// `Memory`, …) join when their systems land.
pub struct EffectTarget<'a> {
    pub needs: &'a mut Needs,
    pub mood: &'a mut Mood,
}

/// Apply one effect to the agent's target components. Unsupported variants
/// log a `tracing::warn!` and return without modifying state, so the loader
/// can ship content with future-system effects without crashing the sim.
#[allow(dead_code, reason = "called by decide/execute systems in Tasks 4-5")]
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
        // v0: not yet implemented.
        Effect::AgentSkillDelta(_, _)
        | Effect::MoneyDelta(_)
        | Effect::InventoryDelta(_, _)
        | Effect::MemoryGenerate { .. }
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
    use crate::agent::{Mood, MoodDim, Need, Needs};
    use crate::object::Effect;
    use crate::systems::decision::effects::{apply, EffectTarget};

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
        };
        // MoneyDelta is not yet implemented; should warn and no-op.
        apply(&Effect::MoneyDelta(100), &mut target);
        // No assertion — just confirm we didn't panic.
    }
}
