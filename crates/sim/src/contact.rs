//! Ship-vs-ship contacts. A vessel met on arrival in a system, resolved by
//! negotiation (reusing CB2 disposition) or a ship fight (reusing CB1 Combat).
//! Pure: the contact roll + disposition are seeded; combat is tuning-derived.
use crate::combat::{Combat, EnemyKind, SquadStats, Stance};
use crate::config::Tuning;
use crate::galaxy::SystemId;
use crate::rng::{Rng, hash};
use crate::ship::Ship;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipKind {
    Peer,
    Remnant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipContact {
    pub kind: ShipKind,
    pub disposition: f64,
    pub combat: Option<Combat>,
}

pub(crate) const CONTACT_SALT: u32 = 0x5417;

/// Seeded starting disposition for a Peer ship (pure on world_seed + system).
pub fn contact_start_disposition(world_seed: u64, system: SystemId, t: &Tuning) -> f64 {
    let mut rng = Rng::new(hash(world_seed ^ CONTACT_SALT as u64, system.0));
    t.ship_peer_start_disp + (rng.next_f64() * 2.0 - 1.0) * t.ship_disp_spread
}

/// The player's ship as combat stats (no morale/rout — ships don't panic).
pub fn ship_stats(ship: &Ship, stance: Stance, t: &Tuning) -> SquadStats {
    let weapon = 1.0 + ship.ship_weapon_tier as f64 * t.ship_weapon_step;
    let (atk_mult, def_bonus) = match stance {
        Stance::Aggressive => (t.stance_atk_mult, 0.0),
        Stance::Defensive => (1.0, t.stance_def_bonus),
    };
    let atk = t.ship_base_atk * weapon * atk_mult;
    let def = t.ship_base_def + ship.hull_plating_tier as f64 * t.hull_plating_def_step + def_bonus;
    SquadStats {
        atk,
        def,
        speed: t.ship_speed,
    }
}

pub fn peer_ship_stats(depth: u32, grad: f64, ship: &Ship, t: &Tuning) -> Combat {
    let m = (1.0 + depth as f64 * t.combat_depth_mult) * grad;
    let mirror = 1.0 + (ship.ship_weapon_tier + ship.hull_plating_tier) as f64 * t.peer_ship_mirror;
    Combat {
        enemy_hp: t.peer_ship_hp * m * mirror,
        enemy_atk: t.peer_ship_atk * m * mirror,
        enemy_def: t.peer_ship_def * m * mirror,
        enemy_speed: t.ship_enemy_speed,
        stance: Stance::Aggressive,
        shield_ticks: 0,
        kind: EnemyKind::Peer,
    }
}

pub fn remnant_ship_stats(depth: u32, grad: f64, t: &Tuning) -> Combat {
    let m = (1.0 + depth as f64 * t.combat_depth_mult) * grad;
    Combat {
        enemy_hp: t.remnant_ship_hp * m,
        enemy_atk: t.remnant_ship_atk * m,
        enemy_def: t.remnant_ship_def * m,
        enemy_speed: t.ship_enemy_speed,
        stance: Stance::Aggressive,
        shield_ticks: 0,
        kind: EnemyKind::Remnant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Tuning;

    #[test]
    fn contact_start_disposition_is_pure() {
        let t = Tuning::default();
        let a = contact_start_disposition(7, crate::galaxy::SystemId(3), &t);
        assert_eq!(
            a,
            contact_start_disposition(7, crate::galaxy::SystemId(3), &t)
        );
        assert_ne!(
            a,
            contact_start_disposition(7, crate::galaxy::SystemId(4), &t)
        );
    }

    #[test]
    fn peer_ship_scales_with_player_ship_tier() {
        let t = Tuning::default();
        let mut weak = crate::ship::Ship::new(&t);
        let mut strong = crate::ship::Ship::new(&t);
        strong.ship_weapon_tier = 2;
        strong.hull_plating_tier = 2;
        let w = peer_ship_stats(1, 1.0, &weak, &t);
        let s = peer_ship_stats(1, 1.0, &strong, &t);
        assert!(s.enemy_hp > w.enemy_hp);
        assert_eq!(w.kind, crate::combat::EnemyKind::Peer);
        let _ = (&mut weak, &mut strong);
    }
}
