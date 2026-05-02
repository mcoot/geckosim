//! Smart-object schema and the advertisement contract per ADR 0011.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::agent::{
    ConditionKind, ItemType, MemoryKind, MoodDim, Need, Personality, RelField, Skill, TargetSpec,
};
use crate::ids::{
    AdvertisementId, InteractionSpotId, LeafAreaId, ObjectId, ObjectTypeId, OwnerRef,
};
use crate::world::Vec2;

// ---------------------------------------------------------------------------
// Catalog and instance
// ---------------------------------------------------------------------------

/// Renderer mesh hint. Kept opaque at v0 — content authors fill these in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MeshId(pub u32);

/// Content-defined string key into a smart object's `StateMap`. ADR 0011
/// vocabulary; aliased to `String` at v0 with room to harden into an interned
/// or typed key later.
pub type StateKey = String;

/// Type-specific instance state. Keys are content-defined string identifiers;
/// values are typed.
pub type StateMap = HashMap<StateKey, StateValue>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StateValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectType {
    pub id: ObjectTypeId,
    pub display_name: String,
    pub mesh_id: MeshId,
    pub default_state: StateMap,
    #[serde(default)]
    pub interaction_spots: Vec<InteractionSpot>,
    pub advertisements: Vec<Advertisement>,
}

/// A usable spot around a smart-object instance. `offset` is in the
/// object's leaf-local coordinates relative to `SmartObject::position`;
/// `facing` is normalized during target resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionSpot {
    pub id: InteractionSpotId,
    pub offset: Vec2,
    pub facing: Vec2,
    pub label: Option<String>,
}

/// Per-instance smart-object state (per ADR 0011). Doubles as the ECS
/// component on smart-object entities (lazy-sharding — schema and
/// component share a type until a future pass needs them to diverge).
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartObject {
    pub id: ObjectId,
    pub type_id: ObjectTypeId,
    pub location: LeafAreaId,
    pub position: Vec2,
    pub owner: Option<OwnerRef>,
    pub state: StateMap,
}

// ---------------------------------------------------------------------------
// Advertisement contract
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterruptClass {
    Always,
    NeedsThresholdOnly,
    Never,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Advertisement {
    pub id: AdvertisementId,
    pub display_name: String,
    pub preconditions: Vec<Predicate>,
    pub effects: Vec<Effect>,
    pub duration_ticks: u32,
    pub interrupt_class: InterruptClass,
    pub score_template: ScoreTemplate,
}

// ---------------------------------------------------------------------------
// Predicates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Op {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpatialReq {
    SameLeafArea,
    AdjacentArea,
    KnownPlace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TickRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MacroVar {
    // Stub variants — populated from ADR 0009 in a later pass.
    Weather,
    EmploymentRate,
    CrimeRate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MacroValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Predicate {
    AgentNeed(Need, Op, f32),
    AgentSkill(Skill, Op, f32),
    AgentInventory(ItemType, Op, u16),
    AgentRelationship(TargetSpec, RelField, Op, f32),
    ObjectState(StateKey, Op, StateValue),
    MacroState(MacroVar, Op, MacroValue),
    Spatial(SpatialReq),
    TimeOfDay(TickRange),
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealthChangeKind {
    AddCondition,
    RemoveCondition,
    AdjustVitality,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthChange {
    pub kind: HealthChangeKind,
    pub condition: Option<ConditionKind>,
    pub amount: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    // Stub — populated from ADR 0009's promoted-event taxonomy in a later pass.
    NeedCrisis,
    CrimeWitnessed,
    Death,
    Birth,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventPayload {
    None,
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    AgentNeedDelta(Need, f32),
    AgentMoodDelta(MoodDim, f32),
    AgentSkillDelta(Skill, f32),
    MoneyDelta(i64),
    InventoryDelta(ItemType, i32),
    MemoryGenerate {
        kind: MemoryKind,
        importance: f32,
        valence: f32,
        participants: TargetSpec,
    },
    RelationshipDelta(TargetSpec, RelField, f32),
    HealthConditionChange(HealthChange),
    PromotedEvent(EventType, EventPayload),
}

// (`PromotedEventId` is allocated by the sim during effect application, not
// stored on the effect itself — see ADR 0011 "Promoted-event emission".)

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SituationalModifier {
    MoodWeight { dim: MoodDim, weight: f32 },
    MacroVarWeight { var: MacroVar, weight: f32 },
    TimeOfDayWeight { peak_tick: u64, falloff: u32 },
    RelationshipWithTarget { field: RelField, weight: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreTemplate {
    pub need_weights: Vec<(Need, f32)>,
    pub personality_weights: Personality,
    pub situational_modifiers: Vec<SituationalModifier>,
}

// ---------------------------------------------------------------------------
// Object catalog resource
// ---------------------------------------------------------------------------

/// `bevy_ecs` resource holding the loaded object-type catalog. Keyed by
/// `ObjectTypeId`. Inserted by `Sim::new`.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Default)]
pub struct ObjectCatalog {
    pub by_id: HashMap<ObjectTypeId, ObjectType>,
}
