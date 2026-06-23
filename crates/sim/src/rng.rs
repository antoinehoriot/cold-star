//! Deterministic, dependency-free PRNG (splitmix64). Stored in `GameState`
//! so the whole simulation is reproducible from (seed, state).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    /// Seed the stream was created from (kept for debugging / reproduction).
    pub seed: u64,
    /// Evolving internal state.
    pub state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { seed, state: seed }
    }

    /// splitmix64: advance state and return the next u64.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform float in [0, 1) using the top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

/// Mix a `u64` seed with a `u32` id into a fresh sub-seed (splitmix64 finalizer).
/// Pure: used to derive independent generation streams per system/site.
pub fn hash(seed: u64, id: u32) -> u64 {
    let mut z = seed
        .wrapping_add((id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let seq_a: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let seq_b: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(seq_a, seq_b);
    }

    #[test]
    fn different_seed_diverges() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn advancing_changes_state() {
        let mut a = Rng::new(7);
        let before = a.state;
        a.next_u64();
        assert_ne!(a.state, before);
    }

    #[test]
    fn float_in_unit_interval() {
        let mut a = Rng::new(99);
        for _ in 0..1000 {
            let f = a.next_f64();
            assert!((0.0..1.0).contains(&f), "f64 out of range: {f}");
        }
    }

    #[test]
    fn known_splitmix64_output_for_seed_zero() {
        // Pins the canonical splitmix64 first output for seed 0, guarding against
        // an accidental constant/shift typo that the relative tests wouldn't catch.
        let mut r = Rng::new(0);
        assert_eq!(r.next_u64(), 0xe220a8397b1dcdaf);
    }

    #[test]
    fn hash_is_deterministic_and_disperses() {
        assert_eq!(hash(123, 7), hash(123, 7)); // stable
        assert_ne!(hash(123, 7), hash(123, 8)); // id matters
        assert_ne!(hash(123, 7), hash(124, 7)); // seed matters
        // Distinct sub-seeds across a small id range (no collisions in 0..32).
        let mut seen = std::collections::BTreeSet::new();
        for id in 0..32u32 {
            assert!(seen.insert(hash(0xDEADBEEF, id)), "collision at {id}");
        }
    }

    #[test]
    fn hash_pins_a_known_value() {
        // Guards against an accidental constant/shift change.
        assert_eq!(hash(0, 0), 16294208416658607535u64);
    }
}
