//! Crew population. In M1 crew arrive on a timer once the source is active and
//! passively auto-salvage a Scrap trickle. No assignment/upkeep yet (that is M2).

use crate::config::Tuning;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Crew {
    pub count: u32,
    pub ticks_until_next: u32,
}

impl Crew {
    /// Start the arrival timer (call when the crew-source is activated).
    pub fn begin_arrivals(&mut self, tuning: &Tuning) {
        self.ticks_until_next = tuning.arrival_interval;
    }

    /// Advance the arrival timer one tick. Returns true iff a crew arrived.
    /// Caller must only invoke this while the crew-source is active.
    pub fn tick_arrival(&mut self, tuning: &Tuning) -> bool {
        if self.count >= tuning.crew_cap {
            return false;
        }
        if self.ticks_until_next > 0 {
            self.ticks_until_next -= 1;
        }
        if self.ticks_until_next == 0 {
            self.count += 1;
            self.ticks_until_next = tuning.arrival_interval;
            true
        } else {
            false
        }
    }

    /// Scrap auto-salvaged this tick across all crew.
    pub fn trickle(&self, tuning: &Tuning) -> f64 {
        self.count as f64 * tuning.crew_trickle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> Tuning {
        Tuning::default() // arrival_interval 30, crew_cap 4, crew_trickle 0.5
    }

    #[test]
    fn arrives_after_interval_up_to_cap() {
        let tn = t();
        let mut c = Crew::default();
        c.begin_arrivals(&tn);
        let mut arrivals = 0;
        for _ in 0..(tn.arrival_interval * tn.crew_cap + tn.arrival_interval) {
            if c.tick_arrival(&tn) {
                arrivals += 1;
            }
        }
        assert_eq!(c.count, tn.crew_cap);
        assert_eq!(arrivals, tn.crew_cap); // never exceeds cap
    }

    #[test]
    fn first_arrival_lands_exactly_on_interval() {
        let tn = t();
        let mut c = Crew::default();
        c.begin_arrivals(&tn);
        for _ in 0..(tn.arrival_interval - 1) {
            assert!(!c.tick_arrival(&tn));
        }
        assert!(c.tick_arrival(&tn)); // the interval-th tick
        assert_eq!(c.count, 1);
    }

    #[test]
    fn trickle_scales_with_count() {
        let tn = t();
        let mut c = Crew::default();
        c.count = 3;
        assert_eq!(c.trickle(&tn), 3.0 * tn.crew_trickle);
    }
}
