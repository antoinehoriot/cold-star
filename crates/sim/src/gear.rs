//! Crafted loadout: persistent gear tiers + countable consumables. Built at the
//! Armory; consumed on the surface in combat. No new base resource.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConsumableKind {
    MedGel,
    Grenade,
    Shield,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gear {
    pub weapon_tier: u8,
    pub armor_tier: u8,
}
