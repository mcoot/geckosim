//! ECS systems per ADR 0010 / 0012.
//!
//! Each v0 system from ADR 0010 lands as its own submodule:
//!   - `needs`         (1) need decay      ← landed
//!   - `personality`   (2) personality (read-only)
//!   - `mood`          (3) mood update     ← landed
//!   - `memory`        (4) memory ring & decay
//!   - …
//!
//! Plus cross-cutting:
//!   - `decision`      utility-AI scoring + commit + execute (per ADR 0004)  ← landed

pub mod decision;
pub mod mood;
pub mod movement;
pub mod needs;
