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
    pub ship_weapon_tier: u8,
    pub hull_plating_tier: u8,
}

impl Ship {
    pub fn new(t: &Tuning) -> Self {
        Self {
            built: false,
            drive_range: t.ship_base_drive_range,
            cargo: t.ship_cargo,
            hull: t.ship_base_hull,
            ship_weapon_tier: 0,
            hull_plating_tier: 0,
        }
    }

    pub fn hull_max(&self, t: &Tuning) -> f64 {
        t.ship_base_hull + self.hull_plating_tier as f64 * t.hull_plating_hp_step
    }
}
