//! The reactor power budget. Pure function: given the priority-ordered consumers
//! and the reactor/fuel state, decide generation, which consumers pause under
//! brownout, the new charge, and fuel burned. See spec §2.2.

use crate::priority::Consumer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct PowerConsumer {
    pub consumer: Consumer,
    pub draw: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerOutcome {
    pub generation: f64,
    pub fuel_burned: f64,
    pub new_charge: f64,
    pub paused: BTreeSet<Consumer>,
    /// Total draw (baseline + active consumers). Authoritative for charge math.
    pub active_draw: f64,
    /// Consumer-only draw (excludes baseline). Used to determine overdraw status.
    pub consumer_draw: f64,
}

/// `ordered` must be highest-priority-first.
pub fn resolve_power(
    ordered: &[PowerConsumer],
    max_output: f64,
    charge: f64,
    capacity: f64,
    fuel: f64,
    fuel_per_output: f64,
    baseline_draw: f64,
) -> PowerOutcome {
    let desired_draw: f64 = baseline_draw + ordered.iter().map(|c| c.draw).sum::<f64>();

    // Generate only what is usable (cover draw + refill headroom), limited by the
    // reactor tier ceiling and available fuel.
    let fuel_limited = if fuel_per_output > 0.0 {
        fuel / fuel_per_output
    } else {
        max_output
    };
    let headroom = (capacity - charge).max(0.0);
    let generation = max_output
        .min(fuel_limited)
        .min(desired_draw + headroom)
        .max(0.0);

    let budget = generation + charge;

    // The baseline is mandatory — always active, never paused.
    let mut active_draw = baseline_draw;
    let mut consumer_draw = 0.0;
    let mut paused = BTreeSet::new();
    let mut cut = false;
    for c in ordered {
        if cut || active_draw + c.draw > budget {
            cut = true;
            paused.insert(c.consumer);
        } else {
            active_draw += c.draw;
            consumer_draw += c.draw;
        }
    }

    let new_charge = (charge + generation - active_draw).clamp(0.0, capacity);
    let fuel_burned = generation * fuel_per_output;

    PowerOutcome {
        generation,
        fuel_burned,
        new_charge,
        paused,
        active_draw,
        consumer_draw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobId;

    fn c(id: JobId, draw: f64) -> PowerConsumer {
        PowerConsumer {
            consumer: Consumer::Job(id),
            draw,
        }
    }

    #[test]
    fn surplus_recharges_and_pauses_nothing() {
        // Full reactor (max_output 5), draw 2, charge 50/100.
        let out = resolve_power(&[c(JobId::Refiner, 2.0)], 5.0, 50.0, 100.0, 100.0, 0.5, 0.0);
        assert!(out.paused.is_empty());
        assert!(out.new_charge > 50.0); // surplus recharges
    }

    #[test]
    fn overdraw_drains_charge_but_runs_while_buffer_lasts() {
        // Draw 8 > max_output 5; charge 50 buffers the deficit, nothing pauses yet.
        let out = resolve_power(&[c(JobId::Refiner, 8.0)], 5.0, 50.0, 100.0, 100.0, 0.5, 0.0);
        assert!(out.paused.is_empty());
        assert!(out.new_charge < 50.0); // buffer drained
    }

    #[test]
    fn brownout_pauses_lowest_priority_when_buffer_empty() {
        // charge 0, max_output 5: budget 5. Two consumers of draw 4 each (ordered).
        let out = resolve_power(
            &[c(JobId::Refiner, 4.0), c(JobId::Fabricator, 4.0)],
            5.0,
            0.0,
            100.0,
            100.0,
            0.5,
            0.0,
        );
        assert!(out.paused.contains(&Consumer::Job(JobId::Fabricator)));
        assert!(!out.paused.contains(&Consumer::Job(JobId::Refiner)));
    }

    #[test]
    fn no_fuel_means_no_generation() {
        let out = resolve_power(&[c(JobId::Refiner, 4.0)], 5.0, 0.0, 100.0, 0.0, 0.5, 0.0);
        assert_eq!(out.generation, 0.0);
        // budget 0 -> everything pauses.
        assert!(out.paused.contains(&Consumer::Job(JobId::Refiner)));
    }

    #[test]
    fn baseline_draws_charge_when_unfuelled() {
        // No consumers, no fuel: the mandatory baseline draws the buffer down.
        let out = resolve_power(&[], 5.0, 50.0, 100.0, 0.0, 0.5, 2.0);
        assert_eq!(out.generation, 0.0);
        assert!(out.paused.is_empty());
        assert_eq!(out.new_charge, 48.0); // 50 - baseline 2
        assert_eq!(out.active_draw, 2.0);
    }

    #[test]
    fn baseline_consumes_budget_before_consumers() {
        // budget = gen 5 + charge 0 = 5. baseline 2 is taken first, leaving 3.
        // A single consumer of draw 4 does NOT fit (2+4 > 5) and is paused,
        // even though its draw (4) is below generation (5).
        let out = resolve_power(&[c(JobId::Refiner, 4.0)], 5.0, 0.0, 100.0, 100.0, 0.5, 2.0);
        assert!(out.paused.contains(&Consumer::Job(JobId::Refiner)));
        assert_eq!(out.active_draw, 2.0); // only the baseline ran
    }
}
