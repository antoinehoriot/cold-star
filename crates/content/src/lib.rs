//! Data-driven balance. The embedded RON is the single source of M0 tuning;
//! `load()` parses it into `sim::Tuning` and `validate()` guards the values.
//! Later milestones move this to external `.ron` assets and a full registry.

use sim::Tuning;
use sim::origin::OriginDef;

const TUNING_RON: &str = include_str!("../data/tuning.ron");
const ORIGINS_RON: &str = include_str!("../data/origins.ron");
const JOBS_RON: &str = include_str!("../data/jobs.ron");
const MODULES_RON: &str = include_str!("../data/modules.ron");
const TECHS_RON: &str = include_str!("../data/techs.ron");
const BIOMES_RON: &str = include_str!("../data/biomes.ron");

#[derive(Debug)]
pub enum ContentError {
    Parse(String),
    Invalid(String),
}

pub fn load() -> Result<Tuning, ContentError> {
    let tuning: Tuning =
        ron::from_str(TUNING_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    validate(&tuning)?;
    Ok(tuning)
}

pub fn validate(t: &Tuning) -> Result<(), ContentError> {
    fn finite_pos(v: f64) -> bool {
        v.is_finite() && v > 0.0
    }
    fn finite_nonneg(v: f64) -> bool {
        v.is_finite() && v >= 0.0
    }
    if !finite_pos(t.reactor_capacity) {
        return Err(ContentError::Invalid(
            "reactor_capacity must be finite > 0".into(),
        ));
    }
    if !finite_nonneg(t.reactor_drain_per_tick) {
        return Err(ContentError::Invalid(
            "reactor_drain_per_tick must be finite >= 0".into(),
        ));
    }
    for (name, frac) in [
        (
            "reactor_establish_threshold_frac",
            t.reactor_establish_threshold_frac,
        ),
        (
            "reactor_failing_threshold_frac",
            t.reactor_failing_threshold_frac,
        ),
    ] {
        if !frac.is_finite() || frac <= 0.0 || frac > 1.0 {
            return Err(ContentError::Invalid(format!(
                "{name} must be finite in (0, 1]"
            )));
        }
    }
    if t.reactor_failing_threshold_frac > t.reactor_establish_threshold_frac {
        return Err(ContentError::Invalid(
            "reactor_failing_threshold_frac must be <= reactor_establish_threshold_frac".into(),
        ));
    }
    for (name, v) in [
        ("reactor_ignition_seed", t.reactor_ignition_seed),
        ("reactor_baseline_draw", t.reactor_baseline_draw),
        ("feed_cost_scrap", t.feed_cost_scrap),
        ("feed_charge", t.feed_charge),
        ("salvage_yield", t.salvage_yield),
        ("crew_source_cost", t.crew_source_cost),
        ("crew_trickle", t.crew_trickle),
    ] {
        if !finite_nonneg(v) {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    if t.crew_cap < 1 {
        return Err(ContentError::Invalid("crew_cap must be >= 1".into()));
    }
    if t.arrival_interval < 1 {
        return Err(ContentError::Invalid(
            "arrival_interval must be >= 1".into(),
        ));
    }
    if t.log_capacity < 1 {
        return Err(ContentError::Invalid("log_capacity must be >= 1".into()));
    }
    for (name, v) in [
        ("fuel_per_output", t.fuel_per_output),
        ("reactor_base_max_output", t.reactor_base_max_output),
        ("resource_base_cap", t.resource_base_cap),
        ("crew_ration_upkeep", t.crew_ration_upkeep),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    if !(t.starving_efficiency.is_finite()
        && t.starving_efficiency > 0.0
        && t.starving_efficiency <= 1.0)
    {
        return Err(ContentError::Invalid(
            "starving_efficiency must be finite in (0, 1]".into(),
        ));
    }
    if t.base_housing < 1 {
        return Err(ContentError::Invalid("base_housing must be >= 1".into()));
    }
    for (name, v) in [
        ("atmosphere_draw_per_crew", t.atmosphere_draw_per_crew),
        ("thermal_draw", t.thermal_draw),
        ("thermal_failure_drain", t.thermal_failure_drain),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    if t.atmosphere_grace_max < 1 {
        return Err(ContentError::Invalid(
            "atmosphere_grace_max must be >= 1".into(),
        ));
    }
    if t.crew_loss_interval < 1 {
        return Err(ContentError::Invalid(
            "crew_loss_interval must be >= 1".into(),
        ));
    }
    for (name, v) in [
        ("galaxy_radius", t.galaxy_radius),
        ("galaxy_reveal_radius", t.galaxy_reveal_radius),
        ("star_weight_dwarf", t.star_weight_dwarf),
        ("star_weight_mainseq", t.star_weight_mainseq),
        ("star_weight_neutron", t.star_weight_neutron),
        ("star_weight_dead", t.star_weight_dead),
        ("star_weight_giant", t.star_weight_giant),
        ("ship_base_drive_range", t.ship_base_drive_range),
        ("ship_cargo", t.ship_cargo),
        ("ship_base_hull", t.ship_base_hull),
        ("jump_base_cost", t.jump_base_cost),
        ("jump_drive_factor", t.jump_drive_factor),
        ("transit_ticks_factor", t.transit_ticks_factor),
        ("scan_data_cost", t.scan_data_cost),
        ("site_asteroid_isotopes", t.site_asteroid_isotopes),
        ("site_asteroid_fuel", t.site_asteroid_fuel),
        ("site_derelict_components", t.site_derelict_components),
        ("site_derelict_data", t.site_derelict_data),
        ("reward_gradient", t.reward_gradient),
        ("drive_upgrade_isotopes", t.drive_upgrade_isotopes),
        ("drive_upgrade_step", t.drive_upgrade_step),
        ("scavenge_fuel", t.scavenge_fuel),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    if t.galaxy_system_count < 2 {
        return Err(ContentError::Invalid(
            "galaxy_system_count must be >= 2".into(),
        ));
    }
    if t.ship_base_drive_range <= 0.0 || t.ship_cargo <= 0.0 {
        return Err(ContentError::Invalid(
            "ship_base_drive_range and ship_cargo must be > 0".into(),
        ));
    }
    for (name, v) in [
        ("transit_ticks_base", t.transit_ticks_base),
        ("scan_ticks", t.scan_ticks),
        ("site_resolve_ticks", t.site_resolve_ticks),
        ("scavenge_ticks", t.scavenge_ticks),
    ] {
        if v < 1 {
            return Err(ContentError::Invalid(format!("{name} must be >= 1")));
        }
    }
    if t.star_weight_dwarf
        + t.star_weight_mainseq
        + t.star_weight_neutron
        + t.star_weight_dead
        + t.star_weight_giant
        <= 0.0
    {
        return Err(ContentError::Invalid("star weights must sum to > 0".into()));
    }
    for (name, v) in [
        ("planet_site_weight", t.planet_site_weight),
        ("suit_max", t.suit_max),
        ("suit_drain_per_tick", t.suit_drain_per_tick),
        ("suit_hazard_damage", t.suit_hazard_damage),
        ("surface_cache_base", t.surface_cache_base),
        ("surface_ruin_data", t.surface_ruin_data),
        ("surface_life_harvest", t.surface_life_harvest),
        ("surface_life_leave_data", t.surface_life_leave_data),
        ("surface_depth_reward", t.surface_depth_reward),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    if t.suit_max <= 0.0 {
        return Err(ContentError::Invalid("suit_max must be > 0".into()));
    }
    if t.traverse_ticks < 1 {
        return Err(ContentError::Invalid("traverse_ticks must be >= 1".into()));
    }
    if t.surface_layer_min < 1 || t.surface_layer_max < t.surface_layer_min {
        return Err(ContentError::Invalid(
            "surface_layer_max must be >= surface_layer_min >= 1".into(),
        ));
    }
    // CB1 — combat float fields: finite >= 0
    for (name, v) in [
        ("marine_hp", t.marine_hp),
        ("marine_atk", t.marine_atk),
        ("marine_def_base", t.marine_def_base),
        ("marine_ration_upkeep", t.marine_ration_upkeep),
        ("morale_rout", t.morale_rout),
        ("morale_per_death", t.morale_per_death),
        ("morale_per_flee", t.morale_per_flee),
        ("morale_per_starve_tick", t.morale_per_starve_tick),
        ("morale_recover_per_tick", t.morale_recover_per_tick),
        ("starve_hp_per_tick", t.starve_hp_per_tick),
        ("weapon_step", t.weapon_step),
        ("armor_hp_step", t.armor_hp_step),
        ("armor_def_step", t.armor_def_step),
        ("medgel_heal", t.medgel_heal),
        ("grenade_damage", t.grenade_damage),
        ("shield_def", t.shield_def),
        ("stance_atk_mult", t.stance_atk_mult),
        ("stance_def_bonus", t.stance_def_bonus),
        ("remnant_hp", t.remnant_hp),
        ("remnant_atk", t.remnant_atk),
        ("remnant_def", t.remnant_def),
        ("remnant_speed", t.remnant_speed),
        ("remnant_components", t.remnant_components),
        ("remnant_data", t.remnant_data),
        ("combat_depth_mult", t.combat_depth_mult),
        ("flee_satchel_loss_frac", t.flee_satchel_loss_frac),
        ("weapon_upgrade_alloy", t.weapon_upgrade_alloy),
        ("weapon_upgrade_components", t.weapon_upgrade_components),
        ("armor_upgrade_alloy", t.armor_upgrade_alloy),
        ("armor_upgrade_components", t.armor_upgrade_components),
        ("medgel_cost", t.medgel_cost),
        ("grenade_cost", t.grenade_cost),
        ("shield_cost", t.shield_cost),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    if !t.morale_max.is_finite() || t.morale_max <= 0.0 {
        return Err(ContentError::Invalid(
            "morale_max must be finite > 0".into(),
        ));
    }
    if t.shield_ticks < 1 {
        return Err(ContentError::Invalid("shield_ticks must be >= 1".into()));
    }
    if !t.morale_floor.is_finite() || t.morale_floor <= 0.0 || t.morale_floor > 1.0 {
        return Err(ContentError::Invalid(
            "morale_floor must be finite in (0, 1]".into(),
        ));
    }
    // CB3 — ships in the dark: finite >= 0
    for (name, v) in [
        ("contact_chance", t.contact_chance),
        ("contact_peer_share", t.contact_peer_share),
        ("ship_peer_start_disp", t.ship_peer_start_disp),
        ("ship_disp_spread", t.ship_disp_spread),
        ("ship_base_atk", t.ship_base_atk),
        ("ship_base_def", t.ship_base_def),
        ("ship_speed", t.ship_speed),
        ("ship_weapon_step", t.ship_weapon_step),
        ("hull_plating_def_step", t.hull_plating_def_step),
        ("hull_plating_hp_step", t.hull_plating_hp_step),
        ("peer_ship_hp", t.peer_ship_hp),
        ("peer_ship_atk", t.peer_ship_atk),
        ("peer_ship_def", t.peer_ship_def),
        ("peer_ship_mirror", t.peer_ship_mirror),
        ("remnant_ship_hp", t.remnant_ship_hp),
        ("remnant_ship_atk", t.remnant_ship_atk),
        ("remnant_ship_def", t.remnant_ship_def),
        ("ship_enemy_speed", t.ship_enemy_speed),
        ("ship_salvage_components", t.ship_salvage_components),
        ("ship_salvage_isotopes", t.ship_salvage_isotopes),
        ("ship_salvage_data", t.ship_salvage_data),
        ("repair_drone_heal", t.repair_drone_heal),
        ("repair_drone_cost", t.repair_drone_cost),
        ("ship_weapon_upgrade_alloy", t.ship_weapon_upgrade_alloy),
        (
            "ship_weapon_upgrade_components",
            t.ship_weapon_upgrade_components,
        ),
        ("hull_plating_upgrade_alloy", t.hull_plating_upgrade_alloy),
        (
            "hull_plating_upgrade_components",
            t.hull_plating_upgrade_components,
        ),
        ("hull_regen_docked", t.hull_regen_docked),
        ("retreat_cargo_loss", t.retreat_cargo_loss),
        ("hull_retreat_floor", t.hull_retreat_floor),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    // M6a — the turn & endings: finite >= 0
    for (name, v) in [
        ("expansion_w_peers", t.expansion_w_peers),
        ("expansion_w_strip", t.expansion_w_strip),
        ("expansion_w_consume", t.expansion_w_consume),
        ("expansion_w_systems", t.expansion_w_systems),
        ("watched_threshold", t.watched_threshold),
        ("communion_min_spared", t.communion_min_spared),
        ("communion_max_kills", t.communion_max_kills),
        ("pattern_max_kills", t.pattern_max_kills),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    // CB3 — probability/fraction fields must also be <= 1.0
    if t.contact_chance > 1.0 {
        return Err(ContentError::Invalid(
            "contact_chance must be <= 1.0".into(),
        ));
    }
    if t.contact_peer_share > 1.0 {
        return Err(ContentError::Invalid(
            "contact_peer_share must be <= 1.0".into(),
        ));
    }
    if t.retreat_cargo_loss > 1.0 {
        return Err(ContentError::Invalid(
            "retreat_cargo_loss must be <= 1.0".into(),
        ));
    }
    // CB2 — aliens & disposition
    // Band thresholds: ordering assertion (not non-negative — they may be negative)
    if !t.disp_hostile.is_finite() || !t.disp_neutral.is_finite() || !t.disp_curious.is_finite() {
        return Err(ContentError::Invalid(
            "disp_hostile/disp_neutral/disp_curious must all be finite".into(),
        ));
    }
    if !(t.disp_hostile < t.disp_neutral && t.disp_neutral < t.disp_curious) {
        return Err(ContentError::Invalid(
            "disposition bands must satisfy disp_hostile < disp_neutral < disp_curious".into(),
        ));
    }
    // Positive-magnitude fields: finite >= 0
    for (name, v) in [
        ("parley_shift", t.parley_shift),
        ("gift_shift", t.gift_shift),
        ("gift_cost", t.gift_cost),
        ("trade_give", t.trade_give),
        ("trade_get", t.trade_get),
        ("trade_disp_shift", t.trade_disp_shift),
        ("observe_data", t.observe_data),
        ("observe_shift", t.observe_shift),
        ("harvest_fauna_yield", t.harvest_fauna_yield),
        ("fauna_hp", t.fauna_hp),
        ("fauna_atk", t.fauna_atk),
        ("fauna_def", t.fauna_def),
        ("peer_hp", t.peer_hp),
        ("peer_atk", t.peer_atk),
        ("peer_def", t.peer_def),
        ("peer_gear_mirror", t.peer_gear_mirror),
        ("peer_salvage", t.peer_salvage),
        ("fauna_salvage", t.fauna_salvage),
        ("disp_spread", t.disp_spread),
        ("peace_band", t.peace_band),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(ContentError::Invalid(format!("{name} must be finite >= 0")));
        }
    }
    Ok(())
}

use sim::galaxy::Biome;
use sim::job::{JobId, JobRegistry};
use sim::module::{ModuleId, ModuleRegistry};
use sim::research::{TechId, TechRegistry};

pub fn load_jobs() -> Result<JobRegistry, ContentError> {
    let jobs: Vec<sim::job::Job> =
        ron::from_str(JOBS_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    let reg = JobRegistry(jobs);
    for want in [
        JobId::Salvager,
        JobId::Refiner,
        JobId::Fabricator,
        JobId::Assembler,
        JobId::Hydroponicist,
        JobId::Scientist,
    ] {
        if reg.get(want).is_none() {
            return Err(ContentError::Invalid(format!("jobs.ron missing {want:?}")));
        }
    }
    for j in reg.iter() {
        if !j.power_draw.is_finite() || j.power_draw < 0.0 {
            return Err(ContentError::Invalid(format!(
                "{:?}: power_draw must be finite >= 0",
                j.id
            )));
        }
    }
    Ok(reg)
}

pub fn load_modules() -> Result<ModuleRegistry, ContentError> {
    let mods: Vec<sim::module::Module> =
        ron::from_str(MODULES_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    let reg = ModuleRegistry(mods);
    for want in [
        ModuleId::Refinery,
        ModuleId::Fabricator,
        ModuleId::Assembler,
        ModuleId::Hydroponics,
        ModuleId::Lab,
        ModuleId::Quarters,
        ModuleId::StorageHold,
        ModuleId::Atmosphere,
        ModuleId::Thermal,
        ModuleId::ReactorUpgrade,
        ModuleId::Capacitor,
        ModuleId::Shipyard,
        ModuleId::SensorArray,
        ModuleId::Armory,
    ] {
        if reg.get(want).is_none() {
            return Err(ContentError::Invalid(format!(
                "modules.ron missing {want:?}"
            )));
        }
    }
    for m in reg.iter() {
        if m.description.trim().is_empty() {
            return Err(ContentError::Invalid(format!(
                "{:?}: description must not be empty",
                m.id
            )));
        }
        if m.cost.iter().any(|&(_, a)| !a.is_finite() || a < 0.0) {
            return Err(ContentError::Invalid(format!(
                "{:?}: cost amounts must be finite >= 0",
                m.id
            )));
        }
        if !(m.cost_scaling.is_finite() && m.cost_scaling >= 1.0) {
            return Err(ContentError::Invalid(format!(
                "{:?}: cost_scaling must be finite >= 1",
                m.id
            )));
        }
        let mut seen = std::collections::BTreeSet::new();
        for &(k, _) in &m.cost {
            if !seen.insert(k) {
                return Err(ContentError::Invalid(format!(
                    "{:?}: duplicate resource kind in cost",
                    m.id
                )));
            }
        }
    }
    Ok(reg)
}

pub fn load_techs() -> Result<TechRegistry, ContentError> {
    let techs: Vec<sim::research::Tech> =
        ron::from_str(TECHS_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    let reg = TechRegistry(techs);
    for want in [
        TechId::CoreTuning,
        TechId::ContainmentFields,
        TechId::ResonantCore,
        TechId::RefinedProcesses,
        TechId::ColdLogistics,
        TechId::DeepScan,
        TechId::JumpTheory,
        TechId::Navigation,
        TechId::CombatDoctrine,
        TechId::XenoContact,
        TechId::Munitions,
    ] {
        if reg.get(want).is_none() {
            return Err(ContentError::Invalid(format!("techs.ron missing {want:?}")));
        }
    }
    for t in reg.iter() {
        if !t.data_cost.is_finite() || t.data_cost < 0.0 {
            return Err(ContentError::Invalid(format!(
                "{:?}: data_cost must be finite >= 0",
                t.id
            )));
        }
        if t.research_ticks < 1 {
            return Err(ContentError::Invalid(format!(
                "{:?}: research_ticks must be >= 1",
                t.id
            )));
        }
        for r in &t.requires {
            if reg.get(*r).is_none() {
                return Err(ContentError::Invalid(format!(
                    "{:?}: requires missing {r:?}",
                    t.id
                )));
            }
            if *r == t.id {
                return Err(ContentError::Invalid(format!(
                    "{:?}: requires itself",
                    t.id
                )));
            }
        }
        if t.lore.trim().is_empty() {
            return Err(ContentError::Invalid(format!(
                "{:?}: lore must not be empty",
                t.id
            )));
        }
    }
    // Detect multi-hop requires cycles (A -> B -> A). DFS with a path stack.
    fn has_cycle(
        reg: &TechRegistry,
        id: TechId,
        visiting: &mut std::collections::BTreeSet<TechId>,
        done: &mut std::collections::BTreeSet<TechId>,
    ) -> bool {
        if done.contains(&id) {
            return false;
        }
        if !visiting.insert(id) {
            return true; // id is already on the current path -> cycle
        }
        if let Some(t) = reg.get(id) {
            for &r in &t.requires {
                if has_cycle(reg, r, visiting, done) {
                    return true;
                }
            }
        }
        visiting.remove(&id);
        done.insert(id);
        false
    }
    let mut visiting = std::collections::BTreeSet::new();
    let mut done = std::collections::BTreeSet::new();
    for t in reg.iter() {
        if has_cycle(&reg, t.id, &mut visiting, &mut done) {
            return Err(ContentError::Invalid(format!(
                "tech requires graph has a cycle at {:?}",
                t.id
            )));
        }
    }
    Ok(reg)
}

pub fn load_origins() -> Result<Vec<OriginDef>, ContentError> {
    let origins: Vec<OriginDef> =
        ron::from_str(ORIGINS_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    if origins.is_empty() {
        return Err(ContentError::Invalid("no origins defined".into()));
    }
    for o in &origins {
        if !(o.start.reactor_capacity.is_finite() && o.start.reactor_capacity > 0.0) {
            return Err(ContentError::Invalid(format!(
                "{:?}: reactor_capacity must be finite > 0",
                o.id
            )));
        }
        if !o.start.reactor_charge.is_finite()
            || o.start.reactor_charge < 0.0
            || o.start.reactor_charge > o.start.reactor_capacity
        {
            return Err(ContentError::Invalid(format!(
                "{:?}: reactor_charge must be in [0, capacity]",
                o.id
            )));
        }
        if !o.start.scrap.is_finite() || o.start.scrap < 0.0 {
            return Err(ContentError::Invalid(format!(
                "{:?}: scrap must be finite >= 0",
                o.id
            )));
        }
    }
    Ok(origins)
}

#[derive(Debug, serde::Deserialize)]
pub struct BiomeDef {
    pub biome: Biome,
    pub blurb: String,
}

pub fn load_biomes() -> Result<Vec<BiomeDef>, ContentError> {
    let defs: Vec<BiomeDef> =
        ron::from_str(BIOMES_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    for want in [
        Biome::Frozen,
        Biome::Toxic,
        Biome::Verdant,
        Biome::Ruined,
        Biome::Molten,
        Biome::Dead,
        Biome::Oceanic,
    ] {
        let def = defs
            .iter()
            .find(|d| d.biome == want)
            .ok_or_else(|| ContentError::Invalid(format!("biomes.ron missing {want:?}")))?;
        if def.blurb.trim().is_empty() {
            return Err(ContentError::Invalid(format!(
                "{want:?}: blurb must not be empty"
            )));
        }
    }
    Ok(defs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tech_graph_has_no_cycle() {
        // load_techs runs the DFS cycle check; the real techs.ron must pass it.
        assert!(load_techs().is_ok());
    }

    #[test]
    fn loads_eleven_techs() {
        let techs = load_techs().unwrap();
        assert_eq!(techs.0.len(), 11);
        // Jump Theory requires Deep Scan and sets a flag.
        let jt = techs.get(sim::research::TechId::JumpTheory).unwrap();
        assert!(jt.requires.contains(&sim::research::TechId::DeepScan));
        assert!(matches!(jt.grant, sim::research::Grant::SetFlag(_)));
        // Every requires-id resolves and no tech requires itself.
        for t in techs.iter() {
            for r in &t.requires {
                assert!(
                    techs.get(*r).is_some(),
                    "{:?} requires missing {:?}",
                    t.id,
                    r
                );
                assert_ne!(*r, t.id);
            }
            assert!(!t.lore.trim().is_empty(), "{:?} missing lore", t.id);
        }
    }

    #[test]
    fn munitions_tech_grants_munitions_known_and_requires_combat_doctrine() {
        let techs = load_techs().unwrap();
        let m = techs.get(sim::research::TechId::Munitions).unwrap();
        assert!(
            matches!(
                m.grant,
                sim::research::Grant::SetFlag(sim::flags::Flag::MunitionsKnown)
            ),
            "Munitions must grant Flag::MunitionsKnown"
        );
        assert!(
            m.requires.contains(&sim::research::TechId::CombatDoctrine),
            "Munitions must require CombatDoctrine"
        );
    }

    #[test]
    fn loads_jobs_and_modules() {
        let jobs = load_jobs().unwrap();
        assert_eq!(jobs.0.len(), 6);
        // Salvager is ungated and produces Scrap with zero draw.
        let salv = jobs.get(sim::job::JobId::Salvager).unwrap();
        assert!(salv.gated_by.is_none());
        assert_eq!(salv.power_draw, 0.0);

        let mods = load_modules().unwrap();
        assert_eq!(mods.0.len(), 14);
        // Every module has a non-empty narrative description (the build tooltip).
        for m in mods.iter() {
            assert!(
                !m.description.trim().is_empty(),
                "{:?} missing description",
                m.id
            );
        }
        // Lab is tier 2 but is NOT Data-gated (it unlocks the Scientist that
        // PRODUCES Data — gating it on Data would deadlock).
        let lab = mods.get(sim::module::ModuleId::Lab).unwrap();
        assert_eq!(lab.tier, 2);
        assert!(
            !lab.cost
                .iter()
                .any(|&(k, _)| k == sim::resources::ResourceKind::Data)
        );
        // The Assembler is the real Data sink (tier 2, costs Data).
        let asm = mods.get(sim::module::ModuleId::Assembler).unwrap();
        assert_eq!(asm.tier, 2);
        assert!(
            asm.cost
                .iter()
                .any(|&(k, _)| k == sim::resources::ResourceKind::Data)
        );
        // Capacitor is the Components sink (tier 2, costs Components).
        let cap = mods.get(sim::module::ModuleId::Capacitor).unwrap();
        assert_eq!(cap.tier, 2);
        assert!(
            cap.cost
                .iter()
                .any(|&(k, _)| k == sim::resources::ResourceKind::Components)
        );
    }

    #[test]
    fn loads_three_origins() {
        let origins = load_origins().unwrap();
        assert_eq!(origins.len(), 3);
        let ids: Vec<_> = origins.iter().map(|o| o.id).collect();
        assert!(ids.contains(&sim::origin::Origin::Drifter));
        assert!(ids.contains(&sim::origin::Origin::Hollow));
        assert!(ids.contains(&sim::origin::Origin::Seed));
        // Every origin resolves the flavor keys the engine logs on.
        for o in &origins {
            for key in [
                "ignite",
                "stable",
                "source_active",
                "crew_arrived",
                "failing",
                "cold",
                "shipyard_complete",
            ] {
                assert!(!o.flavor(key).is_empty(), "{:?} missing flavor {key}", o.id);
            }
            assert!(o.start.reactor_capacity > 0.0);
            assert!(
                o.start.reactor_charge >= 0.0 && o.start.reactor_charge <= o.start.reactor_capacity
            );
        }
    }

    #[test]
    fn loads_expanded_m1_tuning() {
        let t = load().unwrap();
        assert_eq!(t.reactor_capacity, 100.0);
        assert_eq!(t.reactor_drain_per_tick, 1.0);
        assert_eq!(t.reactor_establish_threshold_frac, 0.5);
        assert_eq!(t.reactor_failing_threshold_frac, 0.2);
        assert_eq!(t.reactor_ignition_seed, 10.0);
        assert_eq!(t.feed_cost_scrap, 10.0);
        assert_eq!(t.feed_charge, 20.0);
        assert_eq!(t.salvage_yield, 5.0);
        assert_eq!(t.salvage_cooldown_ticks, 5);
        assert_eq!(t.crew_source_cost, 30.0);
        assert_eq!(t.crew_cap, 4);
        assert_eq!(t.arrival_interval, 30);
        assert_eq!(t.crew_trickle, 0.5);
        assert_eq!(t.log_capacity, 100);
    }

    #[test]
    fn rejects_nonpositive_capacity() {
        let bad = Tuning {
            reactor_capacity: 0.0,
            ..Tuning::default()
        };
        assert!(matches!(validate(&bad), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn rejects_negative_drain() {
        let bad = Tuning {
            reactor_drain_per_tick: -1.0,
            ..Tuning::default()
        };
        assert!(matches!(validate(&bad), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn rejects_nan_capacity() {
        let bad = Tuning {
            reactor_capacity: f64::NAN,
            ..Tuning::default()
        };
        assert!(matches!(validate(&bad), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn loads_m2_tuning() {
        let t = load().unwrap();
        assert_eq!(t.fuel_per_output, 0.5);
        assert_eq!(t.reactor_base_max_output, 5.0);
        assert_eq!(t.resource_base_cap, 100.0);
        assert_eq!(t.crew_ration_upkeep, 0.1);
        assert_eq!(t.starving_efficiency, 0.5);
        assert_eq!(t.base_housing, 4);
        assert_eq!(t.reactor_baseline_draw, 1.0);
    }

    #[test]
    fn loads_m3_tuning() {
        let t = load().unwrap();
        assert_eq!(t.atmosphere_draw_per_crew, 0.2);
        assert_eq!(t.atmosphere_grace_max, 30);
        assert_eq!(t.crew_loss_interval, 10);
        assert_eq!(t.thermal_draw, 1.0);
        assert_eq!(t.thermal_failure_drain, 2.0);
    }

    #[test]
    fn loads_m4_tuning() {
        let t = load().unwrap();
        assert_eq!(t.galaxy_system_count, 10);
        assert_eq!(t.galaxy_radius, 100.0);
        assert_eq!(t.galaxy_reveal_radius, 45.0);
        assert_eq!(t.ship_base_drive_range, 40.0);
        assert_eq!(t.ship_cargo, 60.0);
        assert_eq!(t.jump_base_cost, 5.0);
        assert_eq!(t.jump_drive_factor, 0.2);
        assert_eq!(t.transit_ticks_base, 8);
        assert_eq!(t.scan_data_cost, 4.0);
        assert_eq!(t.scan_ticks, 10);
        assert_eq!(t.site_resolve_ticks, 12);
        assert_eq!(t.site_asteroid_isotopes, 8.0);
        assert_eq!(t.drive_upgrade_isotopes, 10.0);
        assert_eq!(t.drive_upgrade_step, 15.0);
        assert_eq!(t.scavenge_fuel, 6.0);
    }

    #[test]
    fn loads_seven_biomes_with_blurbs() {
        let b = load_biomes().unwrap();
        assert_eq!(b.len(), 7);
        for want in [
            sim::galaxy::Biome::Frozen,
            sim::galaxy::Biome::Toxic,
            sim::galaxy::Biome::Verdant,
            sim::galaxy::Biome::Ruined,
            sim::galaxy::Biome::Molten,
            sim::galaxy::Biome::Dead,
            sim::galaxy::Biome::Oceanic,
        ] {
            let def = b.iter().find(|d| d.biome == want).expect("biome present");
            assert!(!def.blurb.trim().is_empty());
        }
    }

    #[test]
    fn loads_m5_tuning() {
        let t = load().unwrap();
        assert_eq!(t.suit_max, 100.0);
        assert_eq!(t.suit_drain_per_tick, 0.6);
        assert_eq!(t.suit_hazard_damage, 18.0);
        assert_eq!(t.traverse_ticks, 4);
        assert_eq!(t.surface_depth_max, 3);
        assert_eq!(t.surface_cache_base, 14.0);
        assert_eq!(t.surface_ruin_data, 10.0);
        assert_eq!(t.surface_life_harvest, 22.0);
        assert_eq!(t.planet_site_weight, 0.5);
    }

    #[test]
    fn xeno_contact_grants_flag_and_requires_deep_scan() {
        let techs = load_techs().unwrap();
        let xc = techs.get(sim::research::TechId::XenoContact).unwrap();
        assert!(
            matches!(
                xc.grant,
                sim::research::Grant::SetFlag(sim::flags::Flag::XenoContactKnown)
            ),
            "XenoContact must grant XenoContactKnown"
        );
        assert!(
            xc.requires.contains(&sim::research::TechId::DeepScan),
            "XenoContact must require DeepScan"
        );
    }

    #[test]
    fn combat_doctrine_and_armory_present() {
        let techs = load_techs().unwrap();
        let cd = techs.get(sim::research::TechId::CombatDoctrine).unwrap();
        assert!(matches!(
            cd.grant,
            sim::research::Grant::SetFlag(sim::flags::Flag::CombatDoctrineKnown)
        ));
        let mods = load_modules().unwrap();
        let armory = mods.get(sim::module::ModuleId::Armory).unwrap();
        assert_eq!(
            armory.requires_flag,
            Some(sim::flags::Flag::CombatDoctrineKnown)
        );
    }

    #[test]
    fn loads_cb1_tuning() {
        let t = load().unwrap();
        assert_eq!(t.marine_hp, 30.0);
        assert_eq!(t.marine_atk, 6.0);
        assert_eq!(t.morale_max, 100.0);
        assert_eq!(t.morale_rout, 20.0);
        assert_eq!(t.weapon_step, 0.5);
        assert_eq!(t.medgel_heal, 25.0);
        assert_eq!(t.grenade_damage, 40.0);
        assert_eq!(t.shield_ticks, 5);
        assert_eq!(t.remnant_hp, 60.0);
        assert_eq!(t.flee_satchel_loss_frac, 0.25);
        assert_eq!(t.medgel_cost, 5.0);
    }

    #[test]
    fn loads_cb2_tuning() {
        let t = load().unwrap();
        assert_eq!(t.disp_hostile, -40.0);
        assert_eq!(t.disp_curious, 30.0);
        assert_eq!(t.parley_shift, 18.0);
        assert_eq!(t.gift_shift, 35.0);
        assert_eq!(t.peer_hp, 70.0);
        assert_eq!(t.peer_gear_mirror, 0.5);
        assert_eq!(t.harvest_fauna_yield, 16.0);
    }

    #[test]
    fn loads_cb3_tuning() {
        let t = load().unwrap();
        assert_eq!(t.contact_chance, 0.35);
        assert_eq!(t.contact_peer_share, 0.6);
        assert_eq!(t.peer_ship_hp, 90.0);
        assert_eq!(t.peer_ship_mirror, 0.5);
        assert_eq!(t.hull_plating_hp_step, 40.0);
        assert_eq!(t.repair_drone_heal, 35.0);
        assert_eq!(t.hull_regen_docked, 3.0);
        assert_eq!(t.retreat_cargo_loss, 0.4);
    }

    #[test]
    fn loads_m6a_tuning() {
        let t = load().unwrap();
        assert_eq!(t.expansion_w_peers, 8.0);
        assert_eq!(t.watched_threshold, 120.0);
        assert_eq!(t.communion_min_spared, 3.0);
        assert_eq!(t.pattern_max_kills, 2.0);
    }
}
