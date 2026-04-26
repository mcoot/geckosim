//! ECS systems per ADR 0010 / 0012.
//!
//! Each v0 system from ADR 0010 will land as its own submodule:
//!   - `needs`         (1) need decay
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
//! Systems are added in a later pass alongside the live `Sim` API.

// Empty — see ADR 0010 for the v0 system list.
