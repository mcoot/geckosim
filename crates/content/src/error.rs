//! Typed errors for the content loader. Authors hit these at startup —
//! messages always include a path so failures are bisectable.

use std::path::PathBuf;

use gecko_sim_core::agent::Need;
use gecko_sim_core::ids::{
    AccessoryId, AdvertisementId, InteractionSpotId, ObjectTypeId,
};
use gecko_sim_core::object::StateKey;

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse RON in {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: ron::error::SpannedError,
    },

    #[error("duplicate ObjectTypeId {id:?} in {first} and {second}",
        first = first.display(), second = second.display())]
    DuplicateObjectTypeId {
        id: ObjectTypeId,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("duplicate AccessoryId {id:?} in {first} and {second}",
        first = first.display(), second = second.display())]
    DuplicateAccessoryId {
        id: AccessoryId,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("duplicate AdvertisementId {ad:?} within ObjectType {object_type:?} in {path}",
        path = path.display())]
    DuplicateAdvertisementId {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        path: PathBuf,
    },

    #[error(
        "advertisement {ad:?} on ObjectType {object_type:?} ({path}) references unknown ObjectState key {key:?}; \
         known keys: {known:?}",
        path = path.display()
    )]
    UnknownObjectStateKey {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        key: StateKey,
        known: Vec<StateKey>,
        path: PathBuf,
    },

    #[error("advertisement {ad:?} on ObjectType {object_type:?} ({path}) repeats need_weight {need:?}",
        path = path.display())]
    DuplicateNeedWeight {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        need: Need,
        path: PathBuf,
    },

    #[error("advertisement {ad:?} on ObjectType {object_type:?} ({path}) has duration_ticks=0",
        path = path.display())]
    ZeroDuration {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        path: PathBuf,
    },

    #[error("duplicate InteractionSpotId {spot:?} within ObjectType {object_type:?} in {path}",
        path = path.display())]
    DuplicateInteractionSpotId {
        object_type: ObjectTypeId,
        spot: InteractionSpotId,
        path: PathBuf,
    },

    #[error("invalid interaction spot {spot:?} on ObjectType {object_type:?} ({path}): {reason}",
        path = path.display())]
    InvalidInteractionSpotVector {
        object_type: ObjectTypeId,
        spot: InteractionSpotId,
        reason: &'static str,
        path: PathBuf,
    },
}
