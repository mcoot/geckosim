//! Snapshot type: a deterministic, by-value view of sim state at a tick.
//!
//! Agents are sorted by `AgentId` ascending so two `Sim` instances built
//! from the same seed and same calls produce byte-equal `Snapshot`s.
//! Serde derives let `Snapshot` ride directly on the wire (per ADR 0013
//! and the WS transport v0 spec — wire types live in `protocol`, but the
//! `Snapshot` shape itself is the schema-of-record from `core`).

use serde::{Deserialize, Serialize};

use crate::agent::Needs;
use crate::ids::AgentId;

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
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
}

/// Per-agent snapshot row. Holds the slice of state this pass exposes;
/// other groupings (Personality, Mood, Spatial, …) extend this type as
/// their first consumer system lands.
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
}

#[cfg(test)]
mod serde_derive_tests {
    use super::{AgentSnapshot, Snapshot};

    fn assert_serialize<T: serde::Serialize>() {}
    fn assert_deserialize<T: serde::de::DeserializeOwned>() {}

    #[test]
    fn snapshot_types_implement_serde() {
        assert_serialize::<Snapshot>();
        assert_deserialize::<Snapshot>();
        assert_serialize::<AgentSnapshot>();
        assert_deserialize::<AgentSnapshot>();
    }
}
