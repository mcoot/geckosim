//! Live `Sim` driver: wraps a `bevy_ecs::World` and advances tick state
//! via a `bevy_ecs::schedule::Schedule`.
//!
//! Honours the public API contract from ADR 0012 partially:
//!   - `new`, `tick`, `current_tick`, `snapshot`.
//!   - `delta_since`, `apply_input` deferred to a later pass.

use std::collections::HashMap;

use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
use bevy_ecs::world::World;

use crate::agent::{Accessory, AccessoryCatalog, Identity, Mood, Needs};
use crate::ids::{AccessoryId, AgentId, ObjectTypeId};
use crate::object::{ObjectCatalog, ObjectType};
use crate::rng::PrngState;
use crate::snapshot::{AgentSnapshot, Snapshot};
use crate::systems;

/// Catalog data passed into `Sim::new`. Loaded from RON files by the
/// `gecko-sim-content` crate; populated maps after a real load, empty maps
/// after `ContentBundle::default()`.
#[derive(Debug, Clone, Default)]
pub struct ContentBundle {
    pub object_types: HashMap<ObjectTypeId, ObjectType>,
    pub accessories: HashMap<AccessoryId, Accessory>,
}

/// Per-tick stats returned from `Sim::tick`. Empty placeholder; future
/// per-tick counters (decisions made, interrupts raised, promoted events
/// emitted, …) live here.
#[derive(Debug, Clone, Default)]
pub struct TickReport;

/// The live simulation. Owns its `bevy_ecs::World`, a `Schedule` of
/// per-tick systems, and the canonical clock.
pub struct Sim {
    world: World,
    schedule: Schedule,
    tick: u64,
    // Stored for determinism; needs-decay does not consume randomness, but
    // future systems will. The seed is captured here at construction time.
    #[expect(dead_code, reason = "consumed by first RNG-using system in a later pass")]
    rng: PrngState,
    next_agent_id: u64,
}

impl Sim {
    /// Construct a fresh sim with the given world seed and content bundle.
    /// Builds the per-tick `Schedule` once with all v0 systems in their
    /// canonical order; `tick` runs the schedule unchanged each call.
    #[must_use]
    pub fn new(seed: u64, content: ContentBundle) -> Self {
        let mut world = World::new();
        world.insert_resource(ObjectCatalog {
            by_id: content.object_types,
        });
        world.insert_resource(AccessoryCatalog {
            by_id: content.accessories,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems((systems::needs::decay, systems::mood::update).chain());

        Self {
            world,
            schedule,
            tick: 0,
            rng: PrngState::from_seed(seed),
            next_agent_id: 0,
        }
    }

    /// Advance the simulation by one tick (one sim-minute per ADR 0008).
    /// Runs the per-tick `Schedule` against the world.
    pub fn tick(&mut self) -> TickReport {
        self.schedule.run(&mut self.world);
        self.tick += 1;
        TickReport
    }

    /// Current tick count. Starts at 0; each `tick()` increments by 1.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    /// Borrow the loaded object-type catalog. Mirror of the
    /// `Res<ObjectCatalog>` view that systems will use.
    #[must_use]
    pub fn object_catalog(&self) -> &ObjectCatalog {
        self.world
            .get_resource::<ObjectCatalog>()
            .expect("ObjectCatalog resource is inserted in Sim::new")
    }

    /// Borrow the loaded accessory catalog. Mirror of the
    /// `Res<AccessoryCatalog>` view that systems will use.
    #[must_use]
    pub fn accessory_catalog(&self) -> &AccessoryCatalog {
        self.world
            .get_resource::<AccessoryCatalog>()
            .expect("AccessoryCatalog resource is inserted in Sim::new")
    }

    /// Spawn a fresh agent at full needs and neutral mood with a
    /// monotonically allocated `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced in a future pass.
    pub fn spawn_test_agent(&mut self, name: &str) -> AgentId {
        self.spawn_test_agent_with_needs(name, Needs::full())
    }

    /// Spawn a fresh agent with explicit initial needs and neutral mood.
    /// Test-only entry point used by the mood integration test to seed
    /// empty needs without poking the ECS world directly.
    pub fn spawn_test_agent_with_needs(&mut self, name: &str, needs: Needs) -> AgentId {
        let id = AgentId::new(self.next_agent_id);
        self.next_agent_id += 1;
        self.world.spawn((
            Identity {
                id,
                name: name.to_string(),
            },
            needs,
            Mood::neutral(),
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
