//! Top-level game state and the engine entry points `advance` (time) and `apply`
//! (player intent). M2: per tick, advance computes job readiness, resolves the
//! power budget, runs the two-phase economy, applies crew upkeep + arrivals, and
//! updates reactor status + disclosure flags. Pure + deterministic.

use crate::config::Tuning;
use crate::crew::Crew;
use crate::economy::run_economy;
use crate::flags::Flag;
use crate::job::{JobId, JobRegistry};
use crate::log::Log;
use crate::module::{ModuleId, ModuleRegistry};
use crate::origin::{Origin, OriginDef, default_origin_def};
use crate::power::{PowerConsumer, resolve_power};
use crate::priority::{Consumer, Priority};
use crate::reactor::{Reactor, ReactorStatus};
use crate::resources::{ResourceKind, Resources};
use crate::rng::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SAVE_VERSION: u32 = 3;

/// Content registries supplied to the engine each call (like `Tuning`, these are
/// not part of the saved state).
pub struct World<'a> {
    pub tuning: &'a Tuning,
    pub jobs: &'a JobRegistry,
    pub modules: &'a ModuleRegistry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Ignite,
    Salvage,
    FeedCore,
    ActivateCrewSource,
    Build(ModuleId),
    AssignCrew { job: JobId, delta: i32 },
    SetPriority { consumer: Consumer, up: bool },
}

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
    pub salvage_cooldown: u32,
    pub built: BTreeMap<ModuleId, u32>,
    pub priority: Priority,
}

impl GameState {
    pub fn new(world: &World, seed: u64) -> Self {
        Self::new_with_origin(&default_origin_def(world.tuning), world, seed)
    }

    pub fn new_with_origin(origin: &OriginDef, world: &World, seed: u64) -> Self {
        let tuning = world.tuning;
        let mut reactor = Reactor::with_charge(
            origin.start.reactor_capacity,
            origin.start.reactor_charge,
            tuning,
        );
        reactor.max_output = tuning.reactor_base_max_output;
        let mut resources = Resources {
            scrap: origin.start.scrap,
            ..Resources::default()
        };
        resources.set_cap(tuning.resource_base_cap);
        let mut state = Self {
            version: SAVE_VERSION,
            seed,
            rng: Rng::new(seed),
            tick: 0,
            origin: origin.clone(),
            reactor,
            resources,
            crew: Crew::default(),
            flags: BTreeSet::new(),
            log: Log::default(),
            salvage_cooldown: 0,
            built: BTreeMap::new(),
            priority: Priority::default(),
        };
        if !origin.intro.is_empty() {
            state.log.push(0, origin.intro.clone(), tuning.log_capacity);
        }
        state.sync_priority(world);
        state.update_disclosure_flags(tuning);
        state
    }

    pub fn origin_id(&self) -> Origin {
        self.origin.id
    }

    pub fn built_count(&self, id: ModuleId) -> u32 {
        self.built.get(&id).copied().unwrap_or(0)
    }

    fn has_module(&self, id: ModuleId) -> bool {
        self.built_count(id) > 0
    }

    /// Housing cap = base + Σ Quarters housing unlocks.
    pub fn housing(&self, world: &World) -> u32 {
        let mut h = world.tuning.base_housing;
        if let Some(m) = world.modules.get(ModuleId::Quarters) {
            h += m.unlocks.housing * self.built_count(ModuleId::Quarters);
        }
        h
    }

    /// Per-crew passive life-support draw (0 until Life Support is built).
    fn life_support_draw(&self, world: &World) -> f64 {
        if !self.has_module(ModuleId::LifeSupport) {
            return 0.0;
        }
        let per = world
            .modules
            .get(ModuleId::LifeSupport)
            .map(|m| m.unlocks.passive_draw_per_crew)
            .unwrap_or(0.0);
        per * self.crew.total as f64
    }

    /// Build the priority-ordered consumer list for power resolution, using
    /// only consumers that exist (assignable jobs + life support if built).
    fn ordered_consumers(&self, world: &World) -> Vec<PowerConsumer> {
        let mut list: Vec<PowerConsumer> = Vec::new();
        for &c in &self.priority.0 {
            match c {
                Consumer::LifeSupport => {
                    let draw = self.life_support_draw(world);
                    if draw > 0.0 {
                        list.push(PowerConsumer { consumer: c, draw });
                    }
                }
                Consumer::Job(id) => {
                    let crew = self.crew.assigned(id);
                    if crew == 0 {
                        continue;
                    }
                    let Some(job) = world.jobs.get(id) else {
                        continue;
                    };
                    // Only input-ready jobs draw power this tick.
                    if !crate::economy::job_ready(job, crew, &self.resources) {
                        continue;
                    }
                    let draw = job.power_draw * crew as f64;
                    if draw > 0.0 {
                        list.push(PowerConsumer { consumer: c, draw });
                    }
                }
            }
        }
        list
    }

    /// Ensure the priority list contains every assignable consumer, in default
    /// order, appending any new ones (e.g. after a build).
    fn sync_priority(&mut self, world: &World) {
        // Default order: LifeSupport first, then jobs by ascending default_priority.
        let mut jobs: Vec<_> = world.jobs.iter().collect();
        jobs.sort_by_key(|j| j.default_priority);
        let mut defaults = vec![Consumer::LifeSupport];
        defaults.extend(jobs.iter().map(|j| Consumer::Job(j.id)));
        self.priority.ensure_contains(&defaults);
    }

    pub fn advance(&mut self, world: &World, ticks: u64) {
        let tuning = world.tuning;
        for _ in 0..ticks {
            let prev_status = self.reactor.status;
            self.tick += 1;
            if self.salvage_cooldown > 0 {
                self.salvage_cooldown -= 1;
            }

            // --- crew upkeep (Rations) -> starving efficiency ---
            let upkeep = self.crew.ration_upkeep(tuning);
            let starving = !self.resources.try_spend(ResourceKind::Rations, upkeep);
            let efficiency = if starving {
                tuning.starving_efficiency
            } else {
                1.0
            };

            // --- power budget ---
            let consumers = self.ordered_consumers(world);
            let outcome = resolve_power(
                &consumers,
                self.reactor.max_output,
                self.reactor.charge,
                self.reactor.capacity,
                self.resources.fuel,
                tuning.fuel_per_output,
            );
            self.resources.fuel = (self.resources.fuel - outcome.fuel_burned).max(0.0);
            let active_draw: f64 = consumers
                .iter()
                .filter(|c| !outcome.paused.contains(&c.consumer))
                .map(|c| c.draw)
                .sum();
            let overdrawn = active_draw > outcome.generation;
            self.reactor
                .set_from_power(outcome.new_charge, overdrawn, tuning);

            // --- economy (two-phase) ---
            run_economy(
                &self.crew.assignments,
                world.jobs,
                &outcome.paused,
                efficiency,
                &mut self.resources,
            );

            // --- crew arrivals (capped by housing) ---
            let housing = self.housing(world);
            if self.flags.contains(&Flag::CrewSourceActive)
                && self.crew.tick_arrival(housing, tuning)
            {
                self.log_flavor("crew_arrived", tuning);
            }

            // --- status transition logging ---
            if self.reactor.status != prev_status {
                match self.reactor.status {
                    ReactorStatus::Failing => self.log_flavor("failing", tuning),
                    ReactorStatus::Cold => self.log_flavor("cold", tuning),
                    _ => {}
                }
            }
            self.update_disclosure_flags(tuning);
        }
    }

    pub fn apply(&mut self, world: &World, action: Action) -> Result<(), Rejected> {
        let tuning = world.tuning;
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
                if self.salvage_cooldown > 0 {
                    return Err(Rejected("salvage is on cooldown"));
                }
                self.resources
                    .add(ResourceKind::Scrap, tuning.salvage_yield);
                self.salvage_cooldown = tuning.salvage_cooldown_ticks;
                self.update_disclosure_flags(tuning);
                Ok(())
            }
            Action::FeedCore => {
                if !self
                    .resources
                    .try_spend(ResourceKind::Scrap, tuning.feed_cost_scrap)
                {
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
                if !self
                    .resources
                    .try_spend(ResourceKind::Scrap, tuning.crew_source_cost)
                {
                    return Err(Rejected("not enough scrap to activate the crew-source"));
                }
                self.flags.insert(Flag::CrewSourceActive);
                self.crew.begin_arrivals(tuning);
                self.log_flavor("source_active", tuning);
                self.update_disclosure_flags(tuning);
                Ok(())
            }
            Action::Build(id) => {
                let module = world.modules.get(id).ok_or(Rejected("unknown module"))?;
                if !module.repeatable && self.has_module(id) {
                    return Err(Rejected("already built"));
                }
                for &req in &module.requires_modules {
                    if !self.has_module(req) {
                        return Err(Rejected("missing prerequisite module"));
                    }
                }
                let n = self.built_count(id) as f64;
                let cost: Vec<(ResourceKind, f64)> = module
                    .cost
                    .iter()
                    .map(|&(k, a)| (k, a * module.cost_scaling.powf(n)))
                    .collect();
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough resources to build"));
                }
                *self.built.entry(id).or_insert(0) += 1;
                // Apply caps/housing/reactor unlocks immediately.
                if module.unlocks.cap_increase > 0.0 {
                    let new_cap = self.resources.cap + module.unlocks.cap_increase;
                    self.resources.set_cap(new_cap);
                }
                if module.unlocks.max_output > 0.0 {
                    self.reactor.max_output += module.unlocks.max_output;
                }
                if module.unlocks.capacity_increase > 0.0 {
                    self.reactor.capacity += module.unlocks.capacity_increase;
                }
                self.sync_priority(world);
                self.update_disclosure_flags(tuning);
                Ok(())
            }
            Action::AssignCrew { job, delta } => {
                // Job must be unlocked by its gating module.
                if let Some(j) = world.jobs.get(job) {
                    if let Some(gate) = j.gated_by
                        && !self.has_module(gate)
                    {
                        return Err(Rejected("that job's module is not built"));
                    }
                } else {
                    return Err(Rejected("unknown job"));
                }
                if self.crew.assign(job, delta) {
                    Ok(())
                } else {
                    Err(Rejected("no change (no idle crew or none assigned)"))
                }
            }
            Action::SetPriority { consumer, up } => {
                if self.priority.rank(consumer).is_none() {
                    return Err(Rejected("unknown consumer"));
                }
                if up {
                    self.priority.move_up(consumer);
                } else {
                    self.priority.move_down(consumer);
                }
                Ok(())
            }
        }
    }

    pub(crate) fn log_flavor(&mut self, key: &str, tuning: &Tuning) {
        let line = self.origin.flavor(key).to_string();
        if !line.is_empty() {
            self.log.push(self.tick, line, tuning.log_capacity);
        }
    }

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
        if self.crew.total > 0 {
            self.flags.insert(Flag::CrewArrived);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jobs() -> JobRegistry {
        content_jobs()
    }
    fn modules() -> ModuleRegistry {
        content_modules()
    }
    // Minimal in-test registries mirroring content (keeps sim tests self-contained).
    fn content_jobs() -> JobRegistry {
        use crate::job::{Job, Recipe};
        JobRegistry(vec![
            Job {
                id: JobId::Salvager,
                name: "Salvager".into(),
                gated_by: None,
                recipe: Recipe {
                    inputs: vec![],
                    outputs: vec![(ResourceKind::Scrap, 1.0)],
                },
                power_draw: 0.0,
                default_priority: 6,
            },
            Job {
                id: JobId::Refiner,
                name: "Refiner".into(),
                gated_by: Some(ModuleId::Refinery),
                recipe: Recipe {
                    inputs: vec![(ResourceKind::Scrap, 2.0)],
                    outputs: vec![(ResourceKind::Fuel, 1.0)],
                },
                power_draw: 2.0,
                default_priority: 2,
            },
        ])
    }
    fn content_modules() -> ModuleRegistry {
        use crate::module::{Module, ModuleUnlocks};
        ModuleRegistry(vec![
            Module {
                id: ModuleId::Refinery,
                name: "Refinery".into(),
                tier: 1,
                cost: vec![(ResourceKind::Scrap, 20.0)],
                cost_scaling: 1.0,
                repeatable: false,
                requires_modules: vec![],
                unlocks: ModuleUnlocks {
                    job: Some(JobId::Refiner),
                    ..Default::default()
                },
            },
            Module {
                id: ModuleId::Quarters,
                name: "Quarters".into(),
                tier: 1,
                cost: vec![(ResourceKind::Scrap, 10.0)],
                cost_scaling: 1.6,
                repeatable: true,
                requires_modules: vec![],
                unlocks: ModuleUnlocks {
                    housing: 4,
                    ..Default::default()
                },
            },
        ])
    }
    fn world<'a>(t: &'a Tuning, j: &'a JobRegistry, m: &'a ModuleRegistry) -> World<'a> {
        World {
            tuning: t,
            jobs: j,
            modules: m,
        }
    }

    #[test]
    fn new_game_is_versioned_v3() {
        let (t, j, m) = (Tuning::default(), jobs(), modules());
        let s = GameState::new(&world(&t, &j, &m), 1);
        assert_eq!(s.version, 3);
        assert_eq!(s.resources.cap, t.resource_base_cap);
    }

    #[test]
    fn advance_n_equals_n_times_advance_1() {
        let (t, j, m) = (Tuning::default(), jobs(), modules());
        let mut bulk = GameState::new(&world(&t, &j, &m), 1);
        let mut step = GameState::new(&world(&t, &j, &m), 1);
        bulk.advance(&world(&t, &j, &m), 25);
        for _ in 0..25 {
            step.advance(&world(&t, &j, &m), 1);
        }
        assert_eq!(bulk, step);
    }

    #[test]
    fn build_refinery_unlocks_refiner_assignment() {
        let (t, j, m) = (Tuning::default(), jobs(), modules());
        let w = world(&t, &j, &m);
        let mut s = GameState::new(&w, 1);
        // Need scrap + idle crew.
        s.resources.add(ResourceKind::Scrap, 30.0);
        s.crew.total = 2;
        // Can't assign Refiner before building Refinery.
        assert!(
            s.apply(
                &w,
                Action::AssignCrew {
                    job: JobId::Refiner,
                    delta: 1
                }
            )
            .is_err()
        );
        assert!(s.apply(&w, Action::Build(ModuleId::Refinery)).is_ok());
        assert_eq!(s.built_count(ModuleId::Refinery), 1);
        assert!(
            s.apply(
                &w,
                Action::AssignCrew {
                    job: JobId::Refiner,
                    delta: 1
                }
            )
            .is_ok()
        );
        assert_eq!(s.crew.assigned(JobId::Refiner), 1);
    }

    #[test]
    fn repeatable_build_scales_cost() {
        let (t, j, m) = (Tuning::default(), jobs(), modules());
        let w = world(&t, &j, &m);
        let mut s = GameState::new(&w, 1);
        s.resources.add(ResourceKind::Scrap, 100.0);
        let before = s.resources.scrap;
        s.apply(&w, Action::Build(ModuleId::Quarters)).unwrap(); // 10
        let after1 = s.resources.scrap;
        s.apply(&w, Action::Build(ModuleId::Quarters)).unwrap(); // 10 * 1.6
        let after2 = s.resources.scrap;
        assert!((before - after1 - 10.0).abs() < 1e-9);
        assert!((after1 - after2 - 16.0).abs() < 1e-9);
    }

    #[test]
    fn assigned_salvagers_produce_scrap_over_time() {
        let (t, j, m) = (Tuning::default(), jobs(), modules());
        let w = world(&t, &j, &m);
        let mut s = GameState::new(&w, 1);
        s.crew.total = 2;
        s.apply(
            &w,
            Action::AssignCrew {
                job: JobId::Salvager,
                delta: 2,
            },
        )
        .unwrap();
        let before = s.resources.scrap;
        s.advance(&w, 10);
        assert!(s.resources.scrap > before); // 2 salvagers × 1/tick × 10
    }

    #[test]
    fn priority_reorder_via_action() {
        let (t, j, m) = (Tuning::default(), jobs(), modules());
        let w = world(&t, &j, &m);
        let mut s = GameState::new(&w, 1);
        let c = Consumer::Job(JobId::Refiner);
        let before = s.priority.rank(c).unwrap();
        s.apply(
            &w,
            Action::SetPriority {
                consumer: c,
                up: true,
            },
        )
        .unwrap();
        assert!(s.priority.rank(c).unwrap() <= before);
    }
}
