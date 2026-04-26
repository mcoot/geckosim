//! Live `Sim` driver: wraps a `bevy_ecs::World` and advances tick state.
//!
//! Honours the public API contract from ADR 0012 partially:
//!   - `new`, `tick`, `current_tick`, `snapshot` (snapshot lands in Task 4).
//!   - `delta_since`, `apply_input` deferred to the WS pass.

use bevy_ecs::world::World;

use crate::agent::{Identity, Needs};
use crate::ids::AgentId;
use crate::rng::PrngState;
use crate::snapshot::{AgentSnapshot, Snapshot};

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
    world: World,
    tick: u64,
    // Stored for determinism; needs-decay does not consume randomness, but
    // future systems will. The seed is captured here at construction time.
    #[expect(dead_code, reason = "consumed by first RNG-using system in a later pass")]
    rng: PrngState,
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
        crate::systems::needs::decay(&mut self.world);
        self.tick += 1;
        TickReport
    }

    /// Current tick count. Starts at 0; each `tick()` increments by 1.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    /// Spawn a fresh agent at full needs with a monotonically allocated
    /// `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced when RON content loading lands.
    pub fn spawn_test_agent(&mut self, name: &str) -> AgentId {
        let id = AgentId::new(self.next_agent_id);
        self.next_agent_id += 1;
        self.world.spawn((
            Identity {
                id,
                name: name.to_string(),
            },
            Needs::full(),
        ));
        id
    }

    /// Capture the full sim state at the current tick. Agents are sorted
    /// by `AgentId` ascending for determinism.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let mut agents: Vec<AgentSnapshot> = self
            .world
            .iter_entities()
            .filter_map(|entity_ref| {
                let identity = entity_ref.get::<Identity>()?;
                let needs = entity_ref.get::<Needs>()?;
                Some(AgentSnapshot {
                    id: identity.id,
                    name: identity.name.clone(),
                    needs: *needs,
                })
            })
            .collect();
        agents.sort_by_key(|a| a.id);
        Snapshot {
            tick: self.tick,
            agents,
        }
    }
}
