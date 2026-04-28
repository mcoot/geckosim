//! Stable u64-newtype identifiers per ADR 0011.
//!
//! These are the canonical IDs used in saves, the wire protocol, and all
//! cross-references between sim entities. ECS `Entity` handles are not
//! serialized — see ADR 0012 ("Identity surfaces").

use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
        #[cfg_attr(
            feature = "export-ts",
            ts(export, export_to = "../../apps/web/src/types/sim/")
        )]
        pub struct $name(
            #[cfg_attr(feature = "export-ts", ts(type = "number"))] pub u64,
        );

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn raw(self) -> u64 {
                self.0
            }
        }
    };
}

id_newtype!(AgentId);
id_newtype!(ObjectId);
id_newtype!(ObjectTypeId);
id_newtype!(BuildingId);
id_newtype!(FloorId);
id_newtype!(DistrictId);
id_newtype!(LeafAreaId);
id_newtype!(HousingId);
id_newtype!(EmploymentId);
id_newtype!(HouseholdId);
id_newtype!(BusinessId);
id_newtype!(CrimeIncidentId);
id_newtype!(MemoryEntryId);
id_newtype!(AccessoryId);
id_newtype!(AdvertisementId);
id_newtype!(PromotedEventId);

impl LeafAreaId {
    /// v0 stub: every agent and smart-object instance lives in this single
    /// implicit leaf area until the spatial pass introduces a real world
    /// graph (ADR 0007). The decision-runtime's spatial predicate evaluator
    /// returns `true` for all `Predicate::Spatial(_)` variants at v0.
    pub const DEFAULT: Self = Self::new(0);
}

/// An owner reference for entities that can be owned by an agent, household, or business
/// (e.g. a fridge belongs to a household; a register belongs to a business).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OwnerRef {
    Agent(AgentId),
    Household(HouseholdId),
    Business(BusinessId),
}
