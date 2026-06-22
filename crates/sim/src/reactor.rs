//! The reactor — the heartbeat. A 4-state FSM driven by charge level. The
//! `established` latch flips true once charge first reaches the establish
//! threshold and resets only when the core goes Cold; it separates the
//! climbing-from-cold "Igniting" phase from a running core that has dipped low
//! ("Failing"). No Draw budget in M1 — Straining/Critical arrive in M2.

use crate::config::Tuning;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReactorStatus {
    Cold,
    Igniting,
    Stable,
    Failing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reactor {
    pub charge: f64,
    pub capacity: f64,
    pub status: ReactorStatus,
    pub established: bool,
}

impl Reactor {
    /// A full, established core (default/back-compat starting state).
    pub fn new(tuning: &Tuning) -> Self {
        Self::with_charge(tuning.reactor_capacity, tuning.reactor_capacity, tuning)
    }

    /// A core with an explicit capacity and starting charge (per-origin starts).
    pub fn with_charge(capacity: f64, charge: f64, tuning: &Tuning) -> Self {
        let mut r = Self {
            charge,
            capacity,
            status: ReactorStatus::Cold,
            established: false,
        };
        r.update_status(tuning);
        r
    }

    /// One tick of decay.
    pub fn tick(&mut self, tuning: &Tuning) {
        self.charge = (self.charge - tuning.reactor_drain_per_tick).max(0.0);
        self.update_status(tuning);
    }

    /// Ignite a Cold core: seed it with a small charge.
    pub fn ignite(&mut self, tuning: &Tuning) {
        self.charge = tuning.reactor_ignition_seed.min(self.capacity);
        self.update_status(tuning);
    }

    /// Add charge (from feeding), capped at capacity.
    pub fn feed(&mut self, amount: f64, tuning: &Tuning) {
        self.charge = (self.charge + amount).min(self.capacity);
        self.update_status(tuning);
    }

    /// Legacy M0 stoke (kept until Task 6 removes the M0 action path).
    pub fn stoke(&mut self, tuning: &Tuning) {
        self.charge = (self.charge + tuning.reactor_stoke_amount).min(self.capacity);
        self.update_status(tuning);
    }

    fn update_status(&mut self, tuning: &Tuning) {
        if self.charge <= 0.0 {
            self.status = ReactorStatus::Cold;
            self.established = false;
            return;
        }
        let establish = self.capacity * tuning.reactor_establish_threshold_frac;
        let failing = self.capacity * tuning.reactor_failing_threshold_frac;
        if self.charge >= establish {
            self.established = true;
        }
        self.status = if !self.established {
            ReactorStatus::Igniting
        } else if self.charge < failing {
            ReactorStatus::Failing
        } else {
            ReactorStatus::Stable
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> Tuning {
        Tuning::default() // capacity 100, drain 1, establish .5, failing .2, ignition 10
    }

    #[test]
    fn full_core_is_established_and_stable() {
        let r = Reactor::new(&t());
        assert_eq!(r.charge, 100.0);
        assert!(r.established);
        assert_eq!(r.status, ReactorStatus::Stable);
    }

    #[test]
    fn cold_core_ignites_to_igniting() {
        let tn = t();
        let mut r = Reactor::with_charge(60.0, 0.0, &tn);
        assert_eq!(r.status, ReactorStatus::Cold);
        assert!(!r.established);
        r.ignite(&tn); // charge -> 10, below establish (30) -> Igniting
        assert_eq!(r.charge, 10.0);
        assert_eq!(r.status, ReactorStatus::Igniting);
        assert!(!r.established);
    }

    #[test]
    fn feeding_past_establish_becomes_stable() {
        let tn = t();
        let mut r = Reactor::with_charge(100.0, 0.0, &tn);
        r.ignite(&tn); // 10, Igniting
        r.feed(45.0, &tn); // 55 >= establish 50 -> established, Stable
        assert!(r.established);
        assert_eq!(r.status, ReactorStatus::Stable);
    }

    #[test]
    fn established_core_dips_to_failing_then_cold() {
        let tn = t();
        let mut r = Reactor::new(&tn); // 100, established, Stable
        for _ in 0..81 {
            r.tick(&tn);
        } // charge 19 < failing 20
        assert_eq!(r.status, ReactorStatus::Failing);
        assert!(r.established);
        for _ in 0..19 {
            r.tick(&tn);
        } // charge 0
        assert_eq!(r.status, ReactorStatus::Cold);
        assert!(!r.established); // latch reset on Cold
    }

    #[test]
    fn failing_recovers_to_stable_on_feed() {
        let tn = t();
        let mut r = Reactor::new(&tn);
        for _ in 0..85 {
            r.tick(&tn);
        } // 15, Failing
        assert_eq!(r.status, ReactorStatus::Failing);
        r.feed(40.0, &tn); // 55 -> Stable
        assert_eq!(r.status, ReactorStatus::Stable);
    }
}
