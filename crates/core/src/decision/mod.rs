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

use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// ECS components for the decision runtime (per ADR 0011)
// ---------------------------------------------------------------------------

/// Wrapper around the optional committed action so it lives as an ECS
/// component. `None` means the agent is awaiting a decision next tick.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, PartialEq, Default, Serialize, Deserialize,
)]
pub struct CurrentAction(pub Option<CommittedAction>);

/// Bounded ring of recent action templates. FIFO eviction at 16 entries
/// (per ADR 0011). Used by the recency penalty in scoring.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, PartialEq, Default, Serialize, Deserialize,
)]
pub struct RecentActionsRing {
    pub entries: VecDeque<RecentActionEntry>,
}

impl RecentActionsRing {
    /// Per ADR 0011.
    pub const CAPACITY: usize = 16;

    /// Push one entry, evicting the oldest if at capacity.
    pub fn push(&mut self, entry: RecentActionEntry) {
        if self.entries.len() >= Self::CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// True if any entry's `ad_template` matches `(type_id, ad_id)`.
    #[must_use]
    pub fn contains(&self, type_id: crate::ids::ObjectTypeId, ad_id: crate::ids::AdvertisementId) -> bool {
        self.entries
            .iter()
            .any(|e| e.ad_template == (type_id, ad_id))
    }
}

/// `SelfAction(Idle)` duration when no advertisements survive predicate
/// filtering. Re-decides 5 ticks later rather than every tick.
pub const IDLE_DURATION_TICKS: u32 = 5;
