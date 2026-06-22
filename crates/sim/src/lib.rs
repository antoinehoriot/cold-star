//! Cold Star simulation core. Pure, deterministic, platform-agnostic.
#![forbid(unsafe_code)]

pub mod config;
pub mod crew;
pub mod flags;
pub mod log;
pub mod origin;
pub mod reactor;
pub mod resources;
pub mod rng;
pub mod save;
pub mod state;
pub mod time;

pub use config::Tuning;
pub use crew::Crew;
pub use flags::Flag;
pub use log::{Log, LogEntry};
pub use origin::{Origin, OriginDef};
pub use reactor::{Reactor, ReactorStatus};
pub use resources::{ResourceKind, Resources};
pub use rng::Rng;
pub use state::{Action, GameState, Rejected, SAVE_VERSION};
