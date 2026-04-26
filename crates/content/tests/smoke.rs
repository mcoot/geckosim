//! Smoke test for `gecko-sim-content`. The crate is intentionally empty at
//! the scaffold pass; this test just confirms it compiles and links to
//! `gecko-sim-core`.

use gecko_sim_core::AgentId;

#[test]
fn dep_chain_resolves() {
    let _ = AgentId::new(0);
}
