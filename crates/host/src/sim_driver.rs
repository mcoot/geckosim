//! Sim-driver task: ticks `Sim`, samples snapshots, handles pacing inputs.
//!
//! Topology and rationale: see `docs/superpowers/specs/2026-04-27-ws-transport-v0-design.md`.
//! Single-task `tokio::select!` loop with three arms:
//!   1. tick deadline (paced at `1.0 / speed` sec; disabled when `speed == 0.0`)
//!   2. sample interval (33 ms wall-clock, always)
//!   3. input channel (handled inline; pacing inputs never enter `Sim`)
//!
//! `block_in_place` wraps `sim.tick()` and `sim.snapshot()` so a slow
//! tick doesn't starve other tasks on the same tokio worker.

use std::time::Duration;

use gecko_sim_core::{Sim, Snapshot};
use gecko_sim_protocol::PlayerInput;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, sleep_until};

const SAMPLE_PERIOD: Duration = Duration::from_millis(33);
const MAX_SPEED: f32 = 64.0;
const DEFAULT_SPEED: f32 = 1.0;

/// Driver-side pacing state. Lives on the `sim_driver` task; never seen by
/// the sim or by clients beyond its observable effect on tick rate.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PacingState {
    speed: f32,
    last_nonzero_speed: f32,
}

impl Default for PacingState {
    fn default() -> Self {
        Self {
            speed: DEFAULT_SPEED,
            last_nonzero_speed: DEFAULT_SPEED,
        }
    }
}

impl PacingState {
    fn apply(&mut self, input: PlayerInput) {
        match input {
            PlayerInput::SetSpeed { multiplier } => {
                let m = if multiplier.is_nan() {
                    0.0
                } else {
                    multiplier.clamp(0.0, MAX_SPEED)
                };
                self.speed = m;
                if m > 0.0 {
                    self.last_nonzero_speed = m;
                }
            }
            PlayerInput::TogglePause => {
                self.speed = if self.speed == 0.0 {
                    self.last_nonzero_speed
                } else {
                    0.0
                };
            }
        }
    }

    fn tick_period(self) -> Option<Duration> {
        if self.speed > 0.0 {
            Some(Duration::from_secs_f32(1.0 / self.speed))
        } else {
            None
        }
    }
}

/// Drive the sim. Owns `sim`, drains `input_rx`, publishes snapshots
/// to `snapshot_tx` at 33 ms cadence.
pub async fn run(
    mut sim: Sim,
    mut input_rx: mpsc::UnboundedReceiver<PlayerInput>,
    snapshot_tx: watch::Sender<Snapshot>,
) {
    let mut pacing = PacingState::default();
    let mut last_tick_at = Instant::now();

    let mut sample = tokio::time::interval(SAMPLE_PERIOD);
    sample.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        let next_tick_deadline = pacing.tick_period().map(|p| last_tick_at + p);

        tokio::select! {
            biased;

            _ = sample.tick() => {
                let snap = tokio::task::block_in_place(|| sim.snapshot());
                let _ = snapshot_tx.send_replace(snap);
            }

            () = wait_for_optional_deadline(next_tick_deadline) => {
                tokio::task::block_in_place(|| { sim.tick(); });
                last_tick_at = Instant::now();
            }

            maybe_input = input_rx.recv() => {
                match maybe_input {
                    Some(input) => pacing.apply(input),
                    None => break, // sender dropped — host is shutting down
                }
            }
        }
    }
}

/// `select!`-friendly wrapper: when `Some(deadline)`, sleep until it;
/// when `None`, never resolve.
async fn wait_for_optional_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(d) => sleep_until(d).await,
        None => std::future::pending::<()>().await,
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact comparisons against literal speeds we set
mod tests {
    use super::*;

    #[test]
    fn default_speed_is_one() {
        let p = PacingState::default();
        assert_eq!(p.speed, 1.0);
        assert_eq!(p.last_nonzero_speed, 1.0);
        assert_eq!(p.tick_period(), Some(Duration::from_secs_f32(1.0)));
    }

    #[test]
    fn set_speed_clamps_above_64() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 1000.0 });
        assert_eq!(p.speed, 64.0);
        assert_eq!(p.last_nonzero_speed, 64.0);
    }

    #[test]
    fn set_speed_clamps_below_zero() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: -5.0 });
        assert_eq!(p.speed, 0.0);
        // Negative input does not update last_nonzero_speed.
        assert_eq!(p.last_nonzero_speed, 1.0);
    }

    #[test]
    fn set_speed_nan_treated_as_zero() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: f32::NAN });
        assert_eq!(p.speed, 0.0);
    }

    #[test]
    fn set_speed_zero_pauses_without_losing_resume_value() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 4.0 });
        assert_eq!(p.last_nonzero_speed, 4.0);
        p.apply(PlayerInput::SetSpeed { multiplier: 0.0 });
        assert_eq!(p.speed, 0.0);
        assert_eq!(p.last_nonzero_speed, 4.0); // remembered for resume
    }

    #[test]
    fn toggle_pause_from_running_pauses() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 8.0 });
        p.apply(PlayerInput::TogglePause);
        assert_eq!(p.speed, 0.0);
        assert_eq!(p.last_nonzero_speed, 8.0);
    }

    #[test]
    fn toggle_pause_from_paused_restores_last_nonzero() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 8.0 });
        p.apply(PlayerInput::TogglePause); // pause
        p.apply(PlayerInput::TogglePause); // resume
        assert_eq!(p.speed, 8.0);
    }

    #[test]
    fn paused_has_no_tick_period() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 0.0 });
        assert_eq!(p.tick_period(), None);
    }

    #[test]
    fn tick_period_inverts_speed() {
        let mut p = PacingState::default();
        p.apply(PlayerInput::SetSpeed { multiplier: 4.0 });
        assert_eq!(p.tick_period(), Some(Duration::from_secs_f32(0.25)));
    }
}
