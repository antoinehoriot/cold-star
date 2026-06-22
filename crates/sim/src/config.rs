//! Tunable balance constants. Lives in `sim` so the engine is self-contained;
//! the `content` crate parses these from data (RON) and validates them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    /// Reactor charge lost per tick.
    pub reactor_drain_per_tick: f64,
    /// Charge restored by one StokeCore action.
    pub reactor_stoke_amount: f64,
    /// Maximum (and starting) reactor charge.
    pub reactor_capacity: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            reactor_drain_per_tick: 1.0,
            reactor_stoke_amount: 10.0,
            reactor_capacity: 100.0,
        }
    }
}
