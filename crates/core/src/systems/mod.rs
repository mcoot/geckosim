//! ECS systems per ADR 0010 / 0012.
//!
//! Each v0 system from ADR 0010 lands as its own submodule:
//!   - `needs`         (1) need decay      ← landed
//!   - `personality`   (2) personality (read-only)
//!   - `mood`          (3) mood update
//!   - `memory`        (4) memory ring & decay
//!   - `relationships` (5) relationship updates
//!   - `skills`        (6) skill gain
//!   - `money`         (7) wages, transactions
//!   - `housing`       (8) residence assignment
//!   - `employment`    (9) job scheduling
//!   - `health`        (10) condition + vitality
//!   - `crime`         (11) crime + consequences
//!
//! Other systems join in later passes alongside additional ECS components.

pub mod needs;
