//! Two-phase per-tick job execution. Inputs are read/consumed against the live
//! stock in deterministic JobId order (so scarce inputs go to the earlier job);
//! outputs are added after all consumption, so a job's output never feeds another
//! job in the same tick (resources advance one stage per tick). See spec §2.3.

use crate::job::{Job, JobId, JobRegistry};
use crate::priority::Consumer;
use crate::resources::Resources;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EconomyReport {
    pub ran: BTreeSet<JobId>,
    pub starved_input: BTreeSet<JobId>,
    pub overflow: bool,
}

/// A job is input-ready if the live stock covers its full assigned inputs.
pub fn job_ready(job: &Job, crew: u32, res: &Resources) -> bool {
    if crew == 0 {
        return false;
    }
    job.recipe
        .inputs
        .iter()
        .all(|&(k, amt)| res.amount(k) >= amt * crew as f64)
}

/// Execute all assigned jobs for one tick. `paused` jobs (powered-off) and
/// input-starved jobs produce nothing. Outputs are scaled by `efficiency` and
/// an optional per-job multiplier from `output_mult` (e.g. `JobOutputMult` grants).
pub fn run_economy(
    assignments: &BTreeMap<JobId, u32>,
    jobs: &JobRegistry,
    paused: &BTreeSet<Consumer>,
    efficiency: f64,
    output_mult: &BTreeMap<JobId, f64>,
    res: &mut Resources,
) -> EconomyReport {
    let mut report = EconomyReport::default();
    // Phase A — consume (deterministic JobId order via BTreeMap iteration).
    let mut to_produce: Vec<(JobId, u32)> = Vec::new();
    for (&id, &crew) in assignments {
        if crew == 0 {
            continue;
        }
        let Some(job) = jobs.get(id) else { continue };
        if paused.contains(&Consumer::Job(id)) {
            continue; // powered off this tick
        }
        if !job_ready(job, crew, res) {
            report.starved_input.insert(id);
            continue;
        }
        for &(k, amt) in &job.recipe.inputs {
            res.try_spend(k, amt * crew as f64);
        }
        to_produce.push((id, crew));
    }
    // Phase B — produce.
    for (id, crew) in to_produce {
        let job = jobs.get(id).expect("job existed in phase A");
        let m = output_mult.get(&id).copied().unwrap_or(1.0);
        for &(k, amt) in &job.recipe.outputs {
            let overflow = res.add(k, amt * crew as f64 * efficiency * m);
            if overflow > 0.0 {
                report.overflow = true;
            }
        }
        report.ran.insert(id);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Recipe;
    use crate::resources::ResourceKind;

    fn registry() -> JobRegistry {
        JobRegistry(vec![
            Job {
                id: JobId::Salvager,
                name: "Salvager".into(),
                gated_by: None,
                recipe: Recipe {
                    inputs: vec![],
                    outputs: vec![(ResourceKind::Scrap, 1.0)],
                },
                power_draw: 0.0,
                default_priority: 6,
            },
            Job {
                id: JobId::Refiner,
                name: "Refiner".into(),
                gated_by: None,
                recipe: Recipe {
                    inputs: vec![(ResourceKind::Scrap, 2.0)],
                    outputs: vec![(ResourceKind::Fuel, 1.0)],
                },
                power_draw: 2.0,
                default_priority: 2,
            },
        ])
    }

    fn assigns(pairs: &[(JobId, u32)]) -> BTreeMap<JobId, u32> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn salvager_produces_scrap() {
        let mut res = Resources::default();
        let r = run_economy(
            &assigns(&[(JobId::Salvager, 3)]),
            &registry(),
            &BTreeSet::new(),
            1.0,
            &BTreeMap::new(),
            &mut res,
        );
        assert_eq!(res.amount(ResourceKind::Scrap), 3.0);
        assert!(r.ran.contains(&JobId::Salvager));
    }

    #[test]
    fn refiner_consumes_scrap_and_makes_fuel() {
        let mut res = Resources::default();
        res.add(ResourceKind::Scrap, 10.0);
        run_economy(
            &assigns(&[(JobId::Refiner, 2)]),
            &registry(),
            &BTreeSet::new(),
            1.0,
            &BTreeMap::new(),
            &mut res,
        );
        assert_eq!(res.amount(ResourceKind::Scrap), 6.0); // 2 crew × 2 scrap
        assert_eq!(res.amount(ResourceKind::Fuel), 2.0);
    }

    #[test]
    fn input_starved_job_produces_nothing() {
        let mut res = Resources::default();
        res.add(ResourceKind::Scrap, 1.0); // not enough for 1 Refiner (needs 2)
        let r = run_economy(
            &assigns(&[(JobId::Refiner, 1)]),
            &registry(),
            &BTreeSet::new(),
            1.0,
            &BTreeMap::new(),
            &mut res,
        );
        assert_eq!(res.amount(ResourceKind::Fuel), 0.0);
        assert_eq!(res.amount(ResourceKind::Scrap), 1.0);
        assert!(r.starved_input.contains(&JobId::Refiner));
    }

    #[test]
    fn paused_job_does_not_run() {
        let mut res = Resources::default();
        res.add(ResourceKind::Scrap, 10.0);
        let mut paused = BTreeSet::new();
        paused.insert(Consumer::Job(JobId::Refiner));
        let r = run_economy(
            &assigns(&[(JobId::Refiner, 1)]),
            &registry(),
            &paused,
            1.0,
            &BTreeMap::new(),
            &mut res,
        );
        assert_eq!(res.amount(ResourceKind::Fuel), 0.0);
        assert_eq!(res.amount(ResourceKind::Scrap), 10.0);
        assert!(!r.ran.contains(&JobId::Refiner));
    }

    #[test]
    fn no_intra_tick_chaining() {
        // Salvager makes Scrap; Refiner needs Scrap. With 0 starting scrap, the
        // Refiner is starved THIS tick even though the Salvager produces scrap.
        let mut res = Resources::default();
        let r = run_economy(
            &assigns(&[(JobId::Salvager, 5), (JobId::Refiner, 1)]),
            &registry(),
            &BTreeSet::new(),
            1.0,
            &BTreeMap::new(),
            &mut res,
        );
        assert_eq!(res.amount(ResourceKind::Scrap), 5.0); // salvager output only
        assert_eq!(res.amount(ResourceKind::Fuel), 0.0); // refiner starved
        assert!(r.starved_input.contains(&JobId::Refiner));
    }

    #[test]
    fn efficiency_scales_output() {
        let mut res = Resources::default();
        let r = run_economy(
            &assigns(&[(JobId::Salvager, 4)]),
            &registry(),
            &BTreeSet::new(),
            0.5,
            &BTreeMap::new(),
            &mut res,
        );
        assert_eq!(res.amount(ResourceKind::Scrap), 2.0); // 4 × 1 × 0.5
        assert!(!r.overflow);
    }

    #[test]
    fn output_mult_scales_per_job() {
        let mut res = Resources::default();
        let mut mult = BTreeMap::new();
        mult.insert(JobId::Salvager, 2.0);
        run_economy(
            &assigns(&[(JobId::Salvager, 1)]),
            &registry(),
            &BTreeSet::new(),
            1.0,
            &mult,
            &mut res,
        );
        assert_eq!(res.amount(ResourceKind::Scrap), 2.0); // 1 crew x 1 x eff 1.0 x mult 2.0
    }
}
