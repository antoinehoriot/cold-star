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
}
