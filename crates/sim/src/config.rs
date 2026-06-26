//! Tunable balance constants. Lives in `sim` so the engine is self-contained;
//! the `content` crate parses these from data (RON) and validates them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    // Reactor
    /// Maximum (and default starting) reactor charge.
    pub reactor_capacity: f64,
    /// Charge lost per tick.
    pub reactor_drain_per_tick: f64,
    /// Fraction of capacity at which the core becomes "established" (-> Stable).
    pub reactor_establish_threshold_frac: f64,
    /// Fraction of capacity below which an established core is "Failing".
    pub reactor_failing_threshold_frac: f64,
    /// Charge granted by igniting a Cold core.
    pub reactor_ignition_seed: f64,
    /// Mandatory baseline draw the reactor must always cover; drains charge when unfuelled.
    pub reactor_baseline_draw: f64,

    // Feeding / salvage
    /// Scrap consumed by one FeedCore action.
    pub feed_cost_scrap: f64,
    /// Charge added by one FeedCore action.
    pub feed_charge: f64,
    /// Scrap gained by one Salvage action.
    pub salvage_yield: f64,
    /// Ticks the Salvage action is on cooldown after each use (0 = no cooldown).
    pub salvage_cooldown_ticks: u32,

    // Crew
    /// Scrap cost to activate the crew-source.
    pub crew_source_cost: f64,
    /// Maximum crew in M1 (no housing module yet).
    pub crew_cap: u32,
    /// Ticks between crew arrivals once the source is active.
    pub arrival_interval: u32,
    /// Scrap produced per crew per tick (auto-salvage trickle).
    pub crew_trickle: f64,

    // Log
    /// Max retained event-log entries (oldest dropped).
    pub log_capacity: usize,

    // M2 — power budget
    /// Fuel burned per unit of reactor Output generated.
    pub fuel_per_output: f64,
    /// Base reactor max Output (raised by Reactor Upgrade modules).
    pub reactor_base_max_output: f64,
    // M2 — economy
    /// Base storage cap applied to every resource (raised by Storage Hold).
    pub resource_base_cap: f64,
    /// Rations consumed per crew per tick.
    pub crew_ration_upkeep: f64,
    /// Output multiplier applied to all jobs while crew are starving.
    pub starving_efficiency: f64,
    /// Base crew housing cap (raised by Quarters).
    pub base_housing: u32,

    // M3 — life support
    /// Atmosphere power draw per crew per tick.
    pub atmosphere_draw_per_crew: f64,
    /// Ticks the atmosphere survival clock lasts while unpowered before crew die.
    pub atmosphere_grace_max: u32,
    /// Ticks between crew deaths once the atmosphere clock is exhausted.
    pub crew_loss_interval: u32,
    /// Thermal system flat power draw per tick.
    pub thermal_draw: f64,
    /// Extra reactor charge lost per tick while thermal is built but unpowered.
    pub thermal_failure_drain: f64,

    // M4 — galaxy
    pub galaxy_system_count: u32,
    pub galaxy_radius: f64,
    pub galaxy_reveal_radius: f64,
    pub star_weight_dwarf: f64,
    pub star_weight_mainseq: f64,
    pub star_weight_neutron: f64,
    pub star_weight_dead: f64,
    pub star_weight_giant: f64,
    // M4 — ship
    pub ship_base_drive_range: f64,
    pub ship_cargo: f64,
    pub ship_base_hull: f64,
    // M4 — jump / transit
    pub jump_base_cost: f64,
    pub jump_drive_factor: f64,
    pub transit_ticks_base: u32,
    pub transit_ticks_factor: f64,
    // M4 — scan
    pub scan_data_cost: f64,
    pub scan_ticks: u32,
    // M4 — sites
    pub site_resolve_ticks: u32,
    pub site_asteroid_isotopes: f64,
    pub site_asteroid_fuel: f64,
    pub site_derelict_components: f64,
    pub site_derelict_data: f64,
    pub reward_gradient: f64,
    // M4 — drive upgrade
    pub drive_upgrade_isotopes: f64,
    pub drive_upgrade_step: f64,
    // M4 — scavenge (soft stranded recovery)
    pub scavenge_ticks: u32,
    pub scavenge_fuel: f64,

    // M5 — worlds & surface
    pub planet_site_weight: f64, // extra weighted chance a site roll yields a Planet
    pub suit_max: f64,
    pub suit_drain_per_tick: f64,
    pub suit_hazard_damage: f64,
    pub traverse_ticks: u32,
    pub surface_depth_max: u32,  // deepest layer index (entry = 0)
    pub surface_layer_min: u32,  // min nodes per non-entry layer
    pub surface_layer_max: u32,  // max nodes per non-entry layer
    pub surface_cache_base: f64, // base ResourceCache yield (scaled by depth)
    pub surface_ruin_data: f64,
    pub surface_life_harvest: f64,
    pub surface_life_leave_data: f64,
    pub surface_depth_reward: f64, // per-depth multiplier added to yields

    // CB1 — combat
    pub marine_hp: f64,
    pub marine_atk: f64,
    pub marine_def_base: f64,
    pub marine_ration_upkeep: f64,
    pub morale_max: f64,
    pub morale_floor: f64,
    pub morale_rout: f64,
    pub morale_per_death: f64,
    pub morale_per_flee: f64,
    pub morale_per_starve_tick: f64,
    pub morale_recover_per_tick: f64,
    pub starve_hp_per_tick: f64,
    pub weapon_step: f64,
    pub armor_hp_step: f64,
    pub armor_def_step: f64,
    pub medgel_heal: f64,
    pub grenade_damage: f64,
    pub shield_def: f64,
    pub shield_ticks: u32,
    pub stance_atk_mult: f64,
    pub stance_def_bonus: f64,
    pub remnant_hp: f64,
    pub remnant_atk: f64,
    pub remnant_def: f64,
    pub remnant_speed: f64,
    pub remnant_components: f64,
    pub remnant_data: f64,
    pub combat_depth_mult: f64,
    pub flee_satchel_loss_frac: f64,
    // CB1 — gear/consumable crafting costs
    pub weapon_upgrade_alloy: f64,
    pub weapon_upgrade_components: f64,
    pub armor_upgrade_alloy: f64,
    pub armor_upgrade_components: f64,
    pub medgel_cost: f64,
    pub grenade_cost: f64,
    pub shield_cost: f64,

    // CB2 — aliens & disposition
    /// Disposition threshold below which an entity is Hostile.
    pub disp_hostile: f64,
    /// Disposition threshold below which an entity is Neutral (above disp_hostile).
    pub disp_neutral: f64,
    /// Disposition threshold at/above which an entity is Curious/Friendly.
    pub disp_curious: f64,
    /// Starting disposition for fauna encounters.
    pub fauna_start_disp: f64,
    /// Starting disposition for peer (alien crew) encounters.
    pub peer_start_disp: f64,
    /// Seeded random range applied around the start disposition.
    pub disp_spread: f64,
    /// Disposition shift from a successful parley action.
    pub parley_shift: f64,
    /// Disposition shift from offering a gift.
    pub gift_shift: f64,
    /// Resource cost of a gift action.
    pub gift_cost: f64,
    /// Resources given per trade offer.
    pub trade_give: f64,
    /// Resources received per trade offer.
    pub trade_get: f64,
    /// Disposition shift from completing a trade.
    pub trade_disp_shift: f64,
    /// Data gained from observing an alien entity.
    pub observe_data: f64,
    /// Disposition shift from observing (non-hostile).
    pub observe_shift: f64,
    /// Yield from harvesting fauna.
    pub harvest_fauna_yield: f64,
    /// Base HP for fauna combatants.
    pub fauna_hp: f64,
    /// Base attack for fauna combatants.
    pub fauna_atk: f64,
    /// Base defence for fauna combatants.
    pub fauna_def: f64,
    /// Base HP for peer combatants.
    pub peer_hp: f64,
    /// Base attack for peer combatants.
    pub peer_atk: f64,
    /// Base defence for peer combatants.
    pub peer_def: f64,
    /// How strongly a peer scales with the player's weapon tier (0–1).
    pub peer_gear_mirror: f64,
    /// Salvage yield from defeating a peer.
    pub peer_salvage: f64,
    /// Salvage yield from defeating fauna.
    pub fauna_salvage: f64,
    /// Disposition at/above which a Peer is spared on parley (= disp_curious).
    pub peace_band: f64,

    // CB3 — ships in the dark
    /// P(a Contact event on system arrival).
    pub contact_chance: f64,
    /// Of Contact events, fraction that are Peers (vs. Remnants).
    pub contact_peer_share: f64,
    /// Starting disposition for peer ship encounters.
    pub ship_peer_start_disp: f64,
    /// Seeded random range around ship encounter start disposition.
    pub ship_disp_spread: f64,
    /// Base attack for the player's ship.
    pub ship_base_atk: f64,
    /// Base defence for the player's ship.
    pub ship_base_def: f64,
    /// Player ship speed (initiative factor).
    pub ship_speed: f64,
    /// Attack multiplier per weapon tier (ship weapons).
    pub ship_weapon_step: f64,
    /// Defence added per hull plating tier.
    pub hull_plating_def_step: f64,
    /// Max-hull added per hull plating tier.
    pub hull_plating_hp_step: f64,
    /// Base HP for peer ship combatants.
    pub peer_ship_hp: f64,
    /// Base attack for peer ship combatants.
    pub peer_ship_atk: f64,
    /// Base defence for peer ship combatants.
    pub peer_ship_def: f64,
    /// How strongly a peer ship scales with the player's ship tier (0–1).
    pub peer_ship_mirror: f64,
    /// Base HP for remnant ship combatants.
    pub remnant_ship_hp: f64,
    /// Base attack for remnant ship combatants.
    pub remnant_ship_atk: f64,
    /// Base defence for remnant ship combatants.
    pub remnant_ship_def: f64,
    /// Enemy ship speed (initiative factor).
    pub ship_enemy_speed: f64,
    /// Components salvaged after destroying an enemy ship.
    pub ship_salvage_components: f64,
    /// Isotopes salvaged after destroying an enemy ship.
    pub ship_salvage_isotopes: f64,
    /// Data salvaged after destroying an enemy ship.
    pub ship_salvage_data: f64,
    /// Hull restored by one Repair Drone consumable.
    pub repair_drone_heal: f64,
    /// Components cost to craft one Repair Drone.
    pub repair_drone_cost: f64,
    /// Alloy cost to upgrade ship weapons one tier.
    pub ship_weapon_upgrade_alloy: f64,
    /// Components cost to upgrade ship weapons one tier.
    pub ship_weapon_upgrade_components: f64,
    /// Alloy cost to upgrade hull plating one tier.
    pub hull_plating_upgrade_alloy: f64,
    /// Components cost to upgrade hull plating one tier.
    pub hull_plating_upgrade_components: f64,
    /// Hull restored per tick while docked at home.
    pub hull_regen_docked: f64,
    /// Fraction of cargo lost when retreating from ship combat.
    pub retreat_cargo_loss: f64,
    /// Minimum hull remaining after a retreat (floor, not killed).
    pub hull_retreat_floor: f64,

    // M6a — the turn & endings
    /// Scoring weight for peer encounters (expansion scoring).
    pub expansion_w_peers: f64,
    /// Scoring weight for strip-mining systems (expansion scoring).
    pub expansion_w_strip: f64,
    /// Scoring weight for resource consumption (expansion scoring).
    pub expansion_w_consume: f64,
    /// Scoring weight for systems visited (expansion scoring).
    pub expansion_w_systems: f64,
    /// Expansion score above which the player is "watched" by the Communion.
    pub watched_threshold: f64,
    /// Minimum peers spared for the Communion ending to be available.
    pub communion_min_spared: f64,
    /// Maximum kills allowed for the Communion ending to remain available.
    pub communion_max_kills: f64,
    /// Maximum kills allowed for the Pattern ending to remain available.
    pub pattern_max_kills: f64,

    // M6b — word-drift
    /// Expansion score threshold for tier 1 word-drift.
    pub drift_t1: f64,
    /// Expansion score threshold for tier 2 word-drift.
    pub drift_t2: f64,
    /// Expansion score threshold for tier 3 word-drift.
    pub drift_t3: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            reactor_capacity: 100.0,
            reactor_drain_per_tick: 1.0,
            reactor_establish_threshold_frac: 0.5,
            reactor_failing_threshold_frac: 0.2,
            reactor_ignition_seed: 10.0,
            reactor_baseline_draw: 1.0,
            feed_cost_scrap: 10.0,
            feed_charge: 20.0,
            salvage_yield: 5.0,
            salvage_cooldown_ticks: 5,
            crew_source_cost: 30.0,
            crew_cap: 4,
            arrival_interval: 30,
            crew_trickle: 0.5,
            log_capacity: 100,
            fuel_per_output: 0.2,
            reactor_base_max_output: 5.0,
            resource_base_cap: 100.0,
            crew_ration_upkeep: 0.1,
            starving_efficiency: 0.5,
            base_housing: 4,
            atmosphere_draw_per_crew: 0.2,
            atmosphere_grace_max: 30,
            crew_loss_interval: 10,
            thermal_draw: 1.0,
            thermal_failure_drain: 2.0,
            galaxy_system_count: 10,
            galaxy_radius: 100.0,
            galaxy_reveal_radius: 45.0,
            star_weight_dwarf: 4.0,
            star_weight_mainseq: 3.0,
            star_weight_neutron: 2.0,
            star_weight_dead: 2.0,
            star_weight_giant: 1.5,
            ship_base_drive_range: 40.0,
            ship_cargo: 60.0,
            ship_base_hull: 120.0,
            jump_base_cost: 5.0,
            jump_drive_factor: 0.2,
            transit_ticks_base: 8,
            transit_ticks_factor: 0.1,
            scan_data_cost: 4.0,
            scan_ticks: 10,
            site_resolve_ticks: 12,
            site_asteroid_isotopes: 8.0,
            site_asteroid_fuel: 4.0,
            site_derelict_components: 6.0,
            site_derelict_data: 5.0,
            reward_gradient: 0.01,
            drive_upgrade_isotopes: 10.0,
            drive_upgrade_step: 15.0,
            scavenge_ticks: 20,
            scavenge_fuel: 6.0,
            planet_site_weight: 0.5,
            suit_max: 100.0,
            suit_drain_per_tick: 0.6,
            suit_hazard_damage: 18.0,
            traverse_ticks: 4,
            surface_depth_max: 3,
            surface_layer_min: 1,
            surface_layer_max: 3,
            surface_cache_base: 14.0,
            surface_ruin_data: 10.0,
            surface_life_harvest: 22.0,
            surface_life_leave_data: 6.0,
            surface_depth_reward: 0.6,
            // CB1 — combat
            marine_hp: 30.0,
            marine_atk: 6.0,
            marine_def_base: 2.0,
            marine_ration_upkeep: 0.15,
            morale_max: 100.0,
            morale_floor: 0.4,
            morale_rout: 20.0,
            morale_per_death: 25.0,
            morale_per_flee: 10.0,
            morale_per_starve_tick: 1.0,
            morale_recover_per_tick: 0.8,
            starve_hp_per_tick: 1.5,
            weapon_step: 0.5,
            armor_hp_step: 0.4,
            armor_def_step: 2.0,
            medgel_heal: 25.0,
            grenade_damage: 40.0,
            shield_def: 6.0,
            shield_ticks: 5,
            stance_atk_mult: 1.5,
            stance_def_bonus: 3.0,
            remnant_hp: 60.0,
            remnant_atk: 7.0,
            remnant_def: 3.0,
            remnant_speed: 1.0,
            remnant_components: 8.0,
            remnant_data: 6.0,
            combat_depth_mult: 0.5,
            flee_satchel_loss_frac: 0.25,
            // CB1 — gear/consumable crafting costs
            weapon_upgrade_alloy: 20.0,
            weapon_upgrade_components: 10.0,
            armor_upgrade_alloy: 18.0,
            armor_upgrade_components: 8.0,
            medgel_cost: 5.0,
            grenade_cost: 8.0,
            shield_cost: 6.0,
            // CB2 — aliens & disposition
            disp_hostile: -40.0,
            disp_neutral: -10.0,
            disp_curious: 30.0,
            fauna_start_disp: -25.0,
            peer_start_disp: 0.0,
            disp_spread: 40.0,
            parley_shift: 18.0,
            gift_shift: 35.0,
            gift_cost: 10.0,
            trade_give: 15.0,
            trade_get: 10.0,
            trade_disp_shift: 6.0,
            observe_data: 6.0,
            observe_shift: 8.0,
            harvest_fauna_yield: 16.0,
            fauna_hp: 35.0,
            fauna_atk: 5.0,
            fauna_def: 2.0,
            peer_hp: 70.0,
            peer_atk: 8.0,
            peer_def: 4.0,
            peer_gear_mirror: 0.5,
            peer_salvage: 12.0,
            fauna_salvage: 8.0,
            peace_band: 30.0,
            // CB3 — ships in the dark
            contact_chance: 0.35,
            contact_peer_share: 0.6,
            ship_peer_start_disp: 0.0,
            ship_disp_spread: 40.0,
            ship_base_atk: 10.0,
            ship_base_def: 3.0,
            ship_speed: 1.0,
            ship_weapon_step: 0.6,
            hull_plating_def_step: 2.0,
            hull_plating_hp_step: 40.0,
            peer_ship_hp: 90.0,
            peer_ship_atk: 11.0,
            peer_ship_def: 4.0,
            peer_ship_mirror: 0.5,
            remnant_ship_hp: 70.0,
            remnant_ship_atk: 9.0,
            remnant_ship_def: 3.0,
            ship_enemy_speed: 1.0,
            ship_salvage_components: 20.0,
            ship_salvage_isotopes: 10.0,
            ship_salvage_data: 8.0,
            repair_drone_heal: 35.0,
            repair_drone_cost: 12.0,
            ship_weapon_upgrade_alloy: 20.0,
            ship_weapon_upgrade_components: 15.0,
            hull_plating_upgrade_alloy: 20.0,
            hull_plating_upgrade_components: 15.0,
            hull_regen_docked: 3.0,
            retreat_cargo_loss: 0.4,
            hull_retreat_floor: 10.0,
            // M6a — the turn & endings
            expansion_w_peers: 8.0,
            expansion_w_strip: 1.0,
            expansion_w_consume: 0.02,
            expansion_w_systems: 4.0,
            watched_threshold: 120.0,
            communion_min_spared: 3.0,
            communion_max_kills: 1.0,
            pattern_max_kills: 2.0,
            // M6b — word-drift
            drift_t1: 30.0,
            drift_t2: 60.0,
            drift_t3: 100.0,
        }
    }
}
