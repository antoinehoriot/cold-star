//! Data-driven balance. The embedded RON is the single source of M0 tuning;
//! `load()` parses it into `sim::Tuning` and `validate()` guards the values.
//! Later milestones move this to external `.ron` assets and a full registry.

use sim::Tuning;
use sim::origin::OriginDef;

const TUNING_RON: &str = include_str!("../data/tuning.ron");
const ORIGINS_RON: &str = include_str!("../data/origins.ron");

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
    Ok(())
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
}
