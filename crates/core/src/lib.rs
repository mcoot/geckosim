//! Gecko-sim core: schema types and (later) the ECS-based simulation engine.
//!
//! At this scaffold pass, only the v0 schema from ADR 0011 is implemented.
//! The live `Sim` API, ECS components, and systems land in later passes.

pub mod agent;
pub mod decision;
pub mod events;
pub mod ids;
pub mod macro_;
pub mod object;
pub mod rng;
pub mod save;
pub mod systems;
pub mod time;
pub mod world;

// Convenience re-exports of the most-used public types.
pub use ids::{
    AccessoryId, AdvertisementId, AgentId, BuildingId, BusinessId, CrimeIncidentId, EmploymentId,
    HouseholdId, HousingId, LeafAreaId, MemoryEntryId, ObjectId, ObjectTypeId, OwnerRef,
    PromotedEventId,
};
pub use rng::PrngState;
pub use time::Tick;
pub use world::{Color, Vec2};
