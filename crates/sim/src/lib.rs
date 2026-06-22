//! Cold Star simulation core. Pure, deterministic, platform-agnostic.
#![forbid(unsafe_code)]

pub mod config;
pub mod reactor;
pub mod rng;
pub mod state;

pub use config::Tuning;
pub use reactor::{Reactor, ReactorStatus};
pub use rng::Rng;
pub use state::{Action, GameState, Origin, Rejected, SAVE_VERSION};
