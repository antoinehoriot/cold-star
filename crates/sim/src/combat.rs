use crate::config::Tuning;
use crate::gear::Gear;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    Aggressive,
    Defensive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnemyKind {
    Remnant,
    Fauna,
    Peer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Combat {
    pub enemy_hp: f64,
    pub enemy_atk: f64,
    pub enemy_def: f64,
    pub enemy_speed: f64,
    pub stance: Stance,
    pub shield_ticks: u32,
    pub kind: EnemyKind,
}

/// Shared enemy constructor: applies the depth/frontier scaling
/// `m = (1 + depth * combat_depth_mult) * grad`, then `mirror`, to the base
/// `(hp, atk, def)`. Pass `mirror = 1.0` for non-mirroring enemies (exact
/// identity in IEEE arithmetic). All five enemy-stat fns route through this.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scaled_enemy_stats(
    depth: u32,
    grad: f64,
    mirror: f64,
    hp: f64,
    atk: f64,
    def: f64,
    speed: f64,
    stance: Stance,
    kind: EnemyKind,
    t: &Tuning,
) -> Combat {
    let m = (1.0 + depth as f64 * t.combat_depth_mult) * grad;
    Combat {
        enemy_hp: hp * m * mirror,
        enemy_atk: atk * m * mirror,
        enemy_def: def * m * mirror,
        enemy_speed: speed,
        stance,
        shield_ticks: 0,
        kind,
    }
}

pub fn remnant_stats(depth: u32, grad: f64, t: &Tuning) -> Combat {
    scaled_enemy_stats(
        depth,
        grad,
        1.0,
        t.remnant_hp,
        t.remnant_atk,
        t.remnant_def,
        t.remnant_speed,
        Stance::Defensive,
        EnemyKind::Remnant,
        t,
    )
}

pub struct SquadStats {
    pub atk: f64,
    pub def: f64,
    pub speed: f64,
}

pub fn squad_stats(
    crew: u32,
    morale: f64,
    gear: Gear,
    stance: Stance,
    shield_active: bool,
    t: &Tuning,
) -> SquadStats {
    let morale_factor = (morale / t.morale_max).clamp(t.morale_floor, 1.0);
    let weapon = 1.0 + gear.weapon_tier as f64 * t.weapon_step;
    let (atk_mult, def_bonus) = match stance {
        Stance::Aggressive => (t.stance_atk_mult, 0.0),
        Stance::Defensive => (1.0, t.stance_def_bonus),
    };
    let atk = crew as f64 * t.marine_atk * weapon * morale_factor * atk_mult;
    let def = t.marine_def_base
        + gear.armor_tier as f64 * t.armor_def_step
        + def_bonus
        + if shield_active { t.shield_def } else { 0.0 };
    SquadStats {
        atk,
        def,
        speed: 1.0,
    }
}

pub struct TickOutcome {
    pub enemy_damage: f64,
    pub squad_damage: f64,
}

/// One combat exchange (pure). Caller applies the damage + casualty logic.
pub fn resolve_tick(combat: &Combat, squad: &SquadStats, _t: &Tuning) -> TickOutcome {
    let enemy_damage = (squad.atk - combat.enemy_def).max(0.0) * squad.speed;
    let squad_damage = (combat.enemy_atk - squad.def).max(0.0) * combat.enemy_speed;
    TickOutcome {
        enemy_damage,
        squad_damage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Tuning;
    use crate::gear::Gear;

    #[test]
    fn squad_attack_scales_with_crew_weapon_morale_stance() {
        let t = Tuning::default();
        let base = squad_stats(
            2,
            t.morale_max,
            Gear {
                weapon_tier: 0,
                armor_tier: 0,
            },
            Stance::Defensive,
            false,
            &t,
        );
        let strong = squad_stats(
            4,
            t.morale_max,
            Gear {
                weapon_tier: 2,
                armor_tier: 0,
            },
            Stance::Aggressive,
            false,
            &t,
        );
        assert!(strong.atk > base.atk);
        assert!(strong.def < base.def + t.stance_def_bonus); // aggressive lowers def vs defensive
    }

    #[test]
    fn resolve_tick_trades_damage_min_zero() {
        let t = Tuning::default();
        let mut c = remnant_stats(1, 1.0, &t);
        let sq = squad_stats(
            3,
            t.morale_max,
            Gear {
                weapon_tier: 1,
                armor_tier: 1,
            },
            Stance::Aggressive,
            false,
            &t,
        );
        let out = resolve_tick(&c, &sq, &t);
        assert!(out.enemy_damage >= 0.0 && out.squad_damage >= 0.0);
        c.enemy_hp -= out.enemy_damage; // caller applies
        assert!(c.enemy_hp < remnant_stats(1, 1.0, &t).enemy_hp);
    }
}
