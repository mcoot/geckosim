//! Integration test: needs decay over many ticks saturates at zero
//! and decreases other needs by the expected amount.

use gecko_sim_core::systems::needs::{
    COMFORT_DECAY_PER_TICK, FUN_DECAY_PER_TICK, HUNGER_DECAY_PER_TICK, HYGIENE_DECAY_PER_TICK,
    SLEEP_DECAY_PER_TICK, SOCIAL_DECAY_PER_TICK,
};
use gecko_sim_core::{ContentBundle, Sim};

const HUNGER_TICKS: u64 = 480;
const TOL: f32 = 1e-5;

#[test]
#[expect(
    clippy::default_constructed_unit_structs,
    reason = "ContentBundle is a unit struct today but will gain fields when RON content loading lands; using ::default() preserves the call site"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "HUNGER_TICKS is a small u64 (480); the cast to f32 is exact"
)]
fn hunger_saturates_at_zero_after_full_decay_window() {
    let mut sim = Sim::new(0, ContentBundle::default());
    sim.spawn_test_agent("Alice");

    for _ in 0..HUNGER_TICKS {
        sim.tick();
    }

    let snap = sim.snapshot();
    let needs = snap.agents[0].needs;

    // Sanity-check the constant matches the window: HUNGER_TICKS draws the
    // need from 1.0 to (just past) 0.0.
    assert!(HUNGER_TICKS as f32 * HUNGER_DECAY_PER_TICK >= 1.0);

    // Hunger fully drained; saturating subtraction floors at 0.0.
    assert!(needs.hunger.abs() < TOL, "hunger = {}", needs.hunger);

    // Other needs decreased by exactly N * rate, saturating at zero
    // (comfort drains in 360 ticks, so it floors before HUNGER_TICKS).
    let expected = |rate: f32| (1.0 - HUNGER_TICKS as f32 * rate).max(0.0);
    assert!((needs.sleep   - expected(SLEEP_DECAY_PER_TICK  )).abs() < TOL);
    assert!((needs.social  - expected(SOCIAL_DECAY_PER_TICK )).abs() < TOL);
    assert!((needs.hygiene - expected(HYGIENE_DECAY_PER_TICK)).abs() < TOL);
    assert!((needs.fun     - expected(FUN_DECAY_PER_TICK    )).abs() < TOL);
    assert!((needs.comfort - expected(COMFORT_DECAY_PER_TICK)).abs() < TOL);
}
