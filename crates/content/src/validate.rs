//! Validators for the loaded content. Pure functions over the loader's
//! `Vec<(PathBuf, T)>` output. Validation runs after loading and before
//! the entries are collected into the `ContentBundle` maps so duplicate
//! detection sees both colliding paths.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use gecko_sim_core::agent::Accessory;
use gecko_sim_core::ids::{AccessoryId, ObjectTypeId};
use gecko_sim_core::object::{ObjectType, Predicate};

use crate::error::ContentError;

pub(crate) fn validate_object_types(
    entries: &[(PathBuf, ObjectType)],
) -> Result<(), ContentError> {
    let mut seen: HashMap<ObjectTypeId, PathBuf> = HashMap::new();
    for (path, ot) in entries {
        if let Some(prev) = seen.get(&ot.id) {
            return Err(ContentError::DuplicateObjectTypeId {
                id: ot.id,
                first: prev.clone(),
                second: path.clone(),
            });
        }
        validate_interaction_spots(path, ot)?;
        validate_advertisements(path, ot)?;
        seen.insert(ot.id, path.clone());
    }
    Ok(())
}

pub(crate) fn validate_accessories(
    entries: &[(PathBuf, Accessory)],
) -> Result<(), ContentError> {
    let mut seen: HashMap<AccessoryId, PathBuf> = HashMap::new();
    for (path, acc) in entries {
        if let Some(prev) = seen.get(&acc.id) {
            return Err(ContentError::DuplicateAccessoryId {
                id: acc.id,
                first: prev.clone(),
                second: path.clone(),
            });
        }
        seen.insert(acc.id, path.clone());
    }
    Ok(())
}

fn validate_interaction_spots(path: &Path, ot: &ObjectType) -> Result<(), ContentError> {
    let mut seen_spots = HashSet::new();
    for spot in &ot.interaction_spots {
        if !seen_spots.insert(spot.id) {
            return Err(ContentError::DuplicateInteractionSpotId {
                object_type: ot.id,
                spot: spot.id,
                path: path.to_path_buf(),
            });
        }
        if !spot.offset.x.is_finite() || !spot.offset.y.is_finite() {
            return Err(ContentError::InvalidInteractionSpotVector {
                object_type: ot.id,
                spot: spot.id,
                reason: "offset must be finite",
                path: path.to_path_buf(),
            });
        }
        if !spot.facing.x.is_finite() || !spot.facing.y.is_finite() {
            return Err(ContentError::InvalidInteractionSpotVector {
                object_type: ot.id,
                spot: spot.id,
                reason: "facing must be finite",
                path: path.to_path_buf(),
            });
        }
        if spot.facing.x == 0.0 && spot.facing.y == 0.0 {
            return Err(ContentError::InvalidInteractionSpotVector {
                object_type: ot.id,
                spot: spot.id,
                reason: "facing must be non-zero",
                path: path.to_path_buf(),
            });
        }
    }
    Ok(())
}

fn validate_advertisements(path: &Path, ot: &ObjectType) -> Result<(), ContentError> {
    let mut seen_ads = HashSet::new();
    for ad in &ot.advertisements {
        if !seen_ads.insert(ad.id) {
            return Err(ContentError::DuplicateAdvertisementId {
                object_type: ot.id,
                ad: ad.id,
                path: path.to_path_buf(),
            });
        }
        if ad.duration_ticks == 0 {
            return Err(ContentError::ZeroDuration {
                object_type: ot.id,
                ad: ad.id,
                path: path.to_path_buf(),
            });
        }
        // ObjectState predicate keys must reference default_state.
        for predicate in &ad.preconditions {
            if let Predicate::ObjectState(key, _, _) = predicate {
                if !ot.default_state.contains_key(key) {
                    let mut known: Vec<_> = ot.default_state.keys().cloned().collect();
                    known.sort();
                    return Err(ContentError::UnknownObjectStateKey {
                        object_type: ot.id,
                        ad: ad.id,
                        key: key.clone(),
                        known,
                        path: path.to_path_buf(),
                    });
                }
            }
        }
        // No duplicate Need in score_template.need_weights.
        let mut seen_needs = HashSet::new();
        for (need, _) in &ad.score_template.need_weights {
            if !seen_needs.insert(*need) {
                return Err(ContentError::DuplicateNeedWeight {
                    object_type: ot.id,
                    ad: ad.id,
                    need: *need,
                    path: path.to_path_buf(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use gecko_sim_core::ids::{InteractionSpotId, ObjectTypeId};
    use gecko_sim_core::object::{InteractionSpot, MeshId, ObjectType};
    use gecko_sim_core::world::Vec2;

    use super::validate_object_types;
    use crate::error::ContentError;

    fn object_type_with_spot(offset: Vec2, facing: Vec2) -> ObjectType {
        ObjectType {
            id: ObjectTypeId::new(1),
            display_name: "Chair".to_string(),
            mesh_id: MeshId(1),
            default_state: HashMap::new(),
            interaction_spots: vec![InteractionSpot {
                id: InteractionSpotId::new(1),
                offset,
                facing,
                label: None,
            }],
            advertisements: vec![],
        }
    }

    #[test]
    fn non_finite_interaction_spot_offset_rejected() {
        let object_type = object_type_with_spot(
            Vec2::new(f32::INFINITY, 0.0),
            Vec2::new(0.0, 1.0),
        );
        let err = validate_object_types(&[(PathBuf::from("chair.ron"), object_type)])
            .expect_err("should fail");
        assert!(
            matches!(err, ContentError::InvalidInteractionSpotVector { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn non_finite_interaction_spot_facing_rejected() {
        let object_type = object_type_with_spot(
            Vec2::new(0.0, -1.0),
            Vec2::new(f32::NAN, 1.0),
        );
        let err = validate_object_types(&[(PathBuf::from("chair.ron"), object_type)])
            .expect_err("should fail");
        assert!(
            matches!(err, ContentError::InvalidInteractionSpotVector { .. }),
            "got {err:?}"
        );
    }
}
