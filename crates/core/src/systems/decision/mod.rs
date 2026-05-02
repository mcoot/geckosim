//! Decision runtime per ADR 0004 / 0011. Two systems registered in the
//! per-tick schedule (in this order, after `mood::update`):
//!
//! 1. `execute` — completes any committed action whose `expected_end_tick`
//!    has been reached: applies effects atomically, pushes a recent-actions
//!    ring entry, clears the agent's `current_action`.
//! 2. `decide`  — for each agent without a current action, evaluates every
//!    advertisement against preconditions, scores the survivors, picks
//!    weighted-random from top-N, commits the winner.
//!
//! Pure helpers for each phase live in their own submodule so they can be
//! unit-tested without ECS scaffolding.

pub mod decide;
pub mod effects;
pub mod execute;
pub(crate) mod interaction;
pub mod predicates;
pub mod scoring;
