//! ECS system: mood update. System #3 of 11 from ADR 0010.
//!
//! Each tick (one sim-minute per ADR 0008), every agent's mood drifts
//! toward a target derived from the agent's current `Needs` with a
//! small inertia. Pure deterministic function on `Needs` (no RNG, no
//! events). See ADR 0011 for the `Mood` schema and ADR 0010 for the
//! cross-system coupling intent.

use bevy_ecs::system::Query;

use crate::agent::{Mood, Needs};

/// Mood drifts toward its needs-derived target by this fraction each
/// tick. `α = 0.01` means mood reaches ~63% of target in 100 ticks
/// (≈ 1.67 sim-hours), saturates within ~500 ticks. Tunable.
pub const MOOD_DRIFT_RATE_PER_TICK: f32 = 0.01;

/// Stress target activates when the worst need drops below this floor.
/// Below the floor, `stress_target` rises linearly to 1.0 at need=0.
const STRESS_NEED_FLOOR: f32 = 0.5;

/// Apply one tick of mood drift to every agent. Reads the current
/// `Needs` value, computes a target mood vector, and shifts the
/// agent's `Mood` toward the target by `MOOD_DRIFT_RATE_PER_TICK`.
/// Clamps each component to its declared range.
pub(crate) fn update(mut q: Query<(&Needs, &mut Mood)>) {
    for (needs, mut mood) in &mut q {
        let mean_need = mean(needs);
        let min_need = min(needs);

        let valence_target = 2.0 * mean_need - 1.0;
        let arousal_target = (1.0 - mean_need).clamp(0.0, 1.0);
        let stress_target = ((STRESS_NEED_FLOOR - min_need) * 2.0).clamp(0.0, 1.0);

        mood.valence = (mood.valence
            + (valence_target - mood.valence) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(-1.0, 1.0);
        mood.arousal = (mood.arousal
            + (arousal_target - mood.arousal) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(0.0, 1.0);
        mood.stress = (mood.stress
            + (stress_target - mood.stress) * MOOD_DRIFT_RATE_PER_TICK)
            .clamp(0.0, 1.0);
    }
}

fn mean(n: &Needs) -> f32 {
    (n.hunger + n.sleep + n.social + n.hygiene + n.fun + n.comfort) / 6.0
}

fn min(n: &Needs) -> f32 {
    n.hunger
        .min(n.sleep)
        .min(n.social)
        .min(n.hygiene)
        .min(n.fun)
        .min(n.comfort)
}

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::agent::{Mood, Needs};
    use crate::systems::mood::{update, MOOD_DRIFT_RATE_PER_TICK};

    /// Build a single-entity world with the given (Needs, Mood) and a
    /// schedule whose only system is `mood::update`. Run one tick and
    /// return the resulting Mood.
    fn run_one_tick(needs: Needs, mood: Mood) -> Mood {
        let mut world = World::new();
        let entity = world.spawn((needs, mood)).id();
        let mut schedule = Schedule::default();
        schedule.add_systems(update);
        schedule.run(&mut world);
        *world.get::<Mood>(entity).expect("Mood component present")
    }

    #[test]
    fn full_needs_drifts_valence_positive_from_neutral() {
        // mean_need = 1.0 → valence_target = 1.0
        // After 1 tick from valence=0: valence ≈ MOOD_DRIFT_RATE_PER_TICK
        let mood = run_one_tick(Needs::full(), Mood::neutral());
        assert!(
            (mood.valence - MOOD_DRIFT_RATE_PER_TICK).abs() < 1e-6,
            "valence={}",
            mood.valence
        );
        // arousal_target = 0; mood was already 0 → still 0.
        assert!(mood.arousal.abs() < 1e-6, "arousal={}", mood.arousal);
        // stress_target = 0; still 0.
        assert!(mood.stress.abs() < 1e-6, "stress={}", mood.stress);
    }

    #[test]
    fn empty_needs_drifts_valence_negative_arousal_and_stress_up() {
        // mean_need = 0.0 → valence_target = -1.0
        // arousal_target = 1.0
        // min_need = 0.0 → stress_target = 1.0
        let needs = Needs {
            hunger: 0.0,
            sleep: 0.0,
            social: 0.0,
            hygiene: 0.0,
            fun: 0.0,
            comfort: 0.0,
        };
        let mood = run_one_tick(needs, Mood::neutral());
        assert!(
            (mood.valence + MOOD_DRIFT_RATE_PER_TICK).abs() < 1e-6,
            "valence={}",
            mood.valence
        );
        assert!(
            (mood.arousal - MOOD_DRIFT_RATE_PER_TICK).abs() < 1e-6,
            "arousal={}",
            mood.arousal
        );
        assert!(
            (mood.stress - MOOD_DRIFT_RATE_PER_TICK).abs() < 1e-6,
            "stress={}",
            mood.stress
        );
    }

    #[test]
    fn worst_need_above_threshold_yields_zero_stress_target() {
        // min_need = 0.6 (above 0.5) → stress_target = 0
        // mood.stress was 0 → still 0 after one tick.
        let needs = Needs {
            hunger: 0.6,
            sleep: 0.7,
            social: 0.8,
            hygiene: 0.9,
            fun: 0.6,
            comfort: 0.7,
        };
        let mood = run_one_tick(needs, Mood::neutral());
        assert!(mood.stress.abs() < 1e-6, "stress={}", mood.stress);
    }

    #[test]
    fn worst_need_below_threshold_drives_stress_up() {
        // min_need = 0.2 → stress_target = ((0.5 - 0.2) * 2).clamp = 0.6
        // After one tick: stress ≈ 0.6 * α
        let needs = Needs {
            hunger: 0.2,
            sleep: 0.7,
            social: 0.8,
            hygiene: 0.9,
            fun: 0.6,
            comfort: 0.7,
        };
        let mood = run_one_tick(needs, Mood::neutral());
        let expected = 0.6 * MOOD_DRIFT_RATE_PER_TICK;
        assert!(
            (mood.stress - expected).abs() < 1e-6,
            "stress={} expected={}",
            mood.stress,
            expected
        );
    }

    #[test]
    fn mood_saturates_toward_target_after_many_ticks() {
        // Empty needs → targets (-1, 1, 1). After 1000 ticks at α=0.01,
        // mood reaches > 99% of target (1 - (1-α)^1000 ≈ 0.99996).
        let mut world = World::new();
        let entity = world
            .spawn((
                Needs {
                    hunger: 0.0,
                    sleep: 0.0,
                    social: 0.0,
                    hygiene: 0.0,
                    fun: 0.0,
                    comfort: 0.0,
                },
                Mood::neutral(),
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(update);
        for _ in 0..1000 {
            schedule.run(&mut world);
        }
        let mood = *world.get::<Mood>(entity).expect("Mood present");
        assert!(mood.valence < -0.99, "valence={}", mood.valence);
        assert!(mood.arousal > 0.99, "arousal={}", mood.arousal);
        assert!(mood.stress > 0.99, "stress={}", mood.stress);
    }
}
