//! Agent (gecko) schema per ADR 0011.
//!
//! At this scaffold pass, `Gecko` is a single monolithic struct. Sharding
//! into ECS components (`Needs`, `Personality`, `Mood`, …) happens in the
//! next pass when the live `Sim` API lands.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ids::{
    AccessoryId, AgentId, CrimeIncidentId, EmploymentId, HouseholdId, HousingId, LeafAreaId,
    MemoryEntryId,
};
use crate::rng::PrngState;
use crate::world::{Color, Vec2};

// ---------------------------------------------------------------------------
// Identity / appearance
// ---------------------------------------------------------------------------

/// ECS component holding stable identity for an agent entity.
///
/// Lazy-sharded projection of `Gecko`'s identity fields (`id`, `name`).
/// The `Gecko` schema struct keeps its inline fields; `Identity` is
/// the runtime ECS view used by systems and snapshots.
#[derive(bevy_ecs::component::Component, Debug, Clone)]
pub struct Identity {
    pub id: AgentId,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
    NonBinary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pattern {
    Solid,
    Stripes,
    Spots,
    Mottled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Size {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TailLength {
    Short,
    Medium,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntrinsicAppearance {
    pub base_color: Color,
    pub pattern: Pattern,
    pub pattern_color: Color,
    pub eye_color: Color,
    pub body_size: Size,
    pub tail_length: TailLength,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub intrinsic: IntrinsicAppearance,
    /// Bounded at 4 accessory slots per ADR 0011.
    pub accessories: Vec<AccessoryId>,
}

// ---------------------------------------------------------------------------
// Accessory catalog (aesthetic-only at v0 per ADR 0011)
// ---------------------------------------------------------------------------

/// Where on a gecko an `Accessory` attaches. Mirrors the four-slot cap on
/// `Appearance.accessories`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessorySlot {
    Head,
    Neck,
    Body,
    Tail,
}

/// Static accessory catalog entry. Aesthetic-only at v0 — no scoring
/// effects, no economic value, no theft. The `mesh_id` is a renderer hint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Accessory {
    pub id: AccessoryId,
    pub display_name: String,
    pub mesh_id: crate::object::MeshId,
    pub slot: AccessorySlot,
}

/// `bevy_ecs` resource holding the loaded accessory catalog. Keyed by
/// `AccessoryId`. Inserted by `Sim::new`.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Default)]
pub struct AccessoryCatalog {
    pub by_id: HashMap<AccessoryId, Accessory>,
}

// ---------------------------------------------------------------------------
// (1) Needs
// ---------------------------------------------------------------------------

/// Need dimension. Cross-cutting — referenced by `Predicate::AgentNeed` and
/// `Effect::AgentNeedDelta` in the object module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Need {
    Hunger,
    Sleep,
    Social,
    Hygiene,
    Fun,
    Comfort,
}

/// All six need values, each in `[0, 1]`. Per ADR 0011. Doubles as the
/// ECS component for needs (lazy sharding — schema and component share a
/// type until a future pass needs them to diverge).
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub sleep: f32,
    pub social: f32,
    pub hygiene: f32,
    pub fun: f32,
    pub comfort: f32,
}

impl Needs {
    /// All needs at maximum (`1.0`). Convenience for spawning fresh agents.
    #[must_use]
    pub fn full() -> Self {
        Self {
            hunger: 1.0,
            sleep: 1.0,
            social: 1.0,
            hygiene: 1.0,
            fun: 1.0,
            comfort: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// (2) Personality — Big Five, components in [-1, 1]
// ---------------------------------------------------------------------------

/// Big Five personality components; each in `[-1, 1]`. Per ADR 0011.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Personality {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
}

// ---------------------------------------------------------------------------
// (3) Mood
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MoodDim {
    Valence,
    Arousal,
    Stress,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mood {
    pub valence: f32,
    pub arousal: f32,
    pub stress: f32,
}

// ---------------------------------------------------------------------------
// (4) Memory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryKind {
    SocialInteraction,
    Crime,
    LifeEvent,
    Routine,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: MemoryEntryId,
    pub kind: MemoryKind,
    pub tick: u64,
    /// Bounded at 4 participants per ADR 0011.
    pub participants: Vec<AgentId>,
    pub location: LeafAreaId,
    /// In `[-1, 1]`.
    pub valence: f32,
    /// In `[0, 1]`. Decays over time; eviction key.
    pub importance: f32,
}

// ---------------------------------------------------------------------------
// (5) Relationships
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelField {
    Affinity,
    Trust,
    Familiarity,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RelationshipEdge {
    /// In `[-1, 1]`.
    pub affinity: f32,
    /// In `[0, 1]`.
    pub trust: f32,
    /// In `[0, 1]`.
    pub familiarity: f32,
    pub last_interaction_tick: u64,
}

// ---------------------------------------------------------------------------
// (6) Skills
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skill {
    Social,
    Manual,
    Cognitive,
    Physical,
    Artistic,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Skills {
    pub social: f32,
    pub manual: f32,
    pub cognitive: f32,
    pub physical: f32,
    pub artistic: f32,
}

// ---------------------------------------------------------------------------
// (7) Money
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransactionKind {
    Wage,
    Purchase,
    Sale,
    Gift,
    Theft,
    Fine,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub tick: u64,
    pub kind: TransactionKind,
    /// Minor units (cents); per ADR 0011.
    pub amount: i64,
    pub counterparty: Option<AgentId>,
}

// ---------------------------------------------------------------------------
// (10) Health
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConditionKind {
    Illness,
    Injury,
    ChronicCondition,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HealthCondition {
    pub kind: ConditionKind,
    pub severity: f32,
    pub onset_tick: u64,
    pub expected_recovery_tick: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthState {
    pub conditions: Vec<HealthCondition>,
    /// Hits 0 → death (per ADR 0011).
    pub vitality: f32,
}

// ---------------------------------------------------------------------------
// (11) Crime
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Consequence {
    Fine { amount: i64, due_tick: u64 },
    CommunityService { hours: u32, due_tick: u64 },
    Probation { until_tick: u64 },
    Incarceration { until_tick: u64 },
}

// ---------------------------------------------------------------------------
// Cross-cutting: inventory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Food,
    WorkMaterial,
    StolenGood,
    Gift,
    Other,
}

bitflags::bitflags! {
    /// Per-item provenance flags (ADR 0011, "Inventory stacking").
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct ItemFlags: u8 {
        const STOLEN = 0b0000_0001;
        const GIFT = 0b0000_0010;
        const CONTRABAND = 0b0000_0100;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemMeta {
    pub origin: Option<AgentId>,
    pub origin_tick: u64,
    pub flags: ItemFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InventorySlot {
    pub item: ItemType,
    pub count: u16,
    pub metadata: Option<ItemMeta>,
}

// ---------------------------------------------------------------------------
// Top-level: Gecko
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gecko {
    // Identity
    pub id: AgentId,
    pub name: String,
    pub age: u16,
    pub gender: Gender,
    pub birth_tick: u64,
    pub alive: bool,

    pub appearance: Appearance,

    // Spatial
    pub current_leaf: LeafAreaId,
    pub position: Vec2,
    pub facing: Vec2,

    // Per-system state (numbering matches ADR 0010)
    pub needs: Needs,
    pub personality: Personality,
    pub mood: Mood,
    /// Bounded ring at 500 entries per ADR 0011 (importance × recency eviction).
    pub memory: Vec<MemoryEntry>,
    /// Sparse map; only non-trivial pairs stored.
    pub relationships: HashMap<AgentId, RelationshipEdge>,
    pub skills: Skills,
    /// Minor units (cents) per ADR 0011.
    pub money: i64,
    /// Bounded ring at 32 entries per ADR 0011.
    pub recent_transactions: Vec<Transaction>,
    // (8) Housing & (9) Employment
    pub residence: Option<HousingId>,
    pub household: Option<HouseholdId>,
    pub employment: Option<EmploymentId>,
    pub health: HealthState,
    pub criminal_record: Vec<CrimeIncidentId>,
    pub pending_consequences: Vec<Consequence>,

    // Cross-cutting
    /// Bounded at 8 slots per ADR 0011.
    pub inventory: Vec<InventorySlot>,

    // Decision runtime (per ADR 0004)
    pub current_action: Option<crate::decision::CommittedAction>,
    pub pending_interrupts: Vec<crate::decision::Interrupt>,
    /// Bounded ring at 16 entries per ADR 0011 (FIFO eviction).
    pub recent_actions: Vec<crate::decision::RecentActionEntry>,
    /// Bounded at 64 entries per ADR 0011.
    pub known_places: Vec<LeafAreaId>,

    // Determinism (per ADR 0008)
    pub rng: PrngState,
}

/// How to pick a `NearbyAgent` target — see `TargetSpec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NearbySelector {
    Random,
    Closest,
    HighestAffinity,
}

/// Reference target for action effects and predicates (per ADR 0011).
///
/// `Self_` carries a trailing underscore to dodge the `self` keyword while
/// keeping ADR 0011's vocabulary verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetSpec {
    Self_,
    OwnerOfObject,
    OtherAgent { id: AgentId },
    NearbyAgent { selector: NearbySelector },
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact-constant comparisons against `1.0` literals
mod needs_component_tests {
    use super::Needs;
    use bevy_ecs::world::World;

    #[test]
    fn needs_full_is_all_ones() {
        let n = Needs::full();
        assert_eq!(n.hunger, 1.0);
        assert_eq!(n.sleep, 1.0);
        assert_eq!(n.social, 1.0);
        assert_eq!(n.hygiene, 1.0);
        assert_eq!(n.fun, 1.0);
        assert_eq!(n.comfort, 1.0);
    }

    #[test]
    fn needs_can_be_inserted_as_component() {
        let mut world = World::new();
        let entity = world.spawn(Needs::full()).id();
        let needs = world.get::<Needs>(entity).expect("Needs component present");
        assert_eq!(needs.hunger, 1.0);
    }
}

#[cfg(test)]
mod identity_component_tests {
    use super::{Identity, Needs};
    use crate::ids::AgentId;
    use bevy_ecs::world::World;

    #[test]
    fn identity_can_be_inserted_alongside_needs() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Identity {
                    id: AgentId::new(7),
                    name: "Alice".to_string(),
                },
                Needs::full(),
            ))
            .id();
        let id = world.get::<Identity>(entity).expect("Identity component present");
        assert_eq!(id.id, AgentId::new(7));
        assert_eq!(id.name, "Alice");
    }
}
