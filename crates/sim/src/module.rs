//! Buildable modules. Data-driven; the registry is loaded from RON by `content`.

use crate::job::JobId;
use crate::resources::ResourceKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ModuleId {
    Refinery,
    Fabricator,
    Assembler,
    Hydroponics,
    Lab,
    Quarters,
    StorageHold,
    LifeSupport,
    ReactorUpgrade,
    Capacitor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ModuleUnlocks {
    pub job: Option<JobId>,
    pub housing: u32,
    pub cap_increase: f64,
    pub max_output: f64,
    pub capacity_increase: f64,
    pub passive_draw_per_crew: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub id: ModuleId,
    pub name: String,
    /// One-sentence in-world description, shown as a pre-build tooltip.
    pub description: String,
    pub tier: u8,
    pub cost: Vec<(ResourceKind, f64)>,
    pub cost_scaling: f64,
    pub repeatable: bool,
    pub requires_modules: Vec<ModuleId>,
    pub unlocks: ModuleUnlocks,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleRegistry(pub Vec<Module>);

impl ModuleRegistry {
    pub fn get(&self, id: ModuleId) -> Option<&Module> {
        self.0.iter().find(|m| m.id == id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Module> {
        self.0.iter()
    }
}
