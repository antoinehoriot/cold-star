//! Where the player is in the galaxy and what single timed activity (if any)
//! the ship is engaged in. One activity at a time — the single expedition slot.

use crate::galaxy::{HOME, SystemId};
use crate::landing::Landing;
use crate::resources::ResourceKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Activity {
    Transit {
        to: SystemId,
        ticks: u32,
    },
    Scan {
        target: SystemId,
        ticks: u32,
    },
    ResolveSite {
        system: SystemId,
        site_idx: usize,
        ticks: u32,
    },
    Scavenge {
        ticks: u32,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AwayTeam {
    pub crew: u32,
    pub hp: f64,
    pub morale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expedition {
    pub location: SystemId,
    pub activity: Option<Activity>,
    pub cargo: BTreeMap<ResourceKind, f64>,
    pub landing: Option<Landing>,
    pub away_team: AwayTeam,
    pub contact: Option<crate::contact::ShipContact>,
    pub prev_location: SystemId,
}

impl Default for Expedition {
    fn default() -> Self {
        Self {
            location: HOME,
            activity: None,
            cargo: BTreeMap::new(),
            landing: None,
            away_team: AwayTeam::default(),
            contact: None,
            prev_location: HOME,
        }
    }
}

impl Expedition {
    pub fn cargo_total(&self) -> f64 {
        self.cargo.values().copied().sum()
    }
}
