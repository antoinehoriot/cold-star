//! Top-level game state and the two engine entry points: `advance` (time) and
//! `apply` (player intent). Both are pure functions of state + tuning.

use crate::config::Tuning;
use crate::crew::Crew;
use crate::flags::Flag;
use crate::log::Log;
use crate::origin::{Origin, OriginDef, default_origin_def};
use crate::reactor::{Reactor, ReactorStatus};
use crate::resources::Resources;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SAVE_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Ignite,
    Salvage,
    FeedCore,
    ActivateCrewSource,
}

/// Why an action was refused. Carries a human-readable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected(pub &'static str);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub version: u32,
    pub seed: u64,
    pub rng: Rng,
    pub tick: u64,
    pub origin: OriginDef,
    pub reactor: Reactor,
    pub resources: Resources,
    pub crew: Crew,
    pub flags: BTreeSet<Flag>,
    pub log: Log,
}

impl GameState {
    /// Default new game (bare Drifter, full charge) — used by tests and the
    /// no-origin fallback.
    pub fn new(tuning: &Tuning, seed: u64) -> Self {
        Self::new_with_origin(&default_origin_def(tuning), tuning, seed)
    }

    /// New game from an origin definition (the real new-game path).
    pub fn new_with_origin(origin: &OriginDef, tuning: &Tuning, seed: u64) -> Self {
        let reactor = Reactor::with_charge(
            origin.start.reactor_capacity,
            origin.start.reactor_charge,
            tuning,
        );
        let mut state = Self {
            version: SAVE_VERSION,
            seed,
            rng: Rng::new(seed),
            tick: 0,
            origin: origin.clone(),
            reactor,
            resources: Resources {
                scrap: origin.start.scrap,
            },
            crew: Crew::default(),
            flags: BTreeSet::new(),
            log: Log::default(),
        };
        if !origin.intro.is_empty() {
            state.log.push(0, origin.intro.clone(), tuning.log_capacity);
        }
        state.update_disclosure_flags(tuning);
        state
    }

    pub fn origin_id(&self) -> Origin {
        self.origin.id
    }

    /// Advance the simulation by `ticks` whole ticks. (Crew/flag/log advance
    /// behaviors are added in the next task; here only the reactor decays.)
    pub fn advance(&mut self, tuning: &Tuning, ticks: u64) {
        for _ in 0..ticks {
            self.reactor.tick(tuning);
            self.tick += 1;
            self.update_disclosure_flags(tuning);
        }
    }

    /// Apply a player intent. Returns `Err(Rejected)` if invalid.
    pub fn apply(&mut self, tuning: &Tuning, action: Action) -> Result<(), Rejected> {
        match action {
            Action::Ignite => {
                if self.reactor.status != ReactorStatus::Cold {
                    return Err(Rejected("the core is already lit"));
                }
                self.reactor.ignite(tuning);
                self.log_flavor("ignite", tuning);
                self.update_disclosure_flags(tuning);
                Ok(())
            }
            Action::Salvage => {
                self.resources.add_scrap(tuning.salvage_yield);
                self.update_disclosure_flags(tuning);
                Ok(())
            }
            Action::FeedCore => {
                if !self.resources.try_spend_scrap(tuning.feed_cost_scrap) {
                    return Err(Rejected("not enough scrap to feed the core"));
                }
                self.reactor.feed(tuning.feed_charge, tuning);
                self.update_disclosure_flags(tuning);
                Ok(())
            }
            Action::ActivateCrewSource => {
                if !self.flags.contains(&Flag::CrewSourceAvailable) {
                    return Err(Rejected("the crew-source is not available yet"));
                }
                if self.flags.contains(&Flag::CrewSourceActive) {
                    return Err(Rejected("the crew-source is already active"));
                }
                if !self.resources.try_spend_scrap(tuning.crew_source_cost) {
                    return Err(Rejected("not enough scrap to activate the crew-source"));
                }
                self.flags.insert(Flag::CrewSourceActive);
                self.crew.begin_arrivals(tuning);
                self.log_flavor("source_active", tuning);
                self.update_disclosure_flags(tuning);
                Ok(())
            }
        }
    }

    /// Push the origin's flavor line for `key`, if non-empty.
    pub(crate) fn log_flavor(&mut self, key: &str, tuning: &Tuning) {
        let line = self.origin.flavor(key).to_string();
        if !line.is_empty() {
            self.log.push(self.tick, line, tuning.log_capacity);
        }
    }

    /// Idempotently set disclosure flags from current state, logging the
    /// one-time "stable" beat when the core first establishes.
    pub(crate) fn update_disclosure_flags(&mut self, tuning: &Tuning) {
        if self.reactor.status != ReactorStatus::Cold {
            self.flags.insert(Flag::SalvageUnlocked);
        }
        if self.reactor.established && !self.flags.contains(&Flag::CoreEstablished) {
            self.flags.insert(Flag::CoreEstablished);
            self.log_flavor("stable", tuning);
        }
        if self.flags.contains(&Flag::CoreEstablished)
            && self.resources.scrap >= tuning.crew_source_cost
        {
            self.flags.insert(Flag::CrewSourceAvailable);
        }
        if self.crew.count > 0 {
            self.flags.insert(Flag::CrewArrived);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> Tuning {
        Tuning::default()
    }

    #[test]
    fn new_game_is_versioned() {
        let s = GameState::new(&t(), 123);
        assert_eq!(s.version, SAVE_VERSION);
        assert_eq!(s.seed, 123);
        assert_eq!(s.tick, 0);
    }

    #[test]
    fn advance_accumulates_ticks() {
        let mut s = GameState::new(&t(), 1);
        s.advance(&t(), 5);
        assert_eq!(s.tick, 5);
    }

    #[test]
    fn advance_n_equals_n_times_advance_1() {
        let mut bulk = GameState::new(&t(), 1);
        let mut step = GameState::new(&t(), 1);
        bulk.advance(&t(), 37);
        for _ in 0..37 {
            step.advance(&t(), 1);
        }
        assert_eq!(bulk, step);
    }

    #[test]
    fn ignite_only_from_cold() {
        let tn = t();
        // A Cold-start origin.
        let origin = OriginDef {
            start: crate::origin::OriginStart {
                scrap: 0.0,
                reactor_capacity: 60.0,
                reactor_charge: 0.0,
            },
            ..default_origin_def(&tn)
        };
        let mut s = GameState::new_with_origin(&origin, &tn, 1);
        assert_eq!(s.reactor.status, ReactorStatus::Cold);
        assert!(s.apply(&tn, Action::Ignite).is_ok());
        assert_eq!(s.reactor.status, ReactorStatus::Igniting);
        assert!(s.flags.contains(&Flag::SalvageUnlocked));
        assert_eq!(
            s.apply(&tn, Action::Ignite),
            Err(Rejected("the core is already lit"))
        );
    }

    #[test]
    fn salvage_then_feed_consumes_scrap_and_charges() {
        let tn = t();
        let mut s = GameState::new(&tn, 1);
        s.advance(&tn, 60); // drain charge below full
        let before = s.reactor.charge;
        s.apply(&tn, Action::Salvage).unwrap();
        s.apply(&tn, Action::Salvage).unwrap(); // 10 scrap
        assert_eq!(s.resources.scrap, 2.0 * tn.salvage_yield);
        s.apply(&tn, Action::FeedCore).unwrap();
        assert_eq!(
            s.resources.scrap,
            2.0 * tn.salvage_yield - tn.feed_cost_scrap
        );
        assert!(s.reactor.charge > before);
    }

    #[test]
    fn feed_rejected_without_scrap() {
        let tn = t();
        let mut s = GameState::new(&tn, 1);
        assert_eq!(s.resources.scrap, 0.0);
        assert_eq!(
            s.apply(&tn, Action::FeedCore),
            Err(Rejected("not enough scrap to feed the core"))
        );
    }

    #[test]
    fn crew_source_requires_availability_and_scrap() {
        let tn = t();
        let mut s = GameState::new(&tn, 1); // established (full charge) so CoreEstablished sets on first update
        // Not available until enough scrap banked.
        assert_eq!(
            s.apply(&tn, Action::ActivateCrewSource),
            Err(Rejected("the crew-source is not available yet"))
        );
        for _ in 0..(tn.crew_source_cost / tn.salvage_yield) as u32 {
            s.apply(&tn, Action::Salvage).unwrap();
        }
        assert!(s.flags.contains(&Flag::CrewSourceAvailable));
        assert!(s.apply(&tn, Action::ActivateCrewSource).is_ok());
        assert!(s.flags.contains(&Flag::CrewSourceActive));
        assert_eq!(
            s.apply(&tn, Action::ActivateCrewSource),
            Err(Rejected("the crew-source is already active"))
        );
    }
}
