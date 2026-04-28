//! Gecko-sim core: schema types and the ECS-based simulation engine.

pub mod agent;
pub mod decision;
pub mod events;
pub mod ids;
pub mod macro_;
pub mod object;
pub mod rng;
pub mod save;
pub mod sim;
pub mod snapshot;
pub mod systems;
pub mod time;
pub mod world;

// Convenience re-exports of the most-used public types.
pub use ids::{
    AccessoryId, AdvertisementId, AgentId, BuildingId, BusinessId, CrimeIncidentId, EmploymentId,
    HouseholdId, HousingId, LeafAreaId, MemoryEntryId, ObjectId, ObjectTypeId, OwnerRef,
    PromotedEventId,
};
pub use agent::{Accessory, AccessoryCatalog, AccessorySlot};
pub use decision::{CurrentAction, RecentActionsRing, IDLE_DURATION_TICKS};
pub use object::ObjectCatalog;
pub use rng::PrngState;
pub use sim::{ContentBundle, Sim, TickReport};
pub use snapshot::{AgentSnapshot, CurrentActionView, Snapshot};
pub use time::{CurrentTick, Tick};
pub use world::{Color, Rect2, Vec2};
