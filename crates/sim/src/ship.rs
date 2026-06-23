//! The player's vessel. A compact stat block (M4 slice): drive range + cargo
//! are the stats that matter; hull is stored for M5 combat but unexercised.

use crate::config::Tuning;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ship {
    pub built: bool,
    pub drive_range: f64,
    pub cargo: f64,
    pub hull: f64,
}

impl Ship {
    pub fn new(t: &Tuning) -> Self {
        Self {
            built: false,
            drive_range: t.ship_base_drive_range,
            cargo: t.ship_cargo,
            hull: t.ship_base_hull,
        }
    }
}
