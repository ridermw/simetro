//! Seeded random source for the simulation.
//!
//! Determinism (determinism contract) requires a fixed PRNG with no global state.
//! `Pcg64Mcg` is small, fast, and deterministic across platforms.

use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64Mcg;

/// Wrapper around `Pcg64Mcg` so we can swap the algorithm later without
/// touching every callsite.
#[derive(Debug, Clone)]
pub struct SimRng {
    inner: Pcg64Mcg,
}

impl SimRng {
    /// Create a new deterministic RNG.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self {
            inner: Pcg64Mcg::seed_from_u64(seed),
        }
    }

    /// Uniform `f32` in `[0, 1)`. Used for jitter and trivial random
    /// decisions; not for security.
    pub fn gen_f32(&mut self) -> f32 {
        self.inner.gen::<f32>()
    }

    /// Uniform `u32` in the full range.
    pub fn gen_u32(&mut self) -> u32 {
        self.inner.gen::<u32>()
    }

    /// Uniform `usize` in `[0, n)`. Returns 0 when `n == 0`.
    pub fn gen_range_usize(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            self.inner.gen_range(0..n)
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_produces_same_sequence() {
        let mut a = SimRng::from_seed(42);
        let mut b = SimRng::from_seed(42);
        let sa: Vec<u32> = (0..16).map(|_| a.gen_u32()).collect();
        let sb: Vec<u32> = (0..16).map(|_| b.gen_u32()).collect();
        assert_eq!(sa, sb);
    }

    #[test]
    fn different_seeds_produce_different_sequences() {
        let mut a = SimRng::from_seed(1);
        let mut b = SimRng::from_seed(2);
        // The chance of 16 identical u32s by accident is ~1 in 2^512.
        let sa: Vec<u32> = (0..16).map(|_| a.gen_u32()).collect();
        let sb: Vec<u32> = (0..16).map(|_| b.gen_u32()).collect();
        assert_ne!(sa, sb);
    }

    #[test]
    fn gen_f32_stays_in_unit_interval() {
        let mut r = SimRng::from_seed(7);
        for _ in 0..1000 {
            let f = r.gen_f32();
            assert!((0.0..1.0).contains(&f), "f32 out of range: {f}");
        }
    }

    #[test]
    fn gen_range_usize_handles_zero() {
        let mut r = SimRng::from_seed(0);
        assert_eq!(r.gen_range_usize(0), 0);
    }
}
