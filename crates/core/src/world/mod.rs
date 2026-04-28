//! World-level primitives.
//!
//! At v0 this module holds spatial schema (`Vec2`, `Rect2`), the world
//! graph types (ADR 0007 — landed in a later sub-task of the spatial
//! pass), and the cross-cutting `Color` helper used by agent appearance.

mod types;

pub use types::{Rect2, Vec2};

use serde::{Deserialize, Serialize};

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
