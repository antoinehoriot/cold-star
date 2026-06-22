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
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            reactor_capacity: 100.0,
            reactor_drain_per_tick: 1.0,
            reactor_establish_threshold_frac: 0.5,
            reactor_failing_threshold_frac: 0.2,
            reactor_ignition_seed: 10.0,
            feed_cost_scrap: 10.0,
            feed_charge: 20.0,
            salvage_yield: 5.0,
            salvage_cooldown_ticks: 5,
            crew_source_cost: 30.0,
            crew_cap: 4,
            arrival_interval: 30,
            crew_trickle: 0.5,
            log_capacity: 100,
        }
    }
}
