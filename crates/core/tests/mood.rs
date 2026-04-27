//! Integration test: mood drifts toward needs-derived target through
//! `Sim::tick`. Confirms `mood::update` is registered in the schedule
//! and that the wire-shape change in `AgentSnapshot` (Task 5) doesn't
//! break the path. Note: this test runs even before Task 5, since it
//! reads `mood` via the entity-component path through `Sim::snapshot`.

use gecko_sim_core::agent::Needs;
use gecko_sim_core::{ContentBundle, Sim};

#[test]
fn empty_needs_drives_mood_toward_target_through_sim_tick() {
    let mut sim = Sim::new(0, ContentBundle::default());
    sim.spawn_test_agent_with_needs(
        "Hungry",
        Needs {
            hunger: 0.0,
            sleep: 0.0,
            social: 0.0,
            hygiene: 0.0,
            fun: 0.0,
            comfort: 0.0,
        },
    );

    for _ in 0..500 {
        sim.tick();
    }

    let snap = sim.snapshot();
    let agent = &snap.agents[0];

    // After 500 ticks at α=0.01 against targets (-1, 1, 1) from neutral,
    // mood reaches roughly (1 - 0.99^500) ≈ 99.3% of target.
    assert!(agent.mood.valence < -0.5, "valence={}", agent.mood.valence);
    assert!(agent.mood.arousal > 0.5, "arousal={}", agent.mood.arousal);
    assert!(agent.mood.stress > 0.5, "stress={}", agent.mood.stress);
}
