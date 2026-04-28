//! Spatial ECS components per ADR 0011 (lazy-shard projection of
//! `Gecko::current_leaf` / `Gecko::position` / `Gecko::facing`).

use serde::{Deserialize, Serialize};

use crate::ids::LeafAreaId;
use crate::world::Vec2;

/// Where an agent currently is. `leaf` and `pos` update together — an
/// agent crosses a leaf boundary by changing both fields at once. Per
/// ADR 0007: `pos` is continuous f32 within the leaf area; the 0.5m
/// grid mentioned in the ADR is enforced only at smart-object placement,
/// not on agent positions.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize,
)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Position {
    pub leaf: LeafAreaId,
    pub pos: Vec2,
}

/// Unit direction the agent is oriented. Updated by the movement system
/// during walking; mood / social systems may also nudge this without
/// moving the agent — kept as its own component so writes don't
/// conflict with `Position` writers.
#[derive(
    bevy_ecs::component::Component,
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize,
)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "export-ts",
    ts(export, export_to = "../../apps/web/src/types/sim/")
)]
pub struct Facing {
    pub dir: Vec2,
}

impl Default for Facing {
    fn default() -> Self {
        Self {
            dir: Vec2::new(1.0, 0.0),
        }
    }
}
