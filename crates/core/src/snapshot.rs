//! Snapshot type: a deterministic, by-value view of sim state at a tick.
//!
//! Agents are sorted by `AgentId` ascending so two `Sim` instances built
//! from the same seed and same calls produce byte-equal `Snapshot`s.

use crate::agent::Needs;
use crate::ids::AgentId;

/// Full sim state at a tick boundary. `PartialEq` is required by the
/// determinism test in the test suite.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
}

/// Per-agent snapshot row. Holds the slice of state this pass exposes;
/// other groupings (Personality, Mood, Spatial, …) extend this type as
/// their first consumer system lands.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
}
