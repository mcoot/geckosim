//! Live `Sim` driver: wraps a `bevy_ecs::World` and advances tick state.
//!
//! Honours the public API contract from ADR 0012 partially:
//!   - `new`, `tick`, `current_tick`, `snapshot` (snapshot lands in Task 4).
//!   - `delta_since`, `apply_input` deferred to the WS pass.

use bevy_ecs::world::World;

use crate::rng::PrngState;

/// Catalog data passed into `Sim::new`. Empty placeholder until RON
/// content loading lands; lives in `core` because ADR 0012 fixes the
/// dep direction at `core ← content`.
#[derive(Debug, Clone, Default)]
pub struct ContentBundle;

/// Per-tick stats returned from `Sim::tick`. Empty placeholder; future
/// per-tick counters (decisions made, interrupts raised, promoted events
/// emitted, …) live here.
#[derive(Debug, Clone, Default)]
pub struct TickReport;

/// The live simulation. Owns its `bevy_ecs::World` and the canonical clock.
pub struct Sim {
    // World is wired in Task 4 (snapshot) / Task 5 (needs-decay system).
    #[expect(dead_code, reason = "wired in Task 4 (Sim::snapshot, spawn_test_agent)")]
    world: World,
    tick: u64,
    // Stored for determinism; needs-decay does not consume randomness, but
    // future systems will. The seed is captured here at construction time.
    #[expect(dead_code, reason = "consumed by first RNG-using system in a later pass")]
    rng: PrngState,
    // Used by `spawn_test_agent` in Task 4.
    #[expect(dead_code, reason = "used by spawn_test_agent in Task 4")]
    next_agent_id: u64,
}

impl Sim {
    /// Construct a fresh sim with the given world seed and (currently empty)
    /// content bundle.
    #[must_use]
    pub fn new(seed: u64, _content: ContentBundle) -> Self {
        Self {
            world: World::new(),
            tick: 0,
            rng: PrngState::from_seed(seed),
            next_agent_id: 0,
        }
    }

    /// Advance the simulation by one tick (one sim-minute per ADR 0008).
    pub fn tick(&mut self) -> TickReport {
        // Systems land here. Task 5 wires `systems::needs::decay`.
        self.tick += 1;
        TickReport
    }

    /// Current tick count. Starts at 0; each `tick()` increments by 1.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick
    }
}
