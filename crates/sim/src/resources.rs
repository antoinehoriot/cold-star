//! Stockpiled resources. M2: six resources, each subject to a single shared
//! storage `cap` (raised by Storage Hold modules). Overflow is wasted.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    Scrap,
    Alloy,
    Fuel,
    Rations,
    Components,
    Data,
    Isotopes,
}

impl ResourceKind {
    /// All resource kinds, in declaration order. Deterministic iteration.
    pub const ALL: [ResourceKind; 7] = [
        ResourceKind::Scrap,
        ResourceKind::Alloy,
        ResourceKind::Fuel,
        ResourceKind::Rations,
        ResourceKind::Components,
        ResourceKind::Data,
        ResourceKind::Isotopes,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    pub scrap: f64,
    pub alloy: f64,
    pub fuel: f64,
    pub rations: f64,
    pub components: f64,
    pub data: f64,
    pub isotopes: f64,
    /// Shared storage cap applied to every resource.
    pub cap: f64,
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            scrap: 0.0,
            alloy: 0.0,
            fuel: 0.0,
            rations: 0.0,
            components: 0.0,
            data: 0.0,
            isotopes: 0.0,
            cap: f64::INFINITY,
        }
    }
}

impl Resources {
    fn slot(&mut self, k: ResourceKind) -> &mut f64 {
        match k {
            ResourceKind::Scrap => &mut self.scrap,
            ResourceKind::Alloy => &mut self.alloy,
            ResourceKind::Fuel => &mut self.fuel,
            ResourceKind::Rations => &mut self.rations,
            ResourceKind::Components => &mut self.components,
            ResourceKind::Data => &mut self.data,
            ResourceKind::Isotopes => &mut self.isotopes,
        }
    }

    pub fn amount(&self, k: ResourceKind) -> f64 {
        match k {
            ResourceKind::Scrap => self.scrap,
            ResourceKind::Alloy => self.alloy,
            ResourceKind::Fuel => self.fuel,
            ResourceKind::Rations => self.rations,
            ResourceKind::Components => self.components,
            ResourceKind::Data => self.data,
            ResourceKind::Isotopes => self.isotopes,
        }
    }

    pub fn set_cap(&mut self, cap: f64) {
        self.cap = cap;
    }

    /// Add `amount` of `k`, clamped to `cap`. Returns the overflow that was wasted.
    pub fn add(&mut self, k: ResourceKind, amount: f64) -> f64 {
        let cap = self.cap;
        let s = self.slot(k);
        let raw = *s + amount;
        if raw > cap {
            *s = cap;
            raw - cap
        } else {
            *s = raw;
            0.0
        }
    }

    /// Spend `amount` of `k` iff affordable; nothing changes on failure.
    pub fn try_spend(&mut self, k: ResourceKind, amount: f64) -> bool {
        let s = self.slot(k);
        if *s >= amount {
            *s -= amount;
            true
        } else {
            false
        }
    }

    /// Spend all of `costs` atomically iff every line is affordable.
    pub fn try_spend_all(&mut self, costs: &[(ResourceKind, f64)]) -> bool {
        if costs.iter().any(|&(k, a)| self.amount(k) < a) {
            return false;
        }
        for &(k, a) in costs {
            *self.slot(k) -= a;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_spend_uncapped_by_default() {
        let mut r = Resources::default();
        r.add(ResourceKind::Scrap, 15.0);
        assert_eq!(r.amount(ResourceKind::Scrap), 15.0);
        assert!(r.try_spend(ResourceKind::Scrap, 10.0));
        assert_eq!(r.scrap, 5.0);
        assert!(!r.try_spend(ResourceKind::Scrap, 10.0));
        assert_eq!(r.scrap, 5.0);
    }

    #[test]
    fn add_clamps_at_cap_and_reports_overflow() {
        let mut r = Resources::default();
        r.set_cap(100.0);
        let overflow = r.add(ResourceKind::Alloy, 130.0);
        assert_eq!(r.amount(ResourceKind::Alloy), 100.0);
        assert_eq!(overflow, 30.0);
    }

    #[test]
    fn try_spend_all_is_atomic() {
        let mut r = Resources::default();
        r.add(ResourceKind::Alloy, 10.0);
        r.add(ResourceKind::Data, 1.0);
        // Not enough Data for this combined cost -> nothing spent.
        assert!(!r.try_spend_all(&[(ResourceKind::Alloy, 5.0), (ResourceKind::Data, 2.0)]));
        assert_eq!(r.amount(ResourceKind::Alloy), 10.0);
        assert_eq!(r.amount(ResourceKind::Data), 1.0);
        // Affordable combined cost -> all spent.
        assert!(r.try_spend_all(&[(ResourceKind::Alloy, 5.0), (ResourceKind::Data, 1.0)]));
        assert_eq!(r.amount(ResourceKind::Alloy), 5.0);
        assert_eq!(r.amount(ResourceKind::Data), 0.0);
    }

    #[test]
    fn isotopes_is_a_capped_resource() {
        let mut r = Resources::default();
        r.set_cap(50.0);
        let overflow = r.add(ResourceKind::Isotopes, 70.0);
        assert_eq!(r.amount(ResourceKind::Isotopes), 50.0);
        assert_eq!(overflow, 20.0);
        assert!(r.try_spend(ResourceKind::Isotopes, 30.0));
        assert_eq!(r.amount(ResourceKind::Isotopes), 20.0);
    }

    #[test]
    fn all_has_seven_kinds_isotopes_last() {
        assert_eq!(ResourceKind::ALL.len(), 7);
        assert_eq!(ResourceKind::ALL[6], ResourceKind::Isotopes);
    }
}
