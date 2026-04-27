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
