//! Pure scoring helpers per ADR 0011's "Action evaluation contract".
//!
//! Score formula:
//!     base * personality * mood * (1 - recency) + noise
//!
//! All factors stay non-negative; modifier clamps land at `0.1`.

use rand::Rng;

use crate::agent::{Mood, MoodDim, Need, Needs, Personality};
use crate::decision::RecentActionsRing;
use crate::ids::{AdvertisementId, ObjectTypeId};
use crate::object::{ScoreTemplate, SituationalModifier};

/// Multiplier applied to the score of an advertisement that the agent
/// recently performed (per ADR 0011). Halves the score.
pub const RECENCY_PENALTY: f32 = 0.5;

/// Sensitivity coefficient for the personality-dot-product modifier
/// (per ADR 0011). Tunable.
pub const PERSONALITY_SENSITIVITY: f32 = 0.5;

/// Floor for personality and mood modifiers; clamps the raw modifier so
/// the score stays strictly positive.
pub const MODIFIER_FLOOR: f32 = 0.1;

/// Per ADR 0011: `base_utility = Σ over (need, factor) in need_weights:
/// factor * (1.0 - need_value(needs, need))`. Pressure rises as the need
/// drops; weights are content-defined (non-negative in seed catalog).
#[must_use]
pub fn base_utility(needs: &Needs, template: &ScoreTemplate) -> f32 {
    template
        .need_weights
        .iter()
        .map(|(need, factor)| factor * (1.0 - need_value(needs, *need)))
        .sum()
}

/// Per ADR 0011: `1.0 + sensitivity * dot(personality, weights)`,
/// clamped to `[MODIFIER_FLOOR, +∞]`.
#[must_use]
pub fn personality_modifier(personality: &Personality, weights: &Personality) -> f32 {
    let dot = personality.openness * weights.openness
        + personality.conscientiousness * weights.conscientiousness
        + personality.extraversion * weights.extraversion
        + personality.agreeableness * weights.agreeableness
        + personality.neuroticism * weights.neuroticism;
    (1.0 + PERSONALITY_SENSITIVITY * dot).max(MODIFIER_FLOOR)
}

/// Multiplies `(1 + weight * mood_value)` over each `MoodWeight` modifier;
/// returns 1.0 when the ad has no `MoodWeight` entries. Clamps each factor
/// at `MODIFIER_FLOOR`. Other `SituationalModifier` variants are no-ops at
/// v0 (they contribute `1.0`).
#[must_use]
pub fn mood_modifier(mood: &Mood, modifiers: &[SituationalModifier]) -> f32 {
    let mut product = 1.0;
    for modifier in modifiers {
        if let SituationalModifier::MoodWeight { dim, weight } = modifier {
            let factor = (1.0 + weight * mood_value(mood, *dim)).max(MODIFIER_FLOOR);
            product *= factor;
        }
    }
    product
}

/// `RECENCY_PENALTY` if the ad was performed recently (matched by
/// `(ObjectTypeId, AdvertisementId)` template), else 0.
#[must_use]
pub fn recency_penalty(
    ring: &RecentActionsRing,
    type_id: ObjectTypeId,
    ad_id: AdvertisementId,
) -> f32 {
    if ring.contains(type_id, ad_id) {
        RECENCY_PENALTY
    } else {
        0.0
    }
}

/// Pick one candidate at random, weighted by score. If all weights are
/// zero, falls back to uniform random over the candidate list. Returns
/// `None` if the list is empty.
#[must_use]
pub fn weighted_pick<R: Rng + ?Sized>(
    candidates: &[(AdvertisementId, f32)],
    rng: &mut R,
) -> Option<AdvertisementId> {
    if candidates.is_empty() {
        return None;
    }
    let total: f32 = candidates.iter().map(|(_, score)| *score).sum();
    if total <= 0.0 {
        // Uniform fallback.
        let idx = rng.random_range(0..candidates.len());
        return Some(candidates[idx].0);
    }
    let mut roll: f32 = rng.random::<f32>() * total;
    for (id, score) in candidates {
        if roll < *score {
            return Some(*id);
        }
        roll -= *score;
    }
    // Numerical edge case (roll just under total): return the last candidate.
    candidates.last().map(|(id, _)| *id)
}

fn need_value(needs: &Needs, need: Need) -> f32 {
    match need {
        Need::Hunger => needs.hunger,
        Need::Sleep => needs.sleep,
        Need::Social => needs.social,
        Need::Hygiene => needs.hygiene,
        Need::Fun => needs.fun,
        Need::Comfort => needs.comfort,
    }
}

fn mood_value(mood: &Mood, dim: MoodDim) -> f32 {
    match dim {
        MoodDim::Valence => mood.valence,
        MoodDim::Arousal => mood.arousal,
        MoodDim::Stress => mood.stress,
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::{Mood, MoodDim, Need, Needs, Personality};
    use crate::decision::{RecentActionEntry, RecentActionsRing};
    use crate::ids::{AdvertisementId, ObjectTypeId};
    use crate::object::{ScoreTemplate, SituationalModifier};
    use crate::systems::decision::scoring::{
        base_utility, mood_modifier, personality_modifier, recency_penalty, weighted_pick,
    };

    fn empty_score_template() -> ScoreTemplate {
        ScoreTemplate {
            need_weights: vec![],
            personality_weights: Personality::default(),
            situational_modifiers: vec![],
        }
    }

    #[test]
    fn base_utility_zero_when_need_full() {
        let needs = Needs::full();
        let template = ScoreTemplate {
            need_weights: vec![(Need::Hunger, 1.0)],
            ..empty_score_template()
        };
        assert!((base_utility(&needs, &template)).abs() < 1e-6);
    }

    #[test]
    fn base_utility_max_when_need_empty() {
        let needs = Needs {
            hunger: 0.0,
            sleep: 1.0,
            social: 1.0,
            hygiene: 1.0,
            fun: 1.0,
            comfort: 1.0,
        };
        let template = ScoreTemplate {
            need_weights: vec![(Need::Hunger, 1.0), (Need::Sleep, 0.5)],
            ..empty_score_template()
        };
        // hunger pressure = 1.0 * 1.0 = 1.0; sleep pressure = 0.0 * 0.5 = 0.0
        assert!((base_utility(&needs, &template) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn personality_modifier_one_at_zero_personality() {
        let p = Personality::default();
        let weights = Personality {
            openness: 0.5,
            ..Personality::default()
        };
        // dot = 0; modifier = 1 + 0.5*0 = 1.0
        assert!((personality_modifier(&p, &weights) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn personality_modifier_clamped_at_floor() {
        // Construct a strongly opposing personality + weight pair so the
        // raw modifier would go below 0.1.
        let p = Personality {
            openness: 1.0,
            conscientiousness: 1.0,
            extraversion: 1.0,
            agreeableness: 1.0,
            neuroticism: 1.0,
        };
        let weights = Personality {
            openness: -1.0,
            conscientiousness: -1.0,
            extraversion: -1.0,
            agreeableness: -1.0,
            neuroticism: -1.0,
        };
        // dot = -5; raw = 1 + 0.5*(-5) = -1.5; clamped to 0.1
        assert!((personality_modifier(&p, &weights) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn mood_modifier_compounds_multiplicatively() {
        let mood = Mood {
            valence: 0.5,
            arousal: 0.5,
            stress: 0.0,
        };
        let modifiers = vec![
            SituationalModifier::MoodWeight {
                dim: MoodDim::Valence,
                weight: 1.0,
            },
            SituationalModifier::MoodWeight {
                dim: MoodDim::Arousal,
                weight: 1.0,
            },
        ];
        // (1 + 1*0.5) * (1 + 1*0.5) = 1.5 * 1.5 = 2.25
        assert!((mood_modifier(&mood, &modifiers) - 2.25).abs() < 1e-6);
    }

    #[test]
    fn mood_modifier_default_one_when_no_mood_weights() {
        let mood = Mood {
            valence: 0.5,
            arousal: 0.5,
            stress: 0.0,
        };
        let modifiers = vec![];
        assert!((mood_modifier(&mood, &modifiers) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn recency_penalty_zero_when_not_in_ring() {
        let ring = RecentActionsRing::default();
        assert!(
            (recency_penalty(&ring, ObjectTypeId::new(1), AdvertisementId::new(1))).abs() < 1e-6
        );
    }

    #[test]
    fn recency_penalty_half_when_in_ring() {
        let mut ring = RecentActionsRing::default();
        ring.push(RecentActionEntry {
            ad_template: (ObjectTypeId::new(1), AdvertisementId::new(1)),
            completed_tick: 100,
        });
        assert!(
            (recency_penalty(&ring, ObjectTypeId::new(1), AdvertisementId::new(1)) - 0.5).abs()
                < 1e-6
        );
    }

    #[test]
    fn weighted_pick_returns_only_candidate() {
        let mut rng = rand_pcg::Pcg32::new(0, 0);
        let candidates = vec![(AdvertisementId::new(1), 1.0)];
        let picked = weighted_pick(&candidates, &mut rng);
        assert_eq!(picked, Some(AdvertisementId::new(1)));
    }

    #[test]
    fn weighted_pick_none_on_empty() {
        let mut rng = rand_pcg::Pcg32::new(0, 0);
        let candidates: Vec<(AdvertisementId, f32)> = vec![];
        assert_eq!(weighted_pick(&candidates, &mut rng), None);
    }

    #[test]
    fn weighted_pick_falls_back_to_uniform_on_zero_total() {
        let mut rng = rand_pcg::Pcg32::new(0, 0);
        let candidates = vec![
            (AdvertisementId::new(1), 0.0),
            (AdvertisementId::new(2), 0.0),
        ];
        // Pick something — both candidates equally likely. Just confirm
        // we get one of them, not None.
        let picked = weighted_pick(&candidates, &mut rng);
        assert!(picked.is_some());
        let id = picked.unwrap();
        assert!(id == AdvertisementId::new(1) || id == AdvertisementId::new(2));
    }

    #[test]
    fn weighted_pick_picks_proportional_to_weight() {
        // With score 100x larger, the high-weight candidate should win
        // overwhelmingly. Run 100 picks and assert the high-weight wins
        // > 90 times.
        let mut rng = rand_pcg::Pcg32::new(42, 0);
        let candidates = vec![
            (AdvertisementId::new(1), 100.0),
            (AdvertisementId::new(2), 1.0),
        ];
        let mut wins_high = 0;
        for _ in 0..100 {
            if weighted_pick(&candidates, &mut rng) == Some(AdvertisementId::new(1)) {
                wins_high += 1;
            }
        }
        assert!(wins_high > 90, "wins_high={wins_high}");
    }
}
