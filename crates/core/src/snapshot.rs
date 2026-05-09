//! Snapshot type: a deterministic, by-value view of sim state at a tick.
//!
//! Agents are sorted by `AgentId` ascending and objects by `ObjectId`
//! ascending so two `Sim` instances built from the same seed and same
//! calls produce byte-equal `Snapshot`s. Serde derives let `Snapshot`
//! ride directly on the wire (per ADR 0013 and the WS transport v0
//! spec — wire types live in `protocol`, but the `Snapshot` shape
//! itself is the schema-of-record from `core`).

use serde::{Deserialize, Serialize};

use crate::agent::{Mood, Needs, Personality};
use crate::decision::Phase;
use crate::ids::{AgentId, LeafAreaId, ObjectId, ObjectTypeId};
use crate::world::Vec2;

/// Lossy projection of `CommittedAction` for the wire. Carries enough for
/// the frontend to render "Alice is doing X (50%)". The full
/// `CommittedAction` lives only as an ECS component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct CurrentActionView {
    /// `Advertisement.display_name` for object-targeted actions, or
    /// `"Idle"` / `"Wait"` for self-actions.
    pub display_name: String,
    /// Progress through the action's `perform_duration_ticks`. `0.0`
    /// while `Walking`; rises monotonically toward `1.0` while
    /// `Performing`.
    pub fraction_complete: f32,
    /// Current phase of the committed action.
    pub phase: Phase,
    /// Target object id for object-targeted actions.
    pub target_object_id: Option<ObjectId>,
    /// Committed interaction position for the current action.
    pub target_position: Option<Vec2>,
    /// Human-readable target label when one is known.
    pub target_label: Option<String>,
}

/// Per-instance smart-object row sent every snapshot. Carries the data
/// the renderer needs to draw the object: id, type (for mesh lookup),
/// leaf area, world-space position. Per-instance dynamic state
/// (e.g. fridge open/closed) is deferred until any object actually has
/// visible state — see the spatial-pass spec's deferred items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct ObjectSnapshot {
    pub id: ObjectId,
    pub type_id: ObjectTypeId,
    pub leaf: LeafAreaId,
    pub pos: Vec2,
}

/// Full sim state at a tick boundary. `PartialEq` is required by the
/// determinism test in the test suite; serde derives let `protocol`
/// envelope this type without a parallel wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Snapshot {
    #[cfg_attr(feature = "export-ts", ts(type = "number"))]
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
    pub objects: Vec<ObjectSnapshot>,
}

/// Per-agent snapshot row. Holds the slice of state this pass exposes;
/// other groupings (Memory, Skills, …) extend this type as their first
/// consumer system lands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
    pub mood: Mood,
    pub personality: Personality,
    pub leaf: LeafAreaId,
    pub pos: Vec2,
    pub facing: Vec2,
    pub action_phase: Option<Phase>,
    pub current_action: Option<CurrentActionView>,
}

#[cfg(test)]
mod serde_derive_tests {
    use super::{AgentSnapshot, CurrentActionView, ObjectSnapshot, Snapshot};

    fn assert_serialize<T: serde::Serialize>() {}
    fn assert_deserialize<T: serde::de::DeserializeOwned>() {}

    #[test]
    fn snapshot_types_implement_serde() {
        assert_serialize::<Snapshot>();
        assert_deserialize::<Snapshot>();
        assert_serialize::<AgentSnapshot>();
        assert_deserialize::<AgentSnapshot>();
        assert_serialize::<ObjectSnapshot>();
        assert_deserialize::<ObjectSnapshot>();
        assert_serialize::<CurrentActionView>();
        assert_deserialize::<CurrentActionView>();
    }
}
