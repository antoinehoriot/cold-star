//! Crew population + job assignments. M2: crew sit in an idle pool and are
//! assigned to jobs; arrivals (from the crew-source) are capped by housing.

use crate::config::Tuning;
use crate::job::JobId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Crew {
    pub total: u32,
    pub ticks_until_next: u32,
    pub assignments: BTreeMap<JobId, u32>,
    pub away: u32,
}

impl Crew {
    pub fn begin_arrivals(&mut self, tuning: &Tuning) {
        self.ticks_until_next = tuning.arrival_interval;
    }

    /// Advance the arrival timer one tick. Returns true iff a crew arrived.
    /// Arrivals stop at the `housing` cap.
    pub fn tick_arrival(&mut self, housing: u32, tuning: &Tuning) -> bool {
        if self.total >= housing {
            return false;
        }
        if self.ticks_until_next > 0 {
            self.ticks_until_next -= 1;
        }
        if self.ticks_until_next == 0 {
            self.total += 1;
            self.ticks_until_next = tuning.arrival_interval;
            true
        } else {
            false
        }
    }

    pub fn assigned(&self, job: JobId) -> u32 {
        self.assignments.get(&job).copied().unwrap_or(0)
    }

    /// Station crew available for jobs/upkeep (excludes the away-team).
    pub fn at_station(&self) -> u32 {
        self.total.saturating_sub(self.away)
    }

    pub fn idle(&self) -> u32 {
        let assigned: u32 = self.assignments.values().sum();
        self.total.saturating_sub(assigned + self.away)
    }

    /// Commit up to `n` idle crew to the away-team; returns how many committed.
    pub fn commit_away(&mut self, n: u32) -> u32 {
        let take = n.min(self.idle());
        self.away += take;
        take
    }

    /// Release `n` away crew back to the station idle pool.
    pub fn release_away(&mut self, n: u32) {
        self.away = self.away.saturating_sub(n);
    }

    /// Kill one away crew (combat/starvation): shrinks both away and total.
    pub fn kill_away(&mut self) -> bool {
        if self.away == 0 || self.total == 0 {
            return false;
        }
        self.away -= 1;
        self.total -= 1;
        true
    }

    /// Assign (`delta > 0`) or unassign (`delta < 0`) crew to `job`. Bounded by
    /// idle crew (can't assign more than idle) and 0 (can't unassign below 0).
    /// Returns true if any change was applied.
    pub fn assign(&mut self, job: JobId, delta: i32) -> bool {
        let current = self.assigned(job);
        let change = if delta > 0 {
            (delta as u32).min(self.idle())
        } else if delta < 0 {
            // clamp the removal to the current count
            return if current == 0 {
                false
            } else {
                let removed = ((-delta) as u32).min(current);
                self.set(job, current - removed);
                removed > 0
            };
        } else {
            0
        };
        if change == 0 {
            return false;
        }
        self.set(job, current + change);
        true
    }

    /// Remove one crew member. Idle crew go first; otherwise the lowest-JobId
    /// assigned job loses one. Returns false if there is no station crew at all.
    pub fn lose_one(&mut self) -> bool {
        if self.at_station() == 0 {
            return false;
        }
        if self.idle() == 0 {
            // All assigned: take from the lowest JobId (BTreeMap ordered keys).
            if let Some((&job, _)) = self.assignments.iter().find(|&(_, &n)| n > 0) {
                let cur = self.assigned(job);
                self.set(job, cur - 1);
            }
        }
        self.total -= 1;
        true
    }

    fn set(&mut self, job: JobId, count: u32) {
        if count == 0 {
            self.assignments.remove(&job);
        } else {
            self.assignments.insert(job, count);
        }
    }

    /// Total Rations consumed by station crew upkeep this tick.
    /// Away crew eat ship Rations billed separately (Task 5).
    pub fn ration_upkeep(&self, tuning: &Tuning) -> f64 {
        self.at_station() as f64 * tuning.crew_ration_upkeep
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> Tuning {
        Tuning::default()
    }

    #[test]
    fn arrivals_capped_by_housing() {
        let tn = t();
        let mut c = Crew::default();
        c.begin_arrivals(&tn);
        for _ in 0..(tn.arrival_interval * 10) {
            c.tick_arrival(3, &tn);
        }
        assert_eq!(c.total, 3);
    }

    #[test]
    fn assign_bounded_by_idle() {
        let mut c = Crew {
            total: 3,
            ..Crew::default()
        };
        assert!(c.assign(JobId::Salvager, 5)); // only 3 idle
        assert_eq!(c.assigned(JobId::Salvager), 3);
        assert_eq!(c.idle(), 0);
        assert!(!c.assign(JobId::Refiner, 1)); // none idle
    }

    #[test]
    fn unassign_returns_crew_to_idle() {
        let mut c = Crew {
            total: 3,
            ..Crew::default()
        };
        c.assign(JobId::Salvager, 3);
        assert!(c.assign(JobId::Salvager, -2));
        assert_eq!(c.assigned(JobId::Salvager), 1);
        assert_eq!(c.idle(), 2);
    }

    #[test]
    fn lose_one_takes_idle_then_assigned() {
        let mut c = Crew {
            total: 2,
            ..Crew::default()
        };
        c.assign(JobId::Salvager, 1); // 1 assigned, 1 idle
        assert!(c.lose_one()); // idle removed
        assert_eq!(c.total, 1);
        assert_eq!(c.assigned(JobId::Salvager), 1);
        assert!(c.lose_one()); // now from assigned
        assert_eq!(c.total, 0);
        assert_eq!(c.assigned(JobId::Salvager), 0);
        assert!(!c.lose_one());
    }

    #[test]
    fn ration_upkeep_scales_with_station_crew() {
        let tn = t();
        let c = Crew {
            total: 5,
            away: 2,
            ..Crew::default()
        };
        // Only the 3 at-station crew consume station Rations; away crew eat ship Rations.
        assert_eq!(c.ration_upkeep(&tn), 3.0 * tn.crew_ration_upkeep);
    }
}
