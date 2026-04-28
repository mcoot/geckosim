//! Hierarchical world graph (ADR 0007) and the v0 seed world.

use std::collections::HashMap;

use bevy_ecs::prelude::Resource;

use crate::ids::{BuildingId, DistrictId, FloorId, LeafAreaId};
use crate::world::types::{
    Building, District, Floor, LeafArea, LeafKind, OutdoorZoneKind, Rect2, Vec2,
};

/// World graph resource. Inserted by `Sim::new` from `WorldGraph::seed_v0()`.
/// At v0 the seed is hard-coded; a later content pass will load from RON.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct WorldGraph {
    pub districts: HashMap<DistrictId, District>,
    pub buildings: HashMap<BuildingId, Building>,
    pub floors: HashMap<FloorId, Floor>,
    pub leaves: HashMap<LeafAreaId, LeafArea>,
    /// Default leaf where the spawn helpers drop new agents and where
    /// the host's seed-instance smart objects land. Single-leaf at v0
    /// keeps every advertisement reachable without cross-leaf
    /// pathfinding.
    pub default_spawn_leaf: LeafAreaId,
}

impl WorldGraph {
    /// Build the v0 seed world: one district, one building (one floor,
    /// one living-room leaf), one plaza outdoor zone, one forecourt
    /// outdoor zone. Adjacency: plaza ↔ forecourt; forecourt ↔
    /// living-room. `default_spawn_leaf` resolves to the living-room.
    #[must_use]
    pub fn seed_v0() -> Self {
        let district_id = DistrictId::new(1);
        let building_id = BuildingId::new(1);
        let floor_id = FloorId::new(1);

        let plaza = LeafAreaId::new(1);
        let forecourt = LeafAreaId::new(2);
        let living_room = LeafAreaId::new(3);

        let district = District {
            id: district_id,
            display_name: "Old Town".into(),
            bbox: Rect2::new(Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0)),
        };
        let building = Building {
            id: building_id,
            display_name: "Hearth Cottage".into(),
            district: district_id,
            footprint: Rect2::new(Vec2::new(80.0, 80.0), Vec2::new(120.0, 120.0)),
        };
        let floor = Floor {
            id: floor_id,
            building: building_id,
            level: 0,
        };

        let mut leaves = HashMap::new();
        leaves.insert(
            plaza,
            LeafArea {
                id: plaza,
                display_name: "Town Plaza".into(),
                kind: LeafKind::OutdoorZone(OutdoorZoneKind::Plaza),
                bbox: Rect2::new(Vec2::new(0.0, 0.0), Vec2::new(80.0, 80.0)),
                adjacency: vec![forecourt],
            },
        );
        leaves.insert(
            forecourt,
            LeafArea {
                id: forecourt,
                display_name: "Cottage Forecourt".into(),
                kind: LeafKind::OutdoorZone(OutdoorZoneKind::Forecourt),
                bbox: Rect2::new(Vec2::new(60.0, 60.0), Vec2::new(140.0, 80.0)),
                adjacency: vec![plaza, living_room],
            },
        );
        leaves.insert(
            living_room,
            LeafArea {
                id: living_room,
                display_name: "Living Room".into(),
                kind: LeafKind::Room {
                    building: building_id,
                    floor: floor_id,
                },
                bbox: Rect2::new(Vec2::new(80.0, 80.0), Vec2::new(120.0, 120.0)),
                adjacency: vec![forecourt],
            },
        );

        let mut districts = HashMap::new();
        districts.insert(district_id, district);
        let mut buildings = HashMap::new();
        buildings.insert(building_id, building);
        let mut floors = HashMap::new();
        floors.insert(floor_id, floor);

        Self {
            districts,
            buildings,
            floors,
            leaves,
            default_spawn_leaf: living_room,
        }
    }

    #[must_use]
    pub fn leaf(&self, id: LeafAreaId) -> Option<&LeafArea> {
        self.leaves.get(&id)
    }

    /// True iff `b` appears in `a`'s adjacency list. Does **not** check
    /// the reverse — `seed_v0` constructs symmetric edges; later content
    /// loaders should validate.
    #[must_use]
    pub fn are_adjacent(&self, a: LeafAreaId, b: LeafAreaId) -> bool {
        self.leaves
            .get(&a)
            .is_some_and(|l| l.adjacency.contains(&b))
    }
}

#[cfg(test)]
mod tests {
    use super::WorldGraph;

    #[test]
    fn seed_v0_has_one_district_one_building_one_floor_three_leaves() {
        let g = WorldGraph::seed_v0();
        assert_eq!(g.districts.len(), 1);
        assert_eq!(g.buildings.len(), 1);
        assert_eq!(g.floors.len(), 1);
        assert_eq!(g.leaves.len(), 3);
    }

    #[test]
    fn seed_v0_adjacency_is_symmetric() {
        let g = WorldGraph::seed_v0();
        for (id, leaf) in &g.leaves {
            for adj in &leaf.adjacency {
                assert!(
                    g.are_adjacent(*adj, *id),
                    "asymmetric edge {id:?} → {adj:?}"
                );
            }
        }
    }

    #[test]
    fn seed_v0_default_spawn_leaf_resolves() {
        let g = WorldGraph::seed_v0();
        assert!(g.leaf(g.default_spawn_leaf).is_some());
    }
}
