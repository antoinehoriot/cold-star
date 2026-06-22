//! Stockpiled resources. M1 has only Scrap, uncapped. Grows per milestone.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    pub scrap: f64,
}

impl Resources {
    /// Add scrap (uncapped in M1).
    pub fn add_scrap(&mut self, amount: f64) {
        self.scrap += amount;
    }

    /// Spend scrap iff affordable. Returns false (and changes nothing) if not.
    pub fn try_spend_scrap(&mut self, amount: f64) -> bool {
        if self.scrap >= amount {
            self.scrap -= amount;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_spend_scrap() {
        let mut r = Resources::default();
        r.add_scrap(15.0);
        assert_eq!(r.scrap, 15.0);
        assert!(r.try_spend_scrap(10.0));
        assert_eq!(r.scrap, 5.0);
        assert!(!r.try_spend_scrap(10.0)); // insufficient
        assert_eq!(r.scrap, 5.0); // unchanged on failure
    }
}
