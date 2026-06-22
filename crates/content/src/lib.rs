//! Data-driven balance. The embedded RON is the single source of M0 tuning;
//! `load()` parses it into `sim::Tuning` and `validate()` guards the values.
//! Later milestones move this to external `.ron` assets and a full registry.

use sim::Tuning;
use sim::origin::OriginDef;

const TUNING_RON: &str = include_str!("../data/tuning.ron");
const ORIGINS_RON: &str = include_str!("../data/origins.ron");
const JOBS_RON: &str = include_str!("../data/jobs.ron");
const MODULES_RON: &str = include_str!("../data/modules.ron");

#[derive(Debug)]
pub enum ContentError {
    Parse(String),
    Invalid(String),
}

pub fn load() -> Result<Tuning, ContentError> {
    let tuning: Tuning =
        ron::from_str(TUNING_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    validate(&tuning)?;
    Ok(tuning)
}

pub fn validate(t: &Tuning) -> Result<(), ContentError> {
    fn finite_pos(v: f64) -> bool {
        v.is_finite() && v > 0.0
    }
    fn finite_nonneg(v: f64) -> bool {
        v.is_finite() && v >= 0.0
    }
    if !finite_pos(t.reactor_capacity) {
        return Err(ContentError::Invalid(
            "reactor_capacity must be finite > 0".into(),
        ));
    }
    if !finite_nonneg(t.reactor_drain_per_tick) {
        return Err(ContentError::Invalid(
            "reactor_drain_per_tick must be finite >= 0".into(),
        ));
    }
    for (name, frac) in [
        (
            "reactor_establish_threshold_frac",
            t.reactor_establish_threshold_frac,
        ),
        (
            "reactor_failing_threshold_frac",
            t.reactor_failing_threshold_frac,
        ),
    ] {
        if !frac.is_finite() || frac <= 0.0 || frac > 1.0 {
            return Err(ContentError::Invalid(format!(
                "{name} must be finite in (0, 1]"
            )));
        }
    }
    if t.reactor_failing_threshold_frac > t.reactor_establish_threshold_frac {
        return Err(ContentError::Invalid(
            "reactor_failing_threshold_frac must be <= reactor_establish_threshold_frac".into(),
        ));
    }
    for (name, v) in [
        ("reactor_ignition_seed", t.reactor_ignition_seed),
        ("reactor_baseline_draw", t.reactor_baseline_draw),
        ("feed_cost_scrap", t.feed_cost_scrap),
        ("feed_charge", t.feed_charge),
        ("salvage_yield", t.salvage_yield),
        ("crew_source_cost", t.crew_source_cost),
        ("crew_trickle", t.crew_trickle),
    ] {
        if !finite_nonneg(v) {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    if t.crew_cap < 1 {
        return Err(ContentError::Invalid("crew_cap must be >= 1".into()));
    }
    if t.arrival_interval < 1 {
        return Err(ContentError::Invalid(
            "arrival_interval must be >= 1".into(),
        ));
    }
    if t.log_capacity < 1 {
        return Err(ContentError::Invalid("log_capacity must be >= 1".into()));
    }
    for (name, v) in [
        ("fuel_per_output", t.fuel_per_output),
        ("reactor_base_max_output", t.reactor_base_max_output),
        ("resource_base_cap", t.resource_base_cap),
        ("crew_ration_upkeep", t.crew_ration_upkeep),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    if !(t.starving_efficiency.is_finite()
        && t.starving_efficiency > 0.0
        && t.starving_efficiency <= 1.0)
    {
        return Err(ContentError::Invalid(
            "starving_efficiency must be finite in (0, 1]".into(),
        ));
    }
    if t.base_housing < 1 {
        return Err(ContentError::Invalid("base_housing must be >= 1".into()));
    }
    Ok(())
}

use sim::job::{JobId, JobRegistry};
use sim::module::{ModuleId, ModuleRegistry};

pub fn load_jobs() -> Result<JobRegistry, ContentError> {
    let jobs: Vec<sim::job::Job> =
        ron::from_str(JOBS_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    let reg = JobRegistry(jobs);
    for want in [
        JobId::Salvager,
        JobId::Refiner,
        JobId::Fabricator,
        JobId::Assembler,
        JobId::Hydroponicist,
        JobId::Scientist,
    ] {
        if reg.get(want).is_none() {
            return Err(ContentError::Invalid(format!("jobs.ron missing {want:?}")));
        }
    }
    for j in reg.iter() {
        if !j.power_draw.is_finite() || j.power_draw < 0.0 {
            return Err(ContentError::Invalid(format!(
                "{:?}: power_draw must be finite >= 0",
                j.id
            )));
        }
    }
    Ok(reg)
}

pub fn load_modules() -> Result<ModuleRegistry, ContentError> {
    let mods: Vec<sim::module::Module> =
        ron::from_str(MODULES_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    let reg = ModuleRegistry(mods);
    for want in [
        ModuleId::Refinery,
        ModuleId::Fabricator,
        ModuleId::Assembler,
        ModuleId::Hydroponics,
        ModuleId::Lab,
        ModuleId::Quarters,
        ModuleId::StorageHold,
        ModuleId::LifeSupport,
        ModuleId::ReactorUpgrade,
    ] {
        if reg.get(want).is_none() {
            return Err(ContentError::Invalid(format!(
                "modules.ron missing {want:?}"
            )));
        }
    }
    for m in reg.iter() {
        if m.cost.iter().any(|&(_, a)| !a.is_finite() || a < 0.0) {
            return Err(ContentError::Invalid(format!(
                "{:?}: cost amounts must be finite >= 0",
                m.id
            )));
        }
        if !(m.cost_scaling.is_finite() && m.cost_scaling >= 1.0) {
            return Err(ContentError::Invalid(format!(
                "{:?}: cost_scaling must be finite >= 1",
                m.id
            )));
        }
    }
    Ok(reg)
}

pub fn load_origins() -> Result<Vec<OriginDef>, ContentError> {
    let origins: Vec<OriginDef> =
        ron::from_str(ORIGINS_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    if origins.is_empty() {
        return Err(ContentError::Invalid("no origins defined".into()));
    }
    for o in &origins {
        if !(o.start.reactor_capacity.is_finite() && o.start.reactor_capacity > 0.0) {
            return Err(ContentError::Invalid(format!(
                "{:?}: reactor_capacity must be finite > 0",
                o.id
            )));
        }
        if !o.start.reactor_charge.is_finite()
            || o.start.reactor_charge < 0.0
            || o.start.reactor_charge > o.start.reactor_capacity
        {
            return Err(ContentError::Invalid(format!(
                "{:?}: reactor_charge must be in [0, capacity]",
                o.id
            )));
        }
        if !o.start.scrap.is_finite() || o.start.scrap < 0.0 {
            return Err(ContentError::Invalid(format!(
                "{:?}: scrap must be finite >= 0",
                o.id
            )));
        }
    }
    Ok(origins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_jobs_and_modules() {
        let jobs = load_jobs().unwrap();
        assert_eq!(jobs.0.len(), 6);
        // Salvager is ungated and produces Scrap with zero draw.
        let salv = jobs.get(sim::job::JobId::Salvager).unwrap();
        assert!(salv.gated_by.is_none());
        assert_eq!(salv.power_draw, 0.0);

        let mods = load_modules().unwrap();
        assert_eq!(mods.0.len(), 9);
        // Lab is tier 2 but is NOT Data-gated (it unlocks the Scientist that
        // PRODUCES Data — gating it on Data would deadlock).
        let lab = mods.get(sim::module::ModuleId::Lab).unwrap();
        assert_eq!(lab.tier, 2);
        assert!(
            !lab.cost
                .iter()
                .any(|&(k, _)| k == sim::resources::ResourceKind::Data)
        );
        // The Assembler is the real Data sink (tier 2, costs Data).
        let asm = mods.get(sim::module::ModuleId::Assembler).unwrap();
        assert_eq!(asm.tier, 2);
        assert!(
            asm.cost
                .iter()
                .any(|&(k, _)| k == sim::resources::ResourceKind::Data)
        );
    }

    #[test]
    fn loads_three_origins() {
        let origins = load_origins().unwrap();
        assert_eq!(origins.len(), 3);
        let ids: Vec<_> = origins.iter().map(|o| o.id).collect();
        assert!(ids.contains(&sim::origin::Origin::Drifter));
        assert!(ids.contains(&sim::origin::Origin::Hollow));
        assert!(ids.contains(&sim::origin::Origin::Seed));
        // Every origin resolves the flavor keys the engine logs on.
        for o in &origins {
            for key in [
                "ignite",
                "stable",
                "source_active",
                "crew_arrived",
                "failing",
                "cold",
            ] {
                assert!(!o.flavor(key).is_empty(), "{:?} missing flavor {key}", o.id);
            }
            assert!(o.start.reactor_capacity > 0.0);
            assert!(
                o.start.reactor_charge >= 0.0 && o.start.reactor_charge <= o.start.reactor_capacity
            );
        }
    }

    #[test]
    fn loads_expanded_m1_tuning() {
        let t = load().unwrap();
        assert_eq!(t.reactor_capacity, 100.0);
        assert_eq!(t.reactor_drain_per_tick, 1.0);
        assert_eq!(t.reactor_establish_threshold_frac, 0.5);
        assert_eq!(t.reactor_failing_threshold_frac, 0.2);
        assert_eq!(t.reactor_ignition_seed, 10.0);
        assert_eq!(t.feed_cost_scrap, 10.0);
        assert_eq!(t.feed_charge, 20.0);
        assert_eq!(t.salvage_yield, 5.0);
        assert_eq!(t.salvage_cooldown_ticks, 5);
        assert_eq!(t.crew_source_cost, 30.0);
        assert_eq!(t.crew_cap, 4);
        assert_eq!(t.arrival_interval, 30);
        assert_eq!(t.crew_trickle, 0.5);
        assert_eq!(t.log_capacity, 100);
    }

    #[test]
    fn rejects_nonpositive_capacity() {
        let bad = Tuning {
            reactor_capacity: 0.0,
            ..Tuning::default()
        };
        assert!(matches!(validate(&bad), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn rejects_negative_drain() {
        let bad = Tuning {
            reactor_drain_per_tick: -1.0,
            ..Tuning::default()
        };
        assert!(matches!(validate(&bad), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn rejects_nan_capacity() {
        let bad = Tuning {
            reactor_capacity: f64::NAN,
            ..Tuning::default()
        };
        assert!(matches!(validate(&bad), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn loads_m2_tuning() {
        let t = load().unwrap();
        assert_eq!(t.fuel_per_output, 0.5);
        assert_eq!(t.reactor_base_max_output, 5.0);
        assert_eq!(t.resource_base_cap, 100.0);
        assert_eq!(t.crew_ration_upkeep, 0.1);
        assert_eq!(t.starving_efficiency, 0.5);
        assert_eq!(t.base_housing, 4);
        assert_eq!(t.reactor_baseline_draw, 1.0);
    }
}
