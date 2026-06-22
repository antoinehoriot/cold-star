//! Cold Star simulation core. Pure, deterministic, platform-agnostic.
#![forbid(unsafe_code)]

pub mod config;
pub mod reactor;
pub mod rng;

pub use config::Tuning;
pub use reactor::{Reactor, ReactorStatus};
pub use rng::Rng;
