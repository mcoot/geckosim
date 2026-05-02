//! Interaction-target resolution for object actions.

use std::collections::HashSet;

use crate::agent::Position;
use crate::ids::{InteractionSpotId, ObjectId};
use crate::object::{ObjectType, SmartObject};
use crate::world::{Rect2, Vec2};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedInteractionTarget {
    pub position: Vec2,
    pub spot: Option<InteractionSpotId>,
    pub facing: Option<Vec2>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct OccupiedInteractionSpots {
    occupied: HashSet<(ObjectId, InteractionSpotId)>,
}

impl OccupiedInteractionSpots {
    pub(crate) fn insert(&mut self, object: ObjectId, spot: InteractionSpotId) {
        self.occupied.insert((object, spot));
    }

    #[must_use]
    pub(crate) fn contains(&self, object: ObjectId, spot: InteractionSpotId) -> bool {
        self.occupied.contains(&(object, spot))
    }
}

#[must_use]
pub(crate) fn resolve_interaction_target(
    object: &SmartObject,
    object_type: &ObjectType,
    agent_position: &Position,
    occupied: &OccupiedInteractionSpots,
    leaf_bbox: Option<Rect2>,
) -> Option<ResolvedInteractionTarget> {
    if object_type.interaction_spots.is_empty() {
        return Some(ResolvedInteractionTarget {
            position: object.position,
            spot: None,
            facing: None,
        });
    }

    let mut best: Option<(f32, InteractionSpotId, ResolvedInteractionTarget)> = None;
    for spot in &object_type.interaction_spots {
        if occupied.contains(object.id, spot.id) {
            continue;
        }

        let position = Vec2::new(
            object.position.x + spot.offset.x,
            object.position.y + spot.offset.y,
        );
        if leaf_bbox.is_some_and(|bbox| !contains_point(bbox, position)) {
            tracing::warn!(
                object = ?object.id,
                spot = ?spot.id,
                "interaction spot falls outside leaf bounds; skipping"
            );
            continue;
        }
        let Some(facing) = normalize(spot.facing) else {
            tracing::warn!(
                object = ?object.id,
                spot = ?spot.id,
                "interaction spot has invalid facing; skipping"
            );
            continue;
        };

        let distance = distance_sq(agent_position.pos, position);
        let target = ResolvedInteractionTarget {
            position,
            spot: Some(spot.id),
            facing: Some(facing),
        };
        let replace = best
            .as_ref()
            .is_none_or(|(best_distance, best_spot, _)| {
                distance.total_cmp(best_distance).is_lt()
                    || (distance.total_cmp(best_distance).is_eq() && spot.id < *best_spot)
            });
        if replace {
            best = Some((distance, spot.id, target));
        }
    }

    best.map(|(_, _, target)| target)
}

fn normalize(v: Vec2) -> Option<Vec2> {
    let len_sq = v.x * v.x + v.y * v.y;
    if !len_sq.is_finite() || len_sq == 0.0 {
        return None;
    }
    let len = len_sq.sqrt();
    Some(Vec2::new(v.x / len, v.y / len))
}

fn distance_sq(a: Vec2, b: Vec2) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

fn contains_point(rect: Rect2, point: Vec2) -> bool {
    point.x >= rect.min.x
        && point.x <= rect.max.x
        && point.y >= rect.min.y
        && point.y <= rect.max.y
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::agent::Position;
    use crate::ids::{InteractionSpotId, LeafAreaId, ObjectId, ObjectTypeId};
    use crate::object::{InteractionSpot, MeshId, ObjectType, SmartObject};
    use crate::systems::decision::interaction::{
        OccupiedInteractionSpots, resolve_interaction_target,
    };
    use crate::world::{Rect2, Vec2};

    fn spot(id: u64, offset: Vec2, facing: Vec2) -> InteractionSpot {
        InteractionSpot {
            id: InteractionSpotId::new(id),
            offset,
            facing,
            label: None,
        }
    }

    fn object_type_with_spots(interaction_spots: Vec<InteractionSpot>) -> ObjectType {
        ObjectType {
            id: ObjectTypeId::new(1),
            display_name: "Test object".to_string(),
            mesh_id: MeshId(1),
            default_state: HashMap::new(),
            interaction_spots,
            advertisements: vec![],
        }
    }

    fn smart_object_at(position: Vec2) -> SmartObject {
        SmartObject {
            id: ObjectId::new(7),
            type_id: ObjectTypeId::new(1),
            location: LeafAreaId::new(3),
            position,
            owner: None,
            state: HashMap::new(),
        }
    }

    fn agent_at(position: Vec2) -> Position {
        Position {
            leaf: LeafAreaId::new(3),
            pos: position,
        }
    }

    #[test]
    fn resolves_spot_offset_and_normalized_facing() {
        let object = smart_object_at(Vec2::new(10.0, 10.0));
        let object_type = object_type_with_spots(vec![spot(
            1,
            Vec2::new(0.0, -1.0),
            Vec2::new(0.0, 2.0),
        )]);

        let resolved = resolve_interaction_target(
            &object,
            &object_type,
            &agent_at(Vec2::new(10.0, 0.0)),
            &OccupiedInteractionSpots::default(),
            None,
        )
        .expect("spot available");

        assert_eq!(resolved.spot, Some(InteractionSpotId::new(1)));
        assert_eq!(resolved.position, Vec2::new(10.0, 9.0));
        assert_eq!(resolved.facing, Some(Vec2::new(0.0, 1.0)));
    }

    #[test]
    fn picks_nearest_available_spot() {
        let object = smart_object_at(Vec2::new(10.0, 10.0));
        let object_type = object_type_with_spots(vec![
            spot(1, Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)),
            spot(2, Vec2::new(5.0, 0.0), Vec2::new(-1.0, 0.0)),
        ]);

        let resolved = resolve_interaction_target(
            &object,
            &object_type,
            &agent_at(Vec2::new(11.0, 8.0)),
            &OccupiedInteractionSpots::default(),
            None,
        )
        .expect("spot available");

        assert_eq!(resolved.spot, Some(InteractionSpotId::new(1)));
    }

    #[test]
    fn skips_occupied_spots() {
        let object = smart_object_at(Vec2::new(10.0, 10.0));
        let object_type = object_type_with_spots(vec![
            spot(1, Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)),
            spot(2, Vec2::new(5.0, 0.0), Vec2::new(-1.0, 0.0)),
        ]);
        let mut occupied = OccupiedInteractionSpots::default();
        occupied.insert(object.id, InteractionSpotId::new(1));

        let resolved = resolve_interaction_target(
            &object,
            &object_type,
            &agent_at(Vec2::new(11.0, 8.0)),
            &occupied,
            None,
        )
        .expect("second spot available");

        assert_eq!(resolved.spot, Some(InteractionSpotId::new(2)));
    }

    #[test]
    fn returns_none_when_all_spots_are_occupied() {
        let object = smart_object_at(Vec2::new(10.0, 10.0));
        let object_type = object_type_with_spots(vec![spot(
            1,
            Vec2::new(0.0, -1.0),
            Vec2::new(0.0, 1.0),
        )]);
        let mut occupied = OccupiedInteractionSpots::default();
        occupied.insert(object.id, InteractionSpotId::new(1));

        let resolved = resolve_interaction_target(
            &object,
            &object_type,
            &agent_at(Vec2::new(11.0, 8.0)),
            &occupied,
            None,
        );

        assert!(resolved.is_none());
    }

    #[test]
    fn object_without_spots_falls_back_to_center() {
        let object = smart_object_at(Vec2::new(10.0, 10.0));
        let object_type = object_type_with_spots(vec![]);

        let resolved = resolve_interaction_target(
            &object,
            &object_type,
            &agent_at(Vec2::new(11.0, 8.0)),
            &OccupiedInteractionSpots::default(),
            None,
        )
        .expect("center fallback available");

        assert_eq!(resolved.spot, None);
        assert_eq!(resolved.position, Vec2::new(10.0, 10.0));
        assert_eq!(resolved.facing, None);
    }

    #[test]
    fn skips_spots_outside_leaf_bounds() {
        let object = smart_object_at(Vec2::new(10.0, 10.0));
        let object_type = object_type_with_spots(vec![
            spot(1, Vec2::new(100.0, 0.0), Vec2::new(-1.0, 0.0)),
            spot(2, Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)),
        ]);

        let resolved = resolve_interaction_target(
            &object,
            &object_type,
            &agent_at(Vec2::new(11.0, 8.0)),
            &OccupiedInteractionSpots::default(),
            Some(Rect2::new(Vec2::new(0.0, 0.0), Vec2::new(20.0, 20.0))),
        )
        .expect("bounded spot available");

        assert_eq!(resolved.spot, Some(InteractionSpotId::new(2)));
    }
}
