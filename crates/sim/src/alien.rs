//! Aliens & disposition. Fauna + Peers, resolved through a dynamic disposition
//! stat. Pure: dispositions/outcomes are seeded or tuning-driven (no resolve RNG).
use crate::combat::{Combat, EnemyKind, Stance};
use crate::config::Tuning;
use crate::gear::Gear;
use crate::rng::{Rng, hash};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlienKind {
    Fauna,
    Peer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Band {
    Hostile,
    Fearful,
    Neutral,
    Curious,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlienEncounter {
    pub kind: AlienKind,
    pub disposition: f64,
}

const ALIEN_SALT: u32 = 0xA11E;

pub fn band(disposition: f64, t: &Tuning) -> Band {
    if disposition <= t.disp_hostile {
        Band::Hostile
    } else if disposition <= t.disp_neutral {
        Band::Fearful
    } else if disposition < t.disp_curious {
        Band::Neutral
    } else {
        Band::Curious
    }
}

/// Deterministic starting disposition: an anchor by kind ± a seeded spread.
pub fn starting_disposition(site_seed: u64, node: u32, kind: AlienKind, t: &Tuning) -> f64 {
    let mut rng = Rng::new(hash(site_seed, node ^ ALIEN_SALT));
    let anchor = match kind {
        AlienKind::Fauna => t.fauna_start_disp,
        AlienKind::Peer => t.peer_start_disp,
    };
    anchor + (rng.next_f64() * 2.0 - 1.0) * t.disp_spread
}

pub fn fauna_stats(depth: u32, grad: f64, t: &Tuning) -> Combat {
    let m = (1.0 + depth as f64 * t.combat_depth_mult) * grad;
    Combat {
        enemy_hp: t.fauna_hp * m,
        enemy_atk: t.fauna_atk * m,
        enemy_def: t.fauna_def * m,
        enemy_speed: t.remnant_speed,
        stance: Stance::Defensive,
        shield_ticks: 0,
        kind: EnemyKind::Fauna,
    }
}

/// A Peer's strength mirrors the player's own gear tier — they scaled as you did.
pub fn peer_stats(depth: u32, grad: f64, gear: Gear, t: &Tuning) -> Combat {
    let m = (1.0 + depth as f64 * t.combat_depth_mult) * grad;
    let mirror = 1.0 + (gear.weapon_tier + gear.armor_tier) as f64 * t.peer_gear_mirror;
    Combat {
        enemy_hp: t.peer_hp * m * mirror,
        enemy_atk: t.peer_atk * m * mirror,
        enemy_def: t.peer_def * m * mirror,
        enemy_speed: t.remnant_speed,
        stance: Stance::Defensive,
        shield_ticks: 0,
        kind: EnemyKind::Peer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Tuning;
    use crate::gear::Gear;

    #[test]
    fn bands_map_from_disposition() {
        let t = Tuning::default();
        assert_eq!(band(-80.0, &t), Band::Hostile);
        assert_eq!(band(50.0, &t), Band::Curious);
        assert_eq!(band(0.0, &t), Band::Neutral);
    }

    #[test]
    fn starting_disposition_is_pure_and_kind_sensitive() {
        let t = Tuning::default();
        let a = starting_disposition(7, 3, AlienKind::Peer, &t);
        let b = starting_disposition(7, 3, AlienKind::Peer, &t);
        assert_eq!(a, b);
        // Fauna start more wary (lower) than the peer anchor on average — same seed,
        // different kind → different value.
        assert_ne!(a, starting_disposition(7, 3, AlienKind::Fauna, &t));
    }

    #[test]
    fn peer_stats_scale_with_player_gear() {
        let t = Tuning::default();
        let weak = peer_stats(
            1,
            1.0,
            Gear {
                weapon_tier: 0,
                armor_tier: 0,
            },
            &t,
        );
        let strong = peer_stats(
            1,
            1.0,
            Gear {
                weapon_tier: 2,
                armor_tier: 2,
            },
            &t,
        );
        assert!(strong.enemy_hp > weak.enemy_hp); // they scaled as you did
        assert_eq!(weak.kind, crate::combat::EnemyKind::Peer);
    }
}
