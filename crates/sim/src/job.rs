//! Crew jobs. Each job consumes inputs and produces outputs per crew per tick,
//! draws power, and is gated by a module. Data-driven; loaded from RON.

use crate::module::ModuleId;
use crate::resources::ResourceKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JobId {
    Salvager,
    Refiner,
    Fabricator,
    Assembler,
    Hydroponicist,
    Scientist,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub inputs: Vec<(ResourceKind, f64)>,
    pub outputs: Vec<(ResourceKind, f64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub id: JobId,
    pub name: String,
    pub gated_by: Option<ModuleId>,
    pub recipe: Recipe,
    pub power_draw: f64,
    pub default_priority: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRegistry(pub Vec<Job>);

impl JobRegistry {
    pub fn get(&self, id: JobId) -> Option<&Job> {
        self.0.iter().find(|j| j.id == id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Job> {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lookup_by_id() {
        let reg = JobRegistry(vec![Job {
            id: JobId::Salvager,
            name: "Salvager".into(),
            gated_by: None,
            recipe: Recipe {
                inputs: vec![],
                outputs: vec![(ResourceKind::Scrap, 1.0)],
            },
            power_draw: 0.0,
            default_priority: 0,
        }]);
        assert_eq!(reg.get(JobId::Salvager).unwrap().power_draw, 0.0);
        assert!(reg.get(JobId::Refiner).is_none());
    }
}
