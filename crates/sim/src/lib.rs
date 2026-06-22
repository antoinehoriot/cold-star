//! Cold Star simulation core. Pure, deterministic, platform-agnostic.
#![forbid(unsafe_code)]

pub mod config;
pub mod crew;
pub mod economy;
pub mod flags;
pub mod job;
pub mod log;
pub mod module;
pub mod origin;
pub mod power;
pub mod priority;
pub mod reactor;
pub mod resources;
pub mod rng;
pub mod save;
pub mod state;
pub mod time;

pub use config::Tuning;
pub use crew::Crew;
pub use economy::{EconomyReport, job_ready, run_economy};
pub use flags::Flag;
pub use job::{Job, JobId, JobRegistry, Recipe};
pub use log::{Log, LogEntry};
pub use module::{Module, ModuleId, ModuleRegistry, ModuleUnlocks};
pub use origin::{Origin, OriginDef};
pub use power::{PowerConsumer, PowerOutcome, resolve_power};
pub use priority::{Consumer, Priority};
pub use reactor::{Reactor, ReactorStatus};
pub use resources::{ResourceKind, Resources};
pub use rng::Rng;
pub use state::{Action, GameState, Rejected, SAVE_VERSION, World};
