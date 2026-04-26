//! Smoke test for `gecko-sim-protocol`. Empty crate body at the scaffold
//! pass; the smoke test confirms the dep chain compiles.

use gecko_sim_core::AgentId;

#[test]
fn dep_chain_resolves() {
    let _ = AgentId::new(0);
}
