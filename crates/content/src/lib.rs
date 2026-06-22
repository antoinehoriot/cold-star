//! Data-driven balance. The embedded RON is the single source of M0 tuning;
//! `load()` parses it into `sim::Tuning` and `validate()` guards the values.
//! Later milestones move this to external `.ron` assets and a full registry.

use sim::Tuning;

const TUNING_RON: &str = include_str!("../data/tuning.ron");

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
    if !t.reactor_capacity.is_finite() || t.reactor_capacity <= 0.0 {
        return Err(ContentError::Invalid(
            "reactor_capacity must be a finite value > 0".into(),
        ));
    }
    if !t.reactor_drain_per_tick.is_finite() || t.reactor_drain_per_tick < 0.0 {
        return Err(ContentError::Invalid(
            "reactor_drain_per_tick must be a finite value >= 0".into(),
        ));
    }
    if !t.reactor_stoke_amount.is_finite() || t.reactor_stoke_amount < 0.0 {
        return Err(ContentError::Invalid(
            "reactor_stoke_amount must be a finite value >= 0".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_tuning() {
        let t = load().unwrap();
        assert_eq!(t.reactor_capacity, 100.0);
        assert_eq!(t.reactor_drain_per_tick, 1.0);
        assert_eq!(t.reactor_stoke_amount, 10.0);
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
    fn rejects_negative_stoke_amount() {
        let bad = Tuning {
            reactor_stoke_amount: -1.0,
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
