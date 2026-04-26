//! ECS system: needs decay. System #1 of 11 from ADR 0010.
//!
//! Each tick (one sim-minute per ADR 0008), every agent's six needs
//! decrement by a per-need rate, saturating at zero. Replenishment is
//! the responsibility of consumer-action systems landing in later
//! passes; this system never raises a need.

use bevy_ecs::world::World;

use crate::agent::Needs;

// Decay rates: each need empties from 1.0 to 0.0 over the listed
// sim-time. Placeholders for v0 — retunable when advertisement
// scoring lands. See ADR 0011 for the schema.
pub const HUNGER_DECAY_PER_TICK: f32 = 1.0 / 480.0; // empties in  8 sim-hours
pub const SLEEP_DECAY_PER_TICK: f32 = 1.0 / 960.0; // empties in 16 sim-hours
pub const SOCIAL_DECAY_PER_TICK: f32 = 1.0 / 720.0; // empties in 12 sim-hours
pub const HYGIENE_DECAY_PER_TICK: f32 = 1.0 / 480.0; // empties in  8 sim-hours
pub const FUN_DECAY_PER_TICK: f32 = 1.0 / 600.0; // empties in 10 sim-hours
pub const COMFORT_DECAY_PER_TICK: f32 = 1.0 / 360.0; // empties in  6 sim-hours

/// Apply one tick of needs decay to every entity with a `Needs` component.
///
/// Saturating subtraction at zero. No upper clamp — replenishment systems
/// are responsible for keeping values in `[0, 1]` from above.
pub(crate) fn decay(world: &mut World) {
    let mut query = world.query::<&mut Needs>();
    for mut needs in query.iter_mut(world) {
        needs.hunger = (needs.hunger - HUNGER_DECAY_PER_TICK).max(0.0);
        needs.sleep = (needs.sleep - SLEEP_DECAY_PER_TICK).max(0.0);
        needs.social = (needs.social - SOCIAL_DECAY_PER_TICK).max(0.0);
        needs.hygiene = (needs.hygiene - HYGIENE_DECAY_PER_TICK).max(0.0);
        needs.fun = (needs.fun - FUN_DECAY_PER_TICK).max(0.0);
        needs.comfort = (needs.comfort - COMFORT_DECAY_PER_TICK).max(0.0);
    }
}
