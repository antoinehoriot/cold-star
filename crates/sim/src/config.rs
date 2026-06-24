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
            fuel_per_output: 0.5,
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
            ship_base_drive_range: 40.0,
            ship_cargo: 60.0,
            ship_base_hull: 10.0,
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
        }
    }
}
