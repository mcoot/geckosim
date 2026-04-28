//! Spatial schema primitives per ADR 0007.

use serde::{Deserialize, Serialize};

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
