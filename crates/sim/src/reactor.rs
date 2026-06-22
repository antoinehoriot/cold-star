//! The reactor — the heartbeat. Charge decays each tick; the player feeds it.

use crate::config::Tuning;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReactorStatus {
    Cold,
    Igniting,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reactor {
    pub charge: f64,
    pub capacity: f64,
    pub status: ReactorStatus,
}

impl Reactor {
    pub fn new(tuning: &Tuning) -> Self {
        Self {
            charge: tuning.reactor_capacity,
            capacity: tuning.reactor_capacity,
            status: ReactorStatus::Stable,
        }
    }

    /// One tick of decay.
    pub fn tick(&mut self, tuning: &Tuning) {
        self.charge = (self.charge - tuning.reactor_drain_per_tick).max(0.0);
        self.update_status();
    }

    /// Player feeds the core.
    pub fn stoke(&mut self, tuning: &Tuning) {
        self.charge = (self.charge + tuning.reactor_stoke_amount).min(self.capacity);
        self.update_status();
    }

    fn update_status(&mut self) {
        let frac = if self.capacity > 0.0 {
            self.charge / self.capacity
        } else {
            0.0
        };
        self.status = if self.charge <= 0.0 {
            ReactorStatus::Cold
        } else if frac < 0.25 {
            ReactorStatus::Igniting
        } else {
            ReactorStatus::Stable
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> Tuning {
        Tuning {
            reactor_drain_per_tick: 1.0,
            reactor_stoke_amount: 10.0,
            reactor_capacity: 100.0,
            ..Tuning::default()
        }
    }

    #[test]
    fn starts_full_and_stable() {
        let r = Reactor::new(&tuning());
        assert_eq!(r.charge, 100.0);
        assert_eq!(r.status, ReactorStatus::Stable);
    }

    #[test]
    fn tick_drains_charge() {
        let t = tuning();
        let mut r = Reactor::new(&t);
        r.tick(&t);
        assert_eq!(r.charge, 99.0);
    }

    #[test]
    fn drains_to_cold_and_clamps_at_zero() {
        let t = tuning();
        let mut r = Reactor::new(&t);
        for _ in 0..200 {
            r.tick(&t);
        }
        assert_eq!(r.charge, 0.0);
        assert_eq!(r.status, ReactorStatus::Cold);
    }

    #[test]
    fn igniting_when_low() {
        let t = tuning();
        let mut r = Reactor::new(&t);
        for _ in 0..80 {
            r.tick(&t);
        }
        assert_eq!(r.charge, 20.0);
        assert_eq!(r.status, ReactorStatus::Igniting);
    }

    #[test]
    fn stoke_raises_and_caps_at_capacity() {
        let t = tuning();
        let mut r = Reactor::new(&t);
        for _ in 0..50 {
            r.tick(&t);
        } // charge 50
        r.stoke(&t); // +10 -> 60
        assert_eq!(r.charge, 60.0);
        for _ in 0..10 {
            r.stoke(&t);
        } // would exceed capacity
        assert_eq!(r.charge, 100.0);
    }
}
