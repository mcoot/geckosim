//! Lossy renderer-facing projection of `WorldGraph` per ADR 0013.
//!
//! Sent once in `ServerMessage::Init`. Hashmaps in `WorldGraph` flatten
//! to ID-sorted `Vec`s here so the serialized form is deterministic and
//! TypeScript-friendly.

use serde::{Deserialize, Serialize};

use crate::ids::LeafAreaId;
use crate::world::graph::WorldGraph;
use crate::world::types::{Building, District, Floor, LeafArea};

/// One-shot world layout payload for the renderer. Wraps the
/// `WorldGraph` in deterministic, sorted vectors so JSON round-trip is
/// byte-stable for a given graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct WorldLayout {
    pub districts: Vec<District>,
    pub buildings: Vec<Building>,
    pub floors: Vec<Floor>,
    pub leaves: Vec<LeafArea>,
    pub default_spawn_leaf: LeafAreaId,
}

impl From<&WorldGraph> for WorldLayout {
    fn from(g: &WorldGraph) -> Self {
        let mut districts: Vec<District> = g.districts.values().cloned().collect();
        districts.sort_by_key(|d| d.id);
        let mut buildings: Vec<Building> = g.buildings.values().cloned().collect();
        buildings.sort_by_key(|b| b.id);
        let mut floors: Vec<Floor> = g.floors.values().cloned().collect();
        floors.sort_by_key(|f| f.id);
        let mut leaves: Vec<LeafArea> = g.leaves.values().cloned().collect();
        leaves.sort_by_key(|l| l.id);
        Self {
            districts,
            buildings,
            floors,
            leaves,
            default_spawn_leaf: g.default_spawn_leaf,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorldLayout;
    use crate::world::WorldGraph;

    #[test]
    fn layout_from_seed_v0_is_deterministic() {
        let g1 = WorldGraph::seed_v0();
        let g2 = WorldGraph::seed_v0();
        assert_eq!(WorldLayout::from(&g1), WorldLayout::from(&g2));
    }

    #[test]
    fn layout_leaves_are_sorted_by_id() {
        let g = WorldGraph::seed_v0();
        let l = WorldLayout::from(&g);
        let mut prev = 0u64;
        for leaf in &l.leaves {
            assert!(
                leaf.id.raw() >= prev,
                "not sorted: {prev} → {}",
                leaf.id.raw()
            );
            prev = leaf.id.raw();
        }
    }
}
