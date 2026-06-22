//! Top-level game state and the two engine entry points: `advance` (time) and
//! `apply` (player intent). Both are pure functions of state + tuning.

use crate::config::Tuning;
use crate::origin::Origin;
use crate::reactor::Reactor;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

pub const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    StokeCore,
}

/// Why an action was refused. Carries a human-readable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected(pub &'static str);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub version: u32,
    pub seed: u64,
    pub rng: Rng,
    pub tick: u64,
    pub origin: Origin,
    pub reactor: Reactor,
}

impl GameState {
    pub fn new(tuning: &Tuning, seed: u64) -> Self {
        Self {
            version: SAVE_VERSION,
            seed,
            rng: Rng::new(seed),
            tick: 0,
            origin: Origin::Drifter,
            reactor: Reactor::new(tuning),
        }
    }

    /// Advance the simulation by `ticks` whole ticks.
    pub fn advance(&mut self, tuning: &Tuning, ticks: u64) {
        for _ in 0..ticks {
            self.reactor.tick(tuning);
            self.tick += 1;
        }
    }

    /// Apply a player intent. Returns `Err(Rejected)` if invalid.
    pub fn apply(&mut self, tuning: &Tuning, action: Action) -> Result<(), Rejected> {
        match action {
            Action::StokeCore => {
                self.reactor.stoke(tuning);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> Tuning {
        Tuning::default()
    }

    #[test]
    fn new_game_is_versioned_and_full() {
        let s = GameState::new(&t(), 123);
        assert_eq!(s.version, SAVE_VERSION);
        assert_eq!(s.seed, 123);
        assert_eq!(s.tick, 0);
        assert_eq!(s.reactor.charge, t().reactor_capacity);
    }

    #[test]
    fn advance_accumulates_ticks() {
        let mut s = GameState::new(&t(), 1);
        s.advance(&t(), 5);
        assert_eq!(s.tick, 5);
        assert_eq!(s.reactor.charge, t().reactor_capacity - 5.0);
    }

    #[test]
    fn advance_n_equals_n_times_advance_1() {
        let mut bulk = GameState::new(&t(), 1);
        let mut step = GameState::new(&t(), 1);
        bulk.advance(&t(), 37);
        for _ in 0..37 {
            step.advance(&t(), 1);
        }
        assert_eq!(bulk, step);
    }

    #[test]
    fn stoke_action_raises_charge() {
        let mut s = GameState::new(&t(), 1);
        s.advance(&t(), 30); // charge 70
        s.apply(&t(), Action::StokeCore).unwrap();
        assert_eq!(s.reactor.charge, 80.0);
    }
}
