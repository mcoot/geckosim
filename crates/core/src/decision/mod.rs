//! Decision runtime types per ADR 0011 (action commitment & interruption).

use serde::{Deserialize, Serialize};

use crate::agent::Need;
use crate::ids::{AdvertisementId, ObjectId, ObjectTypeId};
use crate::world::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Walking,
    Performing,
    Completing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SelfActionKind {
    /// Agent-internal action with no smart object (e.g. wait, idle).
    Wait,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionRef {
    /// Action against a smart-object advertisement.
    Object {
        object: ObjectId,
        ad: AdvertisementId,
    },
    /// Self-action (no smart-object target).
    SelfAction(SelfActionKind),
}

// `Copy` is intentionally NOT derived: ADR 0011 (open questions) calls out
// action chaining as a deferred extension (`next: Option<Box<CommittedAction>>`),
// which will not be `Copy`. Dropping `Copy` now keeps that change non-breaking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommittedAction {
    pub action: ActionRef,
    pub started_tick: u64,
    pub expected_end_tick: u64,
    pub phase: Phase,
    pub target_position: Option<Vec2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterruptSource {
    NeedThreshold,
    MacroForcedAction,
    MacroPreconditionFailed,
    EnvironmentalEvent,
    AgentTargeted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterruptPayload {
    None,
    NeedThreshold { need: Need },
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Interrupt {
    pub source: InterruptSource,
    pub urgency: f32,
    pub payload: InterruptPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecentActionEntry {
    /// Template identity across instances per ADR 0011.
    pub ad_template: (ObjectTypeId, AdvertisementId),
    pub completed_tick: u64,
}
