//! Live `Sim` driver: wraps a `bevy_ecs::World` and advances tick state
//! via a `bevy_ecs::schedule::Schedule`.
//!
//! Honours the public API contract from ADR 0012 partially:
//!   - `new`, `tick`, `current_tick`, `snapshot`.
//!   - `delta_since`, `apply_input` deferred to a later pass.

use std::collections::HashMap;

use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
use bevy_ecs::world::World;

use crate::agent::{
    Accessory, AccessoryCatalog, Facing, Identity, Memory, MemoryEntry, Mood, Needs, Personality,
    Position,
};
use crate::decision::{CurrentAction, RecentActionsRing};
use crate::ids::{AccessoryId, AgentId, LeafAreaId, ObjectId, ObjectTypeId};
use crate::object::{ObjectCatalog, ObjectType, SmartObject};
use crate::rng::PrngState;
use crate::snapshot::{AgentSnapshot, ObjectSnapshot, Snapshot};
use crate::systems;
use crate::systems::memory::MemoryIdAllocator;
use crate::time::CurrentTick;
use crate::world::{Vec2, WorldGraph};

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

/// Wrapper around the global `PrngState` so systems can borrow it via
/// `ResMut<SimRngResource>`. Per-agent RNG sub-streams are deferred per
/// the spec's "RNG plumbing" section.
#[derive(bevy_ecs::prelude::Resource, Debug)]
pub struct SimRngResource(pub PrngState);

/// The live simulation. Owns its `bevy_ecs::World`, a `Schedule` of
/// per-tick systems, and the canonical clock.
pub struct Sim {
    world: World,
    schedule: Schedule,
    tick: u64,
    next_agent_id: u64,
    next_object_id: u64,
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
        world.insert_resource(SimRngResource(PrngState::from_seed(seed)));
        world.insert_resource(WorldGraph::seed_v0());
        world.insert_resource(CurrentTick(0));
        world.insert_resource(MemoryIdAllocator::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                systems::needs::decay,
                systems::mood::update,
                systems::decision::execute::execute,
                systems::movement::walk,
                systems::decision::decide::decide,
            )
                .chain(),
        );

        Self {
            world,
            schedule,
            tick: 0,
            next_agent_id: 0,
            next_object_id: 0,
        }
    }

    /// Advance the simulation by one tick (one sim-minute per ADR 0008).
    /// Increments the tick counter first so systems see the tick they're
    /// processing via `Res<CurrentTick>`.
    pub fn tick(&mut self) -> TickReport {
        self.tick += 1;
        *self.world.resource_mut::<CurrentTick>() = CurrentTick(self.tick);
        self.schedule.run(&mut self.world);
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

    /// Borrow the world graph (ADR 0007). Inserted by `Sim::new` from
    /// `WorldGraph::seed_v0()`; later content passes will swap in
    /// content-loaded graphs.
    #[must_use]
    pub fn world_graph(&self) -> &WorldGraph {
        self.world
            .get_resource::<WorldGraph>()
            .expect("WorldGraph resource is inserted in Sim::new")
    }

    /// Borrow an agent's memory entries by stable `AgentId`.
    #[must_use]
    pub fn agent_memory(&self, agent_id: AgentId) -> Option<&[MemoryEntry]> {
        self.world.iter_entities().find_map(|entity_ref| {
            let identity = entity_ref.get::<Identity>()?;
            if identity.id != agent_id {
                return None;
            }
            entity_ref
                .get::<Memory>()
                .map(|memory| memory.entries.as_slice())
        })
    }

    /// Spawn a fresh agent at full needs and neutral mood with a
    /// monotonically allocated `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced in a future pass.
    pub fn spawn_test_agent(&mut self, name: &str) -> AgentId {
        self.spawn_test_agent_with_needs(name, Needs::full())
    }

    /// Spawn a fresh agent with explicit initial needs, neutral mood,
    /// a sampled personality, a `Position` at the world graph's
    /// `default_spawn_leaf` (offset deterministically by `next_agent_id`
    /// along +x), `Facing::default()`, and decision-runtime components
    /// (no current action, empty recent-actions ring). Test-only entry.
    ///
    /// Personality is sampled from the world's `SimRngResource` so spawn
    /// order is deterministic from the seed (per ADR 0008). The spawn
    /// offset is structural (no RNG) so personality stays the only RNG
    /// draw per spawn.
    pub fn spawn_test_agent_with_needs(&mut self, name: &str, needs: Needs) -> AgentId {
        let id = AgentId::new(self.next_agent_id);
        self.next_agent_id += 1;
        let personality = {
            let mut rng = self.world.resource_mut::<SimRngResource>();
            Personality::sample(&mut rng.0.0)
        };
        let spawn_leaf = self.world.resource::<WorldGraph>().default_spawn_leaf;
        let position = Position {
            leaf: spawn_leaf,
            pos: spawn_offset(id.raw()),
        };
        let facing = Facing::default();
        self.world.spawn((
            Identity {
                id,
                name: name.to_string(),
            },
            needs,
            Mood::neutral(),
            personality,
            Memory::default(),
            position,
            facing,
            CurrentAction::default(),
            RecentActionsRing::default(),
        ));
        id
    }

    /// Spawn a smart-object instance of the given catalog type. Test-only
    /// entry point until content-driven instance spawning lands.
    /// Reads the type's `default_state` from the catalog and stamps it on
    /// the new instance. Returns the freshly allocated `ObjectId`.
    ///
    /// # Panics
    /// Panics if `type_id` is not in the loaded `ObjectCatalog`.
    pub fn spawn_test_object(
        &mut self,
        type_id: ObjectTypeId,
        location: LeafAreaId,
        position: Vec2,
    ) -> ObjectId {
        let id = ObjectId::new(self.next_object_id);
        self.next_object_id += 1;
        let default_state = self
            .world
            .resource::<ObjectCatalog>()
            .by_id
            .get(&type_id)
            .unwrap_or_else(|| panic!("ObjectTypeId {type_id:?} not in catalog"))
            .default_state
            .clone();
        self.world.spawn(SmartObject {
            id,
            type_id,
            location,
            position,
            owner: None,
            state: default_state,
        });
        id
    }

    /// Spawn one instance of every loaded `ObjectType`. Convenience for
    /// the host's seed-instance spawn at startup. Iterates type IDs in
    /// sorted order for deterministic `ObjectId` allocation.
    pub fn spawn_one_of_each_object_type(
        &mut self,
        location: LeafAreaId,
        position: Vec2,
    ) -> Vec<ObjectId> {
        let mut type_ids: Vec<ObjectTypeId> = self
            .world
            .resource::<ObjectCatalog>()
            .by_id
            .keys()
            .copied()
            .collect();
        type_ids.sort();
        type_ids
            .into_iter()
            .map(|t| self.spawn_test_object(t, location, position))
            .collect()
    }

    /// Capture the full sim state at the current tick. Agents are sorted
    /// by `AgentId` ascending and objects by `ObjectId` ascending for
    /// determinism (per ADR 0008).
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let mut agents: Vec<AgentSnapshot> = self
            .world
            .iter_entities()
            .filter_map(|entity_ref| {
                let identity = entity_ref.get::<Identity>()?;
                let needs = entity_ref.get::<Needs>()?;
                let mood = entity_ref.get::<Mood>()?;
                let personality = entity_ref.get::<Personality>().copied().unwrap_or_default();
                let position = entity_ref.get::<Position>().copied()?;
                let facing = entity_ref.get::<Facing>().copied().unwrap_or_default();
                let action_phase = entity_ref
                    .get::<CurrentAction>()
                    .and_then(|c| c.0.as_ref())
                    .map(|a| a.phase);
                let current_action = entity_ref
                    .get::<CurrentAction>()
                    .and_then(|c| c.0.as_ref())
                    .and_then(|action| project_current_action(action, self.tick, self));
                Some(AgentSnapshot {
                    id: identity.id,
                    name: identity.name.clone(),
                    needs: *needs,
                    mood: *mood,
                    personality,
                    leaf: position.leaf,
                    pos: position.pos,
                    facing: facing.dir,
                    action_phase,
                    current_action,
                })
            })
            .collect();
        agents.sort_by_key(|a| a.id);

        let mut objects: Vec<ObjectSnapshot> = self
            .world
            .iter_entities()
            .filter_map(|entity_ref| {
                let o = entity_ref.get::<SmartObject>()?;
                Some(ObjectSnapshot {
                    id: o.id,
                    type_id: o.type_id,
                    leaf: o.location,
                    pos: o.position,
                })
            })
            .collect();
        objects.sort_by_key(|o| o.id);

        Snapshot {
            tick: self.tick,
            agents,
            objects,
        }
    }
}

/// Deterministic spawn-offset grid for v0 spawn helpers — agents land
/// 1m apart along the +x axis. Avoids the RNG so personality sampling
/// stays the only RNG draw per spawn.
#[allow(
    clippy::cast_precision_loss,
    reason = "v0 agent counts (≤ thousands) sit comfortably below f32 precision limits"
)]
fn spawn_offset(agent_index: u64) -> Vec2 {
    Vec2::new(agent_index as f32, 0.0)
}

/// Build a `CurrentActionView` from a `CommittedAction`. Looks up the
/// advertisement's `display_name` via the catalog (for object-targeted
/// actions); falls back to `"Idle"` / `"Wait"` for self-actions.
/// Returns `None` only if the catalog lookup fails for an object action,
/// which would indicate a data-flow bug — we log and produce `None`.
#[allow(
    clippy::cast_precision_loss,
    reason = "tick numbers are small u64s; f32 precision is fine for fraction display"
)]
fn project_current_action(
    action: &crate::decision::CommittedAction,
    current_tick: u64,
    sim: &Sim,
) -> Option<crate::snapshot::CurrentActionView> {
    // While Walking, expected_end_tick is None and the perform clock
    // hasn't started — show 0%. Once Performing, fraction is elapsed /
    // perform_duration_ticks.
    let fraction_complete = if action.phase == crate::decision::Phase::Performing
        && action.perform_duration_ticks > 0
    {
        let elapsed = current_tick.saturating_sub(action.started_tick) as f32;
        let duration = f32::from(u16::try_from(action.perform_duration_ticks).unwrap_or(u16::MAX));
        // u16 cap is generous: every v0 ad's duration_ticks is ≤ a few
        // hundred. Larger values just clamp to 1.0 below.
        (elapsed / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let display_name = match action.action {
        crate::decision::ActionRef::SelfAction(crate::decision::SelfActionKind::Idle) => {
            "Idle".to_string()
        }
        crate::decision::ActionRef::SelfAction(crate::decision::SelfActionKind::Wait) => {
            "Wait".to_string()
        }
        crate::decision::ActionRef::Object { object, ad } => {
            // Look up the smart-object instance to get its type, then
            // the catalog's advertisement display_name.
            let object_entry = sim
                .world
                .iter_entities()
                .find(|e| e.get::<crate::object::SmartObject>().is_some_and(|o| o.id == object))?;
            let smart_object = object_entry.get::<crate::object::SmartObject>()?;
            let object_type = sim
                .world
                .resource::<ObjectCatalog>()
                .by_id
                .get(&smart_object.type_id)?;
            let advertisement = object_type.advertisements.iter().find(|a| a.id == ad)?;
            advertisement.display_name.clone()
        }
    };
    Some(crate::snapshot::CurrentActionView {
        display_name,
        fraction_complete,
    })
}
