//! Research — a hybrid tech system. A tech costs Data up front to START, then
//! researches over a number of ticks; on completion its `grant` is applied and
//! its `lore` surfaces. Only `completed` (TechId set) + the active timer are
//! state; effects are applied once on completion (see state.rs).

use crate::flags::Flag;
use crate::job::JobId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TechId {
    CoreTuning,
    ContainmentFields,
    ResonantCore,
    RefinedProcesses,
    ColdLogistics,
    DeepScan,
    JumpTheory,
    Navigation,
    CombatDoctrine,
    XenoContact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Grant {
    ReactorMaxOutput(f64),
    ReactorCapacity(f64),
    JobOutputMult { job: JobId, mult: f64 },
    ResourceCapBonus(f64),
    SetFlag(Flag),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tech {
    pub id: TechId,
    pub name: String,
    pub blurb: String,
    pub data_cost: f64,
    pub research_ticks: u32,
    pub requires: Vec<TechId>,
    pub grant: Grant,
    pub lore: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TechRegistry(pub Vec<Tech>);

impl TechRegistry {
    pub fn get(&self, id: TechId) -> Option<&Tech> {
        self.0.iter().find(|t| t.id == id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Tech> {
        self.0.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveResearch {
    pub tech: TechId,
    pub ticks_remaining: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Research {
    pub completed: BTreeSet<TechId>,
    pub active: Option<ActiveResearch>,
}

impl Research {
    pub fn is_completed(&self, id: TechId) -> bool {
        self.completed.contains(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lookup() {
        let reg = TechRegistry(vec![Tech {
            id: TechId::CoreTuning,
            name: "Core Tuning".into(),
            blurb: "+reactor output".into(),
            data_cost: 5.0,
            research_ticks: 30,
            requires: vec![],
            grant: Grant::ReactorMaxOutput(2.0),
            lore: "...".into(),
        }]);
        assert_eq!(reg.get(TechId::CoreTuning).unwrap().research_ticks, 30);
        assert!(reg.get(TechId::JumpTheory).is_none());
    }
}
