//! Origins: the three starting scenarios. An `OriginDef` is a data record
//! (loaded from RON by `content`) that configures the initial game state and
//! supplies reskin labels + flavor text. M1 origins share all mechanics.

use crate::config::Tuning;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    Drifter,
    Hollow,
    Seed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OriginStart {
    pub scrap: f64,
    pub reactor_capacity: f64,
    pub reactor_charge: f64,
}

/// NOTE: per-origin tuning overrides (spec §5 `tuning_overrides`) are deferred to
/// M2 — all M1 origins share base tuning, differing only by labels + `start`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OriginDef {
    pub id: Origin,
    pub name: String,
    pub intro: String,
    pub crew_noun: String,
    pub source_name: String,
    pub flavor: BTreeMap<String, String>,
    pub start: OriginStart,
}

impl OriginDef {
    /// Flavor log line for `key`, or `""` if absent.
    pub fn flavor(&self, key: &str) -> &str {
        self.flavor.get(key).map(String::as_str).unwrap_or("")
    }
}

/// A bare default origin (Drifter, full charge) used by `GameState::new` for
/// tests and the no-origin fallback. The real game uses `content`'s origins.
pub fn default_origin_def(tuning: &Tuning) -> OriginDef {
    OriginDef {
        id: Origin::Drifter,
        name: "Drifter".into(),
        intro: String::new(),
        crew_noun: "survivor".into(),
        source_name: "Signal Beacon".into(),
        flavor: BTreeMap::new(),
        start: OriginStart {
            scrap: 0.0,
            reactor_capacity: tuning.reactor_capacity,
            reactor_charge: tuning.reactor_capacity,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flavor_falls_back_to_empty() {
        let d = default_origin_def(&Tuning::default());
        assert_eq!(d.id, Origin::Drifter);
        assert_eq!(d.flavor("nonexistent"), "");
    }

    #[test]
    fn default_starts_at_full_charge() {
        let t = Tuning::default();
        let d = default_origin_def(&t);
        assert_eq!(d.start.reactor_capacity, t.reactor_capacity);
        assert_eq!(d.start.reactor_charge, t.reactor_capacity);
        assert_eq!(d.start.scrap, 0.0);
    }
}
