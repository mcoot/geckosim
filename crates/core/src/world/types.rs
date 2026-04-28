//! Spatial schema primitives per ADR 0007.

use serde::{Deserialize, Serialize};

use crate::ids::{BuildingId, DistrictId, FloorId, LeafAreaId};

/// 2D vector / point in meters. Wire-friendly `{ x, y }` shape so JSON
/// inspection and TypeScript consumption read naturally. Replaces the
/// former `pub use glam::Vec2;` re-export — `glam` is no longer a runtime
/// dependency of the schema.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned 2D bounding box in meters. ADR 0007 leaf-area / building
/// / district footprint shape.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Rect2 {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect2 {
    #[must_use]
    pub const fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum OutdoorZoneKind {
    Plaza,
    Forecourt,
    StreetSegment,
}

/// What sits inside a leaf area — either a room of a specific floor in a
/// specific building, or an outdoor zone of a given kind. Per ADR 0007.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub enum LeafKind {
    Room { building: BuildingId, floor: FloorId },
    OutdoorZone(OutdoorZoneKind),
}

/// A leaf area: room or outdoor zone. Per ADR 0007 these are the only
/// places agents and smart objects actually live; everything else
/// (district / building / floor) is a containment shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct LeafArea {
    pub id: LeafAreaId,
    pub display_name: String,
    pub kind: LeafKind,
    pub bbox: Rect2,
    /// Other leaf areas reachable in one step. Symmetric in
    /// `WorldGraph::seed_v0`; later content loaders should validate.
    pub adjacency: Vec<LeafAreaId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct District {
    pub id: DistrictId,
    pub display_name: String,
    pub bbox: Rect2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Building {
    pub id: BuildingId,
    pub display_name: String,
    pub district: DistrictId,
    pub footprint: Rect2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Floor {
    pub id: FloorId,
    pub building: BuildingId,
    /// Ground floor = 0; basement levels negative. Per ADR 0007.
    pub level: i16,
}
