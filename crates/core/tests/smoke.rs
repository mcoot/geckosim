//! Smoke test: confirm the headline schema types compile and are reachable
//! from outside the crate via the public surface.

use gecko_sim_core::agent::{Mood, Needs, Personality};
use gecko_sim_core::ids::{AgentId, OwnerRef};
use gecko_sim_core::object::{Predicate, SmartObject};
use gecko_sim_core::{Color, PrngState, Tick, Vec2};

#[test]
fn ids_construct_and_round_trip() {
    let a = AgentId::new(42);
    assert_eq!(a.raw(), 42);
}

#[test]
fn primitives_construct() {
    let _ = Color::new(255, 128, 0);
    let _ = Vec2::new(1.0, 2.0);
    let _ = Tick::new(0);
    let _ = PrngState::from_seed(0xDEAD_BEEF);
}

#[test]
fn schema_types_are_reachable() {
    // We only need to name the types — instantiation requires populating
    // ~30 fields, which is the live-sim pass's job, not the scaffold's.
    let _ = std::mem::size_of::<Needs>();
    let _ = std::mem::size_of::<Personality>();
    let _ = std::mem::size_of::<Mood>();
    let _ = std::mem::size_of::<SmartObject>();
    let _ = std::mem::size_of::<Predicate>();
    let _ = std::mem::size_of::<OwnerRef>();
}
