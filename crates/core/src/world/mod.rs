//! World-level primitives.
//!
//! At v0 this module is intentionally small: just the math/color primitives
//! used by other modules. The hierarchical spatial graph from ADR 0007
//! (district → building → floor → room → zone) lands here in a later pass.

use serde::{Deserialize, Serialize};

/// 2D position vector, re-exported from `glam`. Used for in-leaf-area positions
/// (snapped to a 0.5m grid for object alignment per ADR 0007).
pub use glam::Vec2;

/// 24-bit RGB color. No alpha, no HDR — appearance is pure 8-bit RGB at v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}
