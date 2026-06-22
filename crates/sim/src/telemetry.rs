//! Last-tick engine telemetry — an OUTPUT-ONLY snapshot the UI reads to render
//! live rates and per-job "dark reasons". Overwritten every tick; never read
//! back by the simulation, so it cannot affect determinism.

use crate::job::JobId;
use crate::priority::Consumer;
use crate::resources::ResourceKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Telemetry {
    /// Reactor Output generated this tick.
    pub generation: f64,
    /// Consumer (job) power draw this tick, excluding the reactor baseline.
    pub consumer_draw: f64,
    /// Total power draw this tick (baseline + active consumers).
    pub active_draw: f64,
    /// Consumers browned out (paused for lack of power) this tick.
    pub paused: BTreeSet<Consumer>,
    /// Jobs that had crew but lacked input materials this tick.
    pub starved_input: BTreeSet<JobId>,
    /// Jobs that executed (consumed + produced) this tick.
    pub ran: BTreeSet<JobId>,
    /// Net change in each resource over this tick (after − before).
    pub net: BTreeMap<ResourceKind, f64>,
    /// True if crew Rations upkeep could not be paid this tick.
    pub starving: bool,
}
