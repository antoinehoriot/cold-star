//! The reactor — the heartbeat. A 6-state FSM driven by charge level and
//! overdraw. The `established` latch flips true once charge first reaches the
//! establish threshold and resets only when the core goes Cold; it separates
//! the climbing-from-cold "Igniting" phase from a running core that has dipped
//! low ("Failing"). With overdraw (Draw > generation) the low-charge statuses
//! become "Straining" (above failing) and "Critical" (below failing).

use crate::config::Tuning;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReactorStatus {
    Cold,
    Igniting,
    Stable,
    Failing,
    Straining,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reactor {
    pub charge: f64,
    pub capacity: f64,
    pub status: ReactorStatus,
    pub established: bool,
    pub max_output: f64,
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
            max_output: tuning.reactor_base_max_output,
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

    /// Subtract `amount` from charge (floored at 0) and recompute status. Used for
    /// the Thermal-failure extra drain (the cold creeping back in).
    pub fn drain(&mut self, amount: f64, tuning: &Tuning) {
        self.charge = (self.charge - amount).max(0.0);
        self.update_status_power(false, tuning);
    }

    /// Set charge from the power resolution and recompute status. `overdrawn`
    /// is true when total Draw exceeded generation this tick (charge falling
    /// from overdraw rather than under-fuelling).
    pub fn set_from_power(&mut self, new_charge: f64, overdrawn: bool, tuning: &Tuning) {
        self.charge = new_charge.clamp(0.0, self.capacity);
        self.update_status_power(overdrawn, tuning);
    }

    fn update_status(&mut self, tuning: &Tuning) {
        self.update_status_power(false, tuning);
    }

    fn update_status_power(&mut self, overdrawn: bool, tuning: &Tuning) {
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
        } else if overdrawn {
            if self.charge < failing {
                ReactorStatus::Critical
            } else {
                ReactorStatus::Straining
            }
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

    #[test]
    fn overdraw_above_failing_is_straining() {
        let tn = t();
        let mut r = Reactor::new(&tn); // 100, established
        r.set_from_power(80.0, true, &tn);
        assert_eq!(r.status, ReactorStatus::Straining);
    }

    #[test]
    fn overdraw_below_failing_is_critical() {
        let tn = t();
        let mut r = Reactor::new(&tn);
        r.set_from_power(10.0, true, &tn); // below failing (20)
        assert_eq!(r.status, ReactorStatus::Critical);
    }

    #[test]
    fn under_fuelled_low_charge_is_failing_not_straining() {
        let tn = t();
        let mut r = Reactor::new(&tn);
        r.set_from_power(10.0, false, &tn);
        assert_eq!(r.status, ReactorStatus::Failing);
    }

    #[test]
    fn new_reactor_has_base_max_output() {
        let r = Reactor::new(&t());
        assert_eq!(r.max_output, t().reactor_base_max_output);
    }

    #[test]
    fn drain_lowers_charge_and_can_cold() {
        let tn = t();
        let mut r = Reactor::new(&tn);
        r.drain(40.0, &tn);
        assert_eq!(r.charge, 60.0);
        r.drain(60.0, &tn);
        assert_eq!(r.status, ReactorStatus::Cold);
    }
}
