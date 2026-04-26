//! Seeded PRNG state per ADR 0008.
//!
//! Each agent gets its own seeded sub-stream from the world seed; saves
//! preserve PRNG state so replays are deterministic.

use serde::{Deserialize, Serialize};

/// Per-agent (or per-stream) PRNG state. Wraps `rand_pcg::Pcg64Mcg` because
/// it's deterministic, has small state, and is fast — appropriate for a
/// per-agent sub-stream that gets stepped many times per tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrngState(pub rand_pcg::Pcg64Mcg);

impl PrngState {
    /// Construct a fresh PRNG seeded with the given 64-bit seed.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        use rand::SeedableRng;
        Self(rand_pcg::Pcg64Mcg::seed_from_u64(seed))
    }
}
