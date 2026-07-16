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

/// How a float tuning field is constrained.
#[derive(Clone, Copy)]
enum Check {
    /// Finite only — negative values are legal.
    Finite,
    /// Finite and >= 0.
    NonNeg,
    /// Finite and > 0.
    Positive,
    /// Finite and in (0, 1].
    UnitOpen,
    /// Finite and in [0, 1].
    UnitClosed,
}

impl Check {
    fn ok(self, v: f64) -> bool {
        v.is_finite()
            && match self {
                Check::Finite => true,
                Check::NonNeg => v >= 0.0,
                Check::Positive => v > 0.0,
                Check::UnitOpen => v > 0.0 && v <= 1.0,
                Check::UnitClosed => (0.0..=1.0).contains(&v),
            }
    }
    fn expects(self) -> &'static str {
        match self {
            Check::Finite => "finite",
            Check::NonNeg => "finite >= 0",
            Check::Positive => "finite > 0",
            Check::UnitOpen => "finite in (0, 1]",
            Check::UnitClosed => "finite in [0, 1]",
        }
    }
}

pub fn validate(t: &Tuning) -> Result<(), ContentError> {
    use Check::{Finite, NonNeg, Positive, UnitClosed, UnitOpen};
    // One entry per validated float field: (name, value, check).
    // NOTE: salvage_cooldown_ticks and surface_depth_max are u32 where every
    // representable value is legal (0 = no cooldown / entry-only surface), so
    // the type itself is the validation.
    let float_fields = [
        // Reactor
        ("reactor_capacity", t.reactor_capacity, Positive),
        ("reactor_drain_per_tick", t.reactor_drain_per_tick, NonNeg),
        (
            "reactor_establish_threshold_frac",
            t.reactor_establish_threshold_frac,
            UnitOpen,
        ),
        (
            "reactor_failing_threshold_frac",
            t.reactor_failing_threshold_frac,
            UnitOpen,
        ),
        ("reactor_ignition_seed", t.reactor_ignition_seed, NonNeg),
        ("reactor_baseline_draw", t.reactor_baseline_draw, NonNeg),
        // Feeding / salvage / crew
        ("feed_cost_scrap", t.feed_cost_scrap, NonNeg),
        ("feed_charge", t.feed_charge, NonNeg),
        ("salvage_yield", t.salvage_yield, NonNeg),
        ("crew_source_cost", t.crew_source_cost, NonNeg),
        ("crew_trickle", t.crew_trickle, NonNeg),
        // M2 — power budget & economy
        ("fuel_per_output", t.fuel_per_output, NonNeg),
        ("reactor_base_max_output", t.reactor_base_max_output, NonNeg),
        ("resource_base_cap", t.resource_base_cap, NonNeg),
        ("crew_ration_upkeep", t.crew_ration_upkeep, NonNeg),
        ("starving_efficiency", t.starving_efficiency, UnitOpen),
        // M3 — life support
        (
            "atmosphere_draw_per_crew",
            t.atmosphere_draw_per_crew,
            NonNeg,
        ),
        ("thermal_draw", t.thermal_draw, NonNeg),
        ("thermal_failure_drain", t.thermal_failure_drain, NonNeg),
        // M4 — galaxy
        ("galaxy_radius", t.galaxy_radius, NonNeg),
        ("galaxy_reveal_radius", t.galaxy_reveal_radius, NonNeg),
        (
            "galaxy_min_separation_frac",
            t.galaxy_min_separation_frac,
            Positive,
        ),
        ("galaxy_frontier_reach", t.galaxy_frontier_reach, NonNeg),
        ("star_weight_dwarf", t.star_weight_dwarf, NonNeg),
        ("star_weight_mainseq", t.star_weight_mainseq, NonNeg),
        ("star_weight_neutron", t.star_weight_neutron, NonNeg),
        ("star_weight_dead", t.star_weight_dead, NonNeg),
        ("star_weight_giant", t.star_weight_giant, NonNeg),
        // M4 — ship / jump / scan / sites
        ("ship_base_drive_range", t.ship_base_drive_range, Positive),
        ("ship_cargo", t.ship_cargo, Positive),
        ("ship_base_hull", t.ship_base_hull, NonNeg),
        ("jump_base_cost", t.jump_base_cost, NonNeg),
        ("jump_drive_factor", t.jump_drive_factor, NonNeg),
        ("transit_ticks_factor", t.transit_ticks_factor, NonNeg),
        ("scan_data_cost", t.scan_data_cost, NonNeg),
        ("site_asteroid_isotopes", t.site_asteroid_isotopes, NonNeg),
        ("site_asteroid_fuel", t.site_asteroid_fuel, NonNeg),
        (
            "site_derelict_components",
            t.site_derelict_components,
            NonNeg,
        ),
        ("site_derelict_data", t.site_derelict_data, NonNeg),
        ("reward_gradient", t.reward_gradient, NonNeg),
        ("drive_upgrade_isotopes", t.drive_upgrade_isotopes, NonNeg),
        ("drive_upgrade_step", t.drive_upgrade_step, NonNeg),
        ("scavenge_fuel", t.scavenge_fuel, NonNeg),
        // M5 — worlds & surface
        ("planet_site_weight", t.planet_site_weight, NonNeg),
        ("suit_max", t.suit_max, Positive),
        ("suit_drain_per_tick", t.suit_drain_per_tick, NonNeg),
        ("suit_hazard_damage", t.suit_hazard_damage, NonNeg),
        ("surface_cache_base", t.surface_cache_base, NonNeg),
        ("surface_ruin_data", t.surface_ruin_data, NonNeg),
        ("surface_life_harvest", t.surface_life_harvest, NonNeg),
        ("surface_life_leave_data", t.surface_life_leave_data, NonNeg),
        ("surface_depth_reward", t.surface_depth_reward, NonNeg),
        // CB1 — combat
        ("marine_hp", t.marine_hp, NonNeg),
        ("marine_atk", t.marine_atk, NonNeg),
        ("marine_def_base", t.marine_def_base, NonNeg),
        ("marine_ration_upkeep", t.marine_ration_upkeep, NonNeg),
        ("morale_max", t.morale_max, Positive),
        ("morale_floor", t.morale_floor, UnitOpen),
        ("morale_rout", t.morale_rout, NonNeg),
        ("morale_per_death", t.morale_per_death, NonNeg),
        ("morale_per_flee", t.morale_per_flee, NonNeg),
        ("morale_per_starve_tick", t.morale_per_starve_tick, NonNeg),
        ("morale_recover_per_tick", t.morale_recover_per_tick, NonNeg),
        ("starve_hp_per_tick", t.starve_hp_per_tick, NonNeg),
        ("weapon_step", t.weapon_step, NonNeg),
        ("armor_hp_step", t.armor_hp_step, NonNeg),
        ("armor_def_step", t.armor_def_step, NonNeg),
        ("medgel_heal", t.medgel_heal, NonNeg),
        ("grenade_damage", t.grenade_damage, NonNeg),
        ("shield_def", t.shield_def, NonNeg),
        ("stance_atk_mult", t.stance_atk_mult, NonNeg),
        ("stance_def_bonus", t.stance_def_bonus, NonNeg),
        ("remnant_hp", t.remnant_hp, NonNeg),
        ("remnant_atk", t.remnant_atk, NonNeg),
        ("remnant_def", t.remnant_def, NonNeg),
        ("remnant_speed", t.remnant_speed, NonNeg),
        ("remnant_components", t.remnant_components, NonNeg),
        ("remnant_data", t.remnant_data, NonNeg),
        ("combat_depth_mult", t.combat_depth_mult, NonNeg),
        ("flee_satchel_loss_frac", t.flee_satchel_loss_frac, NonNeg),
        // CB1 — gear/consumable crafting costs
        ("weapon_upgrade_alloy", t.weapon_upgrade_alloy, NonNeg),
        (
            "weapon_upgrade_components",
            t.weapon_upgrade_components,
            NonNeg,
        ),
        ("armor_upgrade_alloy", t.armor_upgrade_alloy, NonNeg),
        (
            "armor_upgrade_components",
            t.armor_upgrade_components,
            NonNeg,
        ),
        ("medgel_cost", t.medgel_cost, NonNeg),
        ("grenade_cost", t.grenade_cost, NonNeg),
        ("shield_cost", t.shield_cost, NonNeg),
        // CB2 — aliens & disposition (band thresholds and start dispositions
        // may legally be negative: finite-only, ordering asserted below)
        ("disp_hostile", t.disp_hostile, Finite),
        ("disp_neutral", t.disp_neutral, Finite),
        ("disp_curious", t.disp_curious, Finite),
        ("fauna_start_disp", t.fauna_start_disp, Finite),
        ("peer_start_disp", t.peer_start_disp, Finite),
        ("disp_spread", t.disp_spread, NonNeg),
        ("parley_shift", t.parley_shift, NonNeg),
        ("gift_shift", t.gift_shift, NonNeg),
        ("gift_cost", t.gift_cost, NonNeg),
        ("trade_give", t.trade_give, NonNeg),
        ("trade_get", t.trade_get, NonNeg),
        ("trade_disp_shift", t.trade_disp_shift, NonNeg),
        ("observe_data", t.observe_data, NonNeg),
        ("observe_shift", t.observe_shift, NonNeg),
        ("harvest_fauna_yield", t.harvest_fauna_yield, NonNeg),
        ("fauna_hp", t.fauna_hp, NonNeg),
        ("fauna_atk", t.fauna_atk, NonNeg),
        ("fauna_def", t.fauna_def, NonNeg),
        ("peer_hp", t.peer_hp, NonNeg),
        ("peer_atk", t.peer_atk, NonNeg),
        ("peer_def", t.peer_def, NonNeg),
        ("peer_gear_mirror", t.peer_gear_mirror, NonNeg),
        ("peer_salvage", t.peer_salvage, NonNeg),
        ("fauna_salvage", t.fauna_salvage, NonNeg),
        ("peace_band", t.peace_band, NonNeg),
        // CB3 — ships in the dark
        ("contact_chance", t.contact_chance, UnitClosed),
        ("contact_peer_share", t.contact_peer_share, UnitClosed),
        ("ship_peer_start_disp", t.ship_peer_start_disp, NonNeg),
        ("ship_disp_spread", t.ship_disp_spread, NonNeg),
        ("ship_base_atk", t.ship_base_atk, NonNeg),
        ("ship_base_def", t.ship_base_def, NonNeg),
        ("ship_speed", t.ship_speed, NonNeg),
        ("ship_weapon_step", t.ship_weapon_step, NonNeg),
        ("hull_plating_def_step", t.hull_plating_def_step, NonNeg),
        ("hull_plating_hp_step", t.hull_plating_hp_step, NonNeg),
        ("peer_ship_hp", t.peer_ship_hp, NonNeg),
        ("peer_ship_atk", t.peer_ship_atk, NonNeg),
        ("peer_ship_def", t.peer_ship_def, NonNeg),
        ("peer_ship_mirror", t.peer_ship_mirror, NonNeg),
        ("remnant_ship_hp", t.remnant_ship_hp, NonNeg),
        ("remnant_ship_atk", t.remnant_ship_atk, NonNeg),
        ("remnant_ship_def", t.remnant_ship_def, NonNeg),
        ("ship_enemy_speed", t.ship_enemy_speed, NonNeg),
        ("ship_salvage_components", t.ship_salvage_components, NonNeg),
        ("ship_salvage_isotopes", t.ship_salvage_isotopes, NonNeg),
        ("ship_salvage_data", t.ship_salvage_data, NonNeg),
        ("repair_drone_heal", t.repair_drone_heal, NonNeg),
        ("repair_drone_cost", t.repair_drone_cost, NonNeg),
        (
            "ship_weapon_upgrade_alloy",
            t.ship_weapon_upgrade_alloy,
            NonNeg,
        ),
        (
            "ship_weapon_upgrade_components",
            t.ship_weapon_upgrade_components,
            NonNeg,
        ),
        (
            "hull_plating_upgrade_alloy",
            t.hull_plating_upgrade_alloy,
            NonNeg,
        ),
        (
            "hull_plating_upgrade_components",
            t.hull_plating_upgrade_components,
            NonNeg,
        ),
        ("hull_regen_docked", t.hull_regen_docked, NonNeg),
        ("retreat_cargo_loss", t.retreat_cargo_loss, UnitClosed),
        ("hull_retreat_floor", t.hull_retreat_floor, NonNeg),
        // M6a — the turn & endings
        ("expansion_w_peers", t.expansion_w_peers, NonNeg),
        ("expansion_w_strip", t.expansion_w_strip, NonNeg),
        ("expansion_w_consume", t.expansion_w_consume, NonNeg),
        ("expansion_w_systems", t.expansion_w_systems, NonNeg),
        ("watched_threshold", t.watched_threshold, NonNeg),
        ("communion_min_spared", t.communion_min_spared, NonNeg),
        ("communion_max_kills", t.communion_max_kills, NonNeg),
        ("pattern_max_kills", t.pattern_max_kills, NonNeg),
        // M6b — word-drift
        ("drift_t1", t.drift_t1, NonNeg),
        ("drift_t2", t.drift_t2, NonNeg),
        ("drift_t3", t.drift_t3, NonNeg),
    ];
    for (name, v, check) in float_fields {
        if !check.ok(v) {
            return Err(ContentError::Invalid(format!(
                "{name} must be {}",
                check.expects()
            )));
        }
    }

    // Integer fields that must be >= 1.
    for (name, v) in [
        ("crew_cap", u64::from(t.crew_cap)),
        ("arrival_interval", u64::from(t.arrival_interval)),
        ("log_capacity", t.log_capacity as u64),
        ("base_housing", u64::from(t.base_housing)),
        ("atmosphere_grace_max", u64::from(t.atmosphere_grace_max)),
        ("crew_loss_interval", u64::from(t.crew_loss_interval)),
        ("transit_ticks_base", u64::from(t.transit_ticks_base)),
        ("scan_ticks", u64::from(t.scan_ticks)),
        ("site_resolve_ticks", u64::from(t.site_resolve_ticks)),
        ("scavenge_ticks", u64::from(t.scavenge_ticks)),
        ("traverse_ticks", u64::from(t.traverse_ticks)),
        ("shield_ticks", u64::from(t.shield_ticks)),
    ] {
        if v < 1 {
            return Err(ContentError::Invalid(format!("{name} must be >= 1")));
        }
    }

    // Relational / cross-field checks.
    if t.reactor_failing_threshold_frac > t.reactor_establish_threshold_frac {
        return Err(ContentError::Invalid(
            "reactor_failing_threshold_frac must be <= reactor_establish_threshold_frac".into(),
        ));
    }
    if t.galaxy_system_count < 2 {
        return Err(ContentError::Invalid(
            "galaxy_system_count must be >= 2".into(),
        ));
    }
    if t.galaxy_inner_count < 2 || t.galaxy_inner_count > t.galaxy_system_count {
        return Err(ContentError::Invalid(
            "galaxy_inner_count must be in [2, galaxy_system_count]".into(),
        ));
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
    if t.surface_layer_min < 1 || t.surface_layer_max < t.surface_layer_min {
        return Err(ContentError::Invalid(
            "surface_layer_max must be >= surface_layer_min >= 1".into(),
        ));
    }
    // M6b — word-drift tier ordering
    if !(t.drift_t1 < t.drift_t2 && t.drift_t2 < t.drift_t3) {
        return Err(ContentError::Invalid(
            "word-drift tiers must satisfy drift_t1 < drift_t2 < drift_t3".into(),
        ));
    }
    // CB2 — disposition band ordering
    if !(t.disp_hostile < t.disp_neutral && t.disp_neutral < t.disp_curious) {
        return Err(ContentError::Invalid(
            "disposition bands must satisfy disp_hostile < disp_neutral < disp_curious".into(),
        ));
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
    validate_jobs(&reg)?;
    Ok(reg)
}

fn validate_jobs(reg: &JobRegistry) -> Result<(), ContentError> {
    let mut seen = std::collections::BTreeSet::new();
    for j in reg.iter() {
        if !seen.insert(j.id) {
            return Err(ContentError::Invalid(format!(
                "jobs.ron: duplicate id {:?}",
                j.id
            )));
        }
    }
    for j in reg.iter() {
        if !j.power_draw.is_finite() || j.power_draw < 0.0 {
            return Err(ContentError::Invalid(format!(
                "{:?}: power_draw must be finite >= 0",
                j.id
            )));
        }
        for (label, list) in [("inputs", &j.recipe.inputs), ("outputs", &j.recipe.outputs)] {
            if list.iter().any(|&(_, a)| !a.is_finite() || a < 0.0) {
                return Err(ContentError::Invalid(format!(
                    "{:?}: recipe {label} amounts must be finite >= 0",
                    j.id
                )));
            }
        }
    }
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
    Ok(())
}

pub fn load_modules() -> Result<ModuleRegistry, ContentError> {
    let mods: Vec<sim::module::Module> =
        ron::from_str(MODULES_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    let reg = ModuleRegistry(mods);
    validate_modules(&reg)?;
    Ok(reg)
}

fn validate_modules(reg: &ModuleRegistry) -> Result<(), ContentError> {
    let mut seen_ids = std::collections::BTreeSet::new();
    for m in reg.iter() {
        if !seen_ids.insert(m.id) {
            return Err(ContentError::Invalid(format!(
                "modules.ron: duplicate id {:?}",
                m.id
            )));
        }
    }
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
    Ok(())
}

pub fn load_techs() -> Result<TechRegistry, ContentError> {
    let techs: Vec<sim::research::Tech> =
        ron::from_str(TECHS_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    let reg = TechRegistry(techs);
    validate_techs(&reg)?;
    Ok(reg)
}

fn validate_techs(reg: &TechRegistry) -> Result<(), ContentError> {
    let mut seen = std::collections::BTreeSet::new();
    for t in reg.iter() {
        if !seen.insert(t.id) {
            return Err(ContentError::Invalid(format!(
                "techs.ron: duplicate id {:?}",
                t.id
            )));
        }
    }
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
        if has_cycle(reg, t.id, &mut visiting, &mut done) {
            return Err(ContentError::Invalid(format!(
                "tech requires graph has a cycle at {:?}",
                t.id
            )));
        }
    }
    Ok(())
}

pub fn load_origins() -> Result<Vec<OriginDef>, ContentError> {
    let origins: Vec<OriginDef> =
        ron::from_str(ORIGINS_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    validate_origins(&origins)?;
    Ok(origins)
}

fn validate_origins(origins: &[OriginDef]) -> Result<(), ContentError> {
    if origins.is_empty() {
        return Err(ContentError::Invalid("no origins defined".into()));
    }
    // Origin has no Ord/Hash; a linear scan over <=3 variants is fine.
    let mut seen: Vec<sim::origin::Origin> = Vec::new();
    for o in origins {
        if seen.contains(&o.id) {
            return Err(ContentError::Invalid(format!(
                "origins.ron: duplicate id {:?}",
                o.id
            )));
        }
        seen.push(o.id);
    }
    for o in origins {
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
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiomeDef {
    pub biome: Biome,
    pub blurb: String,
}

pub fn load_biomes() -> Result<Vec<BiomeDef>, ContentError> {
    let defs: Vec<BiomeDef> =
        ron::from_str(BIOMES_RON).map_err(|e| ContentError::Parse(e.to_string()))?;
    validate_biomes(&defs)?;
    Ok(defs)
}

fn validate_biomes(defs: &[BiomeDef]) -> Result<(), ContentError> {
    // Biome has no Ord/Hash; a linear scan over 7 variants is fine.
    let mut seen: Vec<Biome> = Vec::new();
    for d in defs {
        if seen.contains(&d.biome) {
            return Err(ContentError::Invalid(format!(
                "biomes.ron: duplicate biome {:?}",
                d.biome
            )));
        }
        seen.push(d.biome);
    }
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
    Ok(())
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
        assert_eq!(t.fuel_per_output, 0.2);
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
        assert_eq!(t.galaxy_system_count, 16);
        assert_eq!(t.galaxy_inner_count, 11);
        assert_eq!(t.galaxy_radius, 100.0);
        assert_eq!(t.galaxy_reveal_radius, 45.0);
        assert_eq!(t.galaxy_min_separation_frac, 0.4);
        assert_eq!(t.galaxy_frontier_reach, 25.0);
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

    #[test]
    fn loads_m6b_tuning() {
        let t = load().unwrap();
        assert_eq!(t.drift_t1, 30.0);
        assert_eq!(t.drift_t2, 60.0);
        assert_eq!(t.drift_t3, 100.0);
    }

    #[test]
    fn misspelled_key_is_a_parse_error() {
        // `require_flag` (missing the s) must be a hard parse error via
        // deny_unknown_fields — not a silently-defaulted `requires_flag: None`,
        // which would ship a gated module ungated.
        let bad = r#"[(
            id: Armory,
            name: "Armory",
            description: "d",
            tier: 1,
            cost: [],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: [],
            require_flag: Some(CombatDoctrineKnown),
            unlocks: (),
        )]"#;
        assert!(ron::from_str::<Vec<sim::module::Module>>(bad).is_err());
        // Control: the correctly-spelled key parses.
        let good = bad.replace("require_flag:", "requires_flag:");
        assert!(ron::from_str::<Vec<sim::module::Module>>(&good).is_ok());
    }

    fn invalid_msg(r: Result<(), ContentError>) -> String {
        match r.unwrap_err() {
            ContentError::Invalid(m) => m,
            ContentError::Parse(m) => panic!("expected Invalid, got Parse({m})"),
        }
    }

    fn sample_job() -> sim::job::Job {
        sim::job::Job {
            id: sim::job::JobId::Salvager,
            name: "Salvager".into(),
            gated_by: None,
            recipe: sim::job::Recipe {
                inputs: vec![],
                outputs: vec![(sim::resources::ResourceKind::Scrap, 1.0)],
            },
            power_draw: 0.0,
            default_priority: 0,
        }
    }

    fn sample_module() -> sim::module::Module {
        sim::module::Module {
            id: sim::module::ModuleId::Refinery,
            name: "Refinery".into(),
            description: "d".into(),
            tier: 1,
            cost: vec![],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: None,
            unlocks: Default::default(),
        }
    }

    fn sample_tech() -> sim::research::Tech {
        sim::research::Tech {
            id: sim::research::TechId::CoreTuning,
            name: "Core Tuning".into(),
            blurb: "b".into(),
            data_cost: 5.0,
            research_ticks: 30,
            requires: vec![],
            grant: sim::research::Grant::ReactorMaxOutput(2.0),
            lore: "l".into(),
        }
    }

    #[test]
    fn duplicate_ids_are_rejected_in_every_registry() {
        // Jobs
        let j = sample_job();
        let msg = invalid_msg(validate_jobs(&sim::job::JobRegistry(vec![j.clone(), j])));
        assert!(msg.contains("duplicate"), "{msg}");
        // Modules
        let m = sample_module();
        let msg = invalid_msg(validate_modules(&sim::module::ModuleRegistry(vec![
            m.clone(),
            m,
        ])));
        assert!(msg.contains("duplicate"), "{msg}");
        // Techs
        let tc = sample_tech();
        let msg = invalid_msg(validate_techs(&sim::research::TechRegistry(vec![
            tc.clone(),
            tc,
        ])));
        assert!(msg.contains("duplicate"), "{msg}");
        // Origins
        let origins = load_origins().unwrap();
        let mut dup = origins.clone();
        dup.push(origins[0].clone());
        let msg = invalid_msg(validate_origins(&dup));
        assert!(msg.contains("duplicate"), "{msg}");
        // Biomes
        let dup = [
            BiomeDef {
                biome: Biome::Frozen,
                blurb: "a".into(),
            },
            BiomeDef {
                biome: Biome::Frozen,
                blurb: "b".into(),
            },
        ];
        let msg = invalid_msg(validate_biomes(&dup));
        assert!(msg.contains("duplicate"), "{msg}");
    }

    #[test]
    fn bad_recipe_amounts_are_rejected() {
        // Negative output
        let mut j = sample_job();
        j.recipe.outputs[0].1 = -1.0;
        let msg = invalid_msg(validate_jobs(&sim::job::JobRegistry(vec![j])));
        assert!(msg.contains("recipe outputs"), "{msg}");
        // NaN input
        let mut j = sample_job();
        j.recipe.inputs = vec![(sim::resources::ResourceKind::Alloy, f64::NAN)];
        let msg = invalid_msg(validate_jobs(&sim::job::JobRegistry(vec![j])));
        assert!(msg.contains("recipe inputs"), "{msg}");
    }

    #[test]
    fn validates_new_tuning_fields() {
        // fauna_start_disp / peer_start_disp: negatives legal, non-finite not.
        let ok = Tuning {
            fauna_start_disp: -25.0,
            peer_start_disp: -5.0,
            ..Tuning::default()
        };
        assert!(validate(&ok).is_ok());
        let bad = Tuning {
            fauna_start_disp: f64::NAN,
            ..Tuning::default()
        };
        assert!(matches!(validate(&bad), Err(ContentError::Invalid(_))));
        let bad = Tuning {
            peer_start_disp: f64::INFINITY,
            ..Tuning::default()
        };
        assert!(matches!(validate(&bad), Err(ContentError::Invalid(_))));
    }
}
