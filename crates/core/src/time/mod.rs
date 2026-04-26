//! Simulation time primitives per ADR 0008.

use serde::{Deserialize, Serialize};

/// One micro-tick of simulation. ADR 0008 fixes 1 tick = 1 sim-minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tick(pub u64);

impl Tick {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Number of micro-ticks in one sim-hour (per ADR 0008).
pub const TICKS_PER_SIM_HOUR: u64 = 60;

/// Number of micro-ticks in one sim-day.
pub const TICKS_PER_SIM_DAY: u64 = TICKS_PER_SIM_HOUR * 24;

/// Macro tick cadence per ADR 0009: one macro tick per sim-hour.
pub const TICKS_PER_MACRO_TICK: u64 = TICKS_PER_SIM_HOUR;
