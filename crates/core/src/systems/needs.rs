//! ECS system: needs decay. System #1 of 11 from ADR 0010.
//!
//! Each tick (one sim-minute per ADR 0008), every agent's six needs
//! decrement by a per-need rate, saturating at zero. Replenishment is
//! the responsibility of consumer-action systems landing in later
//! passes; this system never raises a need.

use bevy_ecs::system::Query;

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
pub(crate) fn decay(mut needs: Query<&mut Needs>) {
    for mut n in &mut needs {
        n.hunger = (n.hunger - HUNGER_DECAY_PER_TICK).max(0.0);
        n.sleep = (n.sleep - SLEEP_DECAY_PER_TICK).max(0.0);
        n.social = (n.social - SOCIAL_DECAY_PER_TICK).max(0.0);
        n.hygiene = (n.hygiene - HYGIENE_DECAY_PER_TICK).max(0.0);
        n.fun = (n.fun - FUN_DECAY_PER_TICK).max(0.0);
        n.comfort = (n.comfort - COMFORT_DECAY_PER_TICK).max(0.0);
    }
}
