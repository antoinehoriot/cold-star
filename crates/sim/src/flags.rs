//! Progressive-disclosure flags. The engine sets these at milestones during
//! `advance`/`apply`; the UI gates components on their presence. Never cleared
//! in M1. `Ord` so they live in a deterministic `BTreeSet`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Flag {
    SalvageUnlocked,
    CoreEstablished,
    CrewSourceAvailable,
    CrewSourceActive,
    CrewArrived,
    EconomyUnlocked,
    BuildMenuUnlocked,
    PowerBudgetVisible,
    Tier2Unlocked,
    JumpTheoryKnown,
    ResearchUnlocked,
    CodexUnlocked,
    ShipyardComplete,
    NavigationKnown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn flags_live_in_a_btreeset() {
        let mut s = BTreeSet::new();
        s.insert(Flag::SalvageUnlocked);
        s.insert(Flag::SalvageUnlocked); // idempotent
        s.insert(Flag::CrewArrived);
        assert_eq!(s.len(), 2);
        assert!(s.contains(&Flag::SalvageUnlocked));
    }
}
