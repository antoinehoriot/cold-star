//! Top-level game state and the engine entry points `advance` (time) and `apply`
//! (player intent). M2: per tick, advance computes job readiness, resolves the
//! power budget, runs the two-phase economy, applies crew upkeep + arrivals, and
//! updates reactor status + disclosure flags. Pure + deterministic.

use crate::codex::{Codex, Discovery, DiscoveryId, NoteId};
use crate::config::Tuning;
use crate::crew::Crew;
use crate::economy::run_economy;
use crate::expedition::Activity;
use crate::expedition::Expedition;
use crate::flags::Flag;
use crate::galaxy::{
    Biome, Galaxy, GalaxyState, HOME, SiteKind, SystemId, distance, jump_cost, materialize_sites,
    reveal_from,
};
use crate::job::{JobId, JobRegistry};
use crate::landing::{Landing, SurfaceSite, surface_seed};
use crate::log::Log;
use crate::module::{ModuleId, ModuleRegistry};
use crate::origin::{Origin, OriginDef, default_origin_def};
use crate::power::{PowerConsumer, resolve_power};
use crate::priority::{Consumer, Priority};
use crate::reactor::{Reactor, ReactorStatus};
use crate::research::{ActiveResearch, Grant, Research, TechId};
use crate::resources::{ResourceKind, Resources};
use crate::rng::Rng;
use crate::ship::Ship;
use crate::telemetry::Telemetry;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Cumulative scrap salvaged that triggers the heavy-consumption uneasy note.
const HEAVY_CONSUMPTION_SCRAP: f64 = 500.0;

pub const SAVE_VERSION: u32 = 7;

/// Content registries supplied to the engine each call (like `Tuning`, these are
/// not part of the saved state).
pub struct World<'a> {
    pub tuning: &'a Tuning,
    pub jobs: &'a JobRegistry,
    pub modules: &'a ModuleRegistry,
    pub techs: &'a crate::research::TechRegistry,
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
    StartResearch(TechId),
    UpgradeDrive,
    Jump(crate::galaxy::SystemId),
    ScanSystem(crate::galaxy::SystemId),
    ResolveSite(crate::galaxy::SystemId, usize),
    Scavenge,
    Land(usize),
    Extract,
    MoveToNode(u32),
    HarvestLife,
    LeaveLife,
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
    pub telemetry: Telemetry,
    pub research: Research,
    pub codex: Codex,
    pub atmosphere_grace: u32,
    pub crew_loss_timer: u32,
    pub research_output_mult: BTreeMap<JobId, f64>,
    pub total_scrap_gained: f64,
    pub world_seed: u64,
    pub ship: Ship,
    pub expedition: Expedition,
    pub galaxy_state: GalaxyState,
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
            telemetry: Telemetry::default(),
            research: Research::default(),
            codex: Codex::default(),
            atmosphere_grace: tuning.atmosphere_grace_max,
            crew_loss_timer: 0,
            research_output_mult: BTreeMap::new(),
            total_scrap_gained: 0.0,
            world_seed: seed,
            ship: Ship::new(tuning),
            expedition: Expedition::default(),
            galaxy_state: GalaxyState::default(),
        };
        if !origin.intro.is_empty() {
            state.log.push(0, origin.intro.clone(), tuning.log_capacity);
        }
        state.sync_priority(world);
        state.update_disclosure_flags(world);
        let galaxy = Galaxy::generate(state.world_seed, world.tuning);
        reveal_from(
            &galaxy,
            HOME,
            world.tuning.galaxy_reveal_radius,
            &mut state.galaxy_state.revealed,
        );
        state
    }

    pub fn origin_id(&self) -> Origin {
        self.origin.id
    }

    /// Regenerate the (unsaved) galaxy from the world seed.
    pub fn galaxy(&self, world: &World) -> Galaxy {
        Galaxy::generate(self.world_seed, world.tuning)
    }

    /// Away = not docked at home, or mid-jump. Station management locks while away.
    pub fn away(&self) -> bool {
        self.expedition.location != HOME
            || matches!(
                self.expedition.activity,
                Some(crate::expedition::Activity::Transit { .. })
            )
    }

    /// Landed = currently on a planet surface.
    pub fn landed(&self) -> bool {
        self.expedition.landing.is_some()
    }

    fn surface_site(
        &self,
        world: &World,
        system: SystemId,
        site_idx: usize,
        biome: Biome,
    ) -> SurfaceSite {
        SurfaceSite::generate(
            surface_seed(self.world_seed, system, site_idx),
            biome,
            world.tuning,
        )
    }

    /// Stranded = away and cannot afford the cheapest jump to any revealed,
    /// in-range system (so the player could otherwise be stuck). Drives the
    /// soft-recovery Scavenge action.
    fn is_stranded(&self, world: &World) -> bool {
        if !self.away() {
            return false;
        }
        let galaxy = self.galaxy(world);
        let Some(here) = galaxy.get(self.expedition.location) else {
            return true;
        };
        let cheapest = self
            .galaxy_state
            .revealed
            .iter()
            .map(|&id| crate::galaxy::SystemId(id))
            .filter(|&id| id != self.expedition.location)
            .filter_map(|id| galaxy.get(id))
            .map(|s| distance(here, s))
            .filter(|&d| d <= self.ship.drive_range)
            .map(|d| jump_cost(d, world.tuning))
            .fold(f64::INFINITY, f64::min);
        // No reachable system, or can't afford the cheapest one.
        cheapest.is_infinite() || self.resources.amount(ResourceKind::Fuel) < cheapest
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

    /// Atmosphere power draw (per crew); 0 until the Atmosphere module is built.
    fn atmosphere_draw(&self, world: &World) -> f64 {
        if !self.has_module(ModuleId::Atmosphere) {
            return 0.0;
        }
        world.tuning.atmosphere_draw_per_crew * self.crew.total as f64
    }

    /// Thermal power draw (flat); 0 until the Thermal module is built.
    fn thermal_draw(&self, world: &World) -> f64 {
        if self.has_module(ModuleId::Thermal) {
            world.tuning.thermal_draw
        } else {
            0.0
        }
    }

    /// Build the priority-ordered consumer list for power resolution, using only
    /// consumers that exist (assignable jobs + Atmosphere/Thermal if built).
    fn ordered_consumers(&self, world: &World) -> Vec<PowerConsumer> {
        let mut list: Vec<PowerConsumer> = Vec::new();
        for &c in &self.priority.0 {
            match c {
                Consumer::Atmosphere => {
                    let draw = self.atmosphere_draw(world);
                    if draw > 0.0 {
                        list.push(PowerConsumer { consumer: c, draw });
                    }
                }
                Consumer::Thermal => {
                    let draw = self.thermal_draw(world);
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
        // Default order: Atmosphere + Thermal first, then jobs by ascending default_priority.
        let mut jobs: Vec<_> = world.jobs.iter().collect();
        jobs.sort_by_key(|j| j.default_priority);
        let mut defaults = vec![Consumer::Atmosphere, Consumer::Thermal];
        defaults.extend(jobs.iter().map(|j| Consumer::Job(j.id)));
        self.priority.ensure_contains(&defaults);
    }

    pub fn advance(&mut self, world: &World, ticks: u64) {
        let tuning = world.tuning;
        for _ in 0..ticks {
            let before: [f64; 7] = ResourceKind::ALL.map(|k| self.resources.amount(k));
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
                tuning.reactor_baseline_draw,
            );
            self.resources.fuel = (self.resources.fuel - outcome.fuel_burned).max(0.0);
            // Overdraw status is driven by consumer demand vs generation only;
            // the baseline is the reactor's own idle cost and does not trigger
            // the Straining/Critical branch.
            let overdrawn = outcome.consumer_draw > outcome.generation;
            self.reactor
                .set_from_power(outcome.new_charge, overdrawn, tuning);

            // --- thermal failure: unpowered thermal lets the cold creep back ---
            if self.has_module(ModuleId::Thermal) && outcome.paused.contains(&Consumer::Thermal) {
                self.reactor.drain(tuning.thermal_failure_drain, tuning);
            }

            // --- economy (two-phase) ---
            let report = run_economy(
                &self.crew.assignments,
                world.jobs,
                &outcome.paused,
                efficiency,
                &self.research_output_mult,
                &mut self.resources,
            );

            // Track cumulative scrap gained this tick (for the heavy-consumption note).
            let scrap_after = self.resources.amount(ResourceKind::Scrap);
            let scrap_before = before[0]; // ResourceKind::ALL[0] == Scrap
            if scrap_after > scrap_before {
                self.total_scrap_gained += scrap_after - scrap_before;
            }

            // --- research progress ---
            if let Some(active) = self.research.active.as_mut() {
                if active.ticks_remaining > 0 {
                    active.ticks_remaining -= 1;
                }
                if active.ticks_remaining == 0 {
                    let id = active.tech;
                    self.research.active = None;
                    self.research.completed.insert(id);
                    if let Some(tech) = world.techs.get(id) {
                        let grant = tech.grant.clone();
                        self.apply_grant(&grant);
                        let (name, lore) = (tech.name.clone(), tech.lore.clone());
                        self.log.push(
                            self.tick,
                            format!("Research complete: {name}"),
                            tuning.log_capacity,
                        );
                        self.codex.add(Discovery {
                            id: DiscoveryId::Tech(id),
                            title: name,
                            text: lore,
                            tick: self.tick,
                        });
                    }
                }
            }

            // --- expedition activity (jump/scan/resolve/scavenge timers) ---
            self.tick_expedition(world);

            // --- surface landing (Suit drain + forced retreat) ---
            self.tick_landing(world);

            // --- atmosphere failure: survival clock -> crew loss ---
            let atmosphere_failing = self.has_module(ModuleId::Atmosphere)
                && self.crew.total > 0
                && (self.reactor.status == ReactorStatus::Cold
                    || outcome.paused.contains(&Consumer::Atmosphere));
            if atmosphere_failing {
                if self.atmosphere_grace > 0 {
                    self.atmosphere_grace -= 1;
                    if self.atmosphere_grace == 0 {
                        self.crew_loss_timer = tuning.crew_loss_interval;
                        self.log.push(
                            self.tick,
                            "ATMOSPHERE FAILING. The air is going.",
                            tuning.log_capacity,
                        );
                    }
                } else {
                    // Clock exhausted: lose a crew every crew_loss_interval ticks.
                    if self.crew_loss_timer > 0 {
                        self.crew_loss_timer -= 1;
                    }
                    if self.crew_loss_timer == 0 && self.crew.lose_one() {
                        self.crew_loss_timer = tuning.crew_loss_interval;
                        self.note_first_crew_death(tuning);
                        self.log.push(
                            self.tick,
                            "Someone stops breathing in the dark.",
                            tuning.log_capacity,
                        );
                    }
                }
            } else {
                // Powered again: reset the clock.
                self.atmosphere_grace = tuning.atmosphere_grace_max;
                self.crew_loss_timer = 0;
            }

            // --- triggered narrative notes (each fires once) ---
            self.check_triggered_notes(tuning);

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
            let mut net = std::collections::BTreeMap::new();
            for (i, k) in ResourceKind::ALL.iter().enumerate() {
                let delta = self.resources.amount(*k) - before[i];
                if delta != 0.0 {
                    net.insert(*k, delta);
                }
            }
            self.telemetry = Telemetry {
                generation: outcome.generation,
                consumer_draw: outcome.consumer_draw,
                active_draw: outcome.active_draw,
                paused: outcome.paused.clone(),
                starved_input: report.starved_input.clone(),
                ran: report.ran.clone(),
                net,
                starving,
            };
            // NOTE: telemetry is fresh-written just above; update_disclosure_flags
            // reads telemetry.consumer_draw for PowerBudgetVisible. Do not reorder.
            self.update_disclosure_flags(world);
        }
    }

    /// Actions that manage the home station — refused while the player is away.
    /// Expedition actions (Jump/ScanSystem/ResolveSite/Scavenge) are always allowed.
    fn is_station_management(action: &Action) -> bool {
        matches!(
            action,
            Action::Ignite
                | Action::Salvage
                | Action::FeedCore
                | Action::ActivateCrewSource
                | Action::Build(_)
                | Action::AssignCrew { .. }
                | Action::SetPriority { .. }
                | Action::StartResearch(_)
                | Action::UpgradeDrive
        )
    }

    /// While landed, only the landing actions are allowed.
    fn is_landing_blocked(action: &Action) -> bool {
        !matches!(
            action,
            Action::MoveToNode(_) | Action::HarvestLife | Action::LeaveLife | Action::Extract
        )
    }

    pub fn apply(&mut self, world: &World, action: Action) -> Result<(), Rejected> {
        if self.landed() && Self::is_landing_blocked(&action) {
            return Err(Rejected("you are on the surface"));
        }
        if self.away() && Self::is_station_management(&action) {
            return Err(Rejected("you are away from the station"));
        }
        let tuning = world.tuning;
        match action {
            Action::Ignite => {
                if self.reactor.status != ReactorStatus::Cold {
                    return Err(Rejected("the core is already lit"));
                }
                self.reactor.ignite(tuning);
                self.log_flavor("ignite", tuning);
                self.update_disclosure_flags(world);
                Ok(())
            }
            Action::Salvage => {
                if self.salvage_cooldown > 0 {
                    return Err(Rejected("salvage is on cooldown"));
                }
                self.resources
                    .add(ResourceKind::Scrap, tuning.salvage_yield);
                self.salvage_cooldown = tuning.salvage_cooldown_ticks;
                self.update_disclosure_flags(world);
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
                self.update_disclosure_flags(world);
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
                self.update_disclosure_flags(world);
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
                if let Some(flag) = module.requires_flag
                    && !self.flags.contains(&flag)
                {
                    return Err(Rejected("jump theory is not yet understood"));
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
                if id == ModuleId::Shipyard {
                    self.ship.built = true;
                    self.flags.insert(Flag::ShipyardComplete);
                    self.log_flavor("shipyard_complete", tuning);
                }
                self.sync_priority(world);
                self.update_disclosure_flags(world);
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
            Action::StartResearch(id) => {
                if !self.has_module(ModuleId::Lab) {
                    return Err(Rejected("no lab to research in"));
                }
                let tech = world.techs.get(id).ok_or(Rejected("unknown tech"))?;
                if self.research.is_completed(id) {
                    return Err(Rejected("already researched"));
                }
                if self.research.active.is_some() {
                    return Err(Rejected("research already in progress"));
                }
                if !tech.requires.iter().all(|r| self.research.is_completed(*r)) {
                    return Err(Rejected("missing prerequisite research"));
                }
                if !self.resources.try_spend(ResourceKind::Data, tech.data_cost) {
                    return Err(Rejected("not enough data"));
                }
                self.research.active = Some(ActiveResearch {
                    tech: id,
                    ticks_remaining: tech.research_ticks,
                });
                self.log.push(
                    self.tick,
                    format!("Research begun: {}", tech.name),
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::UpgradeDrive => {
                if !self.flags.contains(&Flag::NavigationKnown) {
                    return Err(Rejected("navigation is not yet understood"));
                }
                if !self
                    .resources
                    .try_spend(ResourceKind::Isotopes, tuning.drive_upgrade_isotopes)
                {
                    return Err(Rejected("not enough isotopes to recalibrate the drive"));
                }
                self.ship.drive_range += tuning.drive_upgrade_step;
                self.log.push(
                    self.tick,
                    "The drive reaches a little further into the dark.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::ScanSystem(target) => {
                if !self.has_module(ModuleId::SensorArray) {
                    return Err(Rejected("no sensor array to scan with"));
                }
                if self.expedition.activity.is_some() {
                    return Err(Rejected("the ship is already busy"));
                }
                if !self.galaxy_state.revealed.contains(&target.0) {
                    return Err(Rejected("that system is not on the map"));
                }
                if self.galaxy_state.scanned.contains(&target.0) {
                    return Err(Rejected("already scanned"));
                }
                if !self
                    .resources
                    .try_spend(ResourceKind::Data, tuning.scan_data_cost)
                {
                    return Err(Rejected("not enough data to scan"));
                }
                self.expedition.activity = Some(Activity::Scan {
                    target,
                    ticks: tuning.scan_ticks,
                });
                Ok(())
            }
            Action::ResolveSite(system, idx) => {
                if self.expedition.activity.is_some() {
                    return Err(Rejected("the ship is already busy"));
                }
                if self.expedition.location != system {
                    return Err(Rejected("you are not in that system"));
                }
                match self
                    .galaxy_state
                    .sites
                    .get(&system.0)
                    .and_then(|v| v.get(idx))
                {
                    Some(site) if !site.depleted => {}
                    Some(_) => return Err(Rejected("that site is stripped")),
                    None => return Err(Rejected("no such site")),
                }
                if let Some(SiteKind::Planet(_)) = self
                    .galaxy_state
                    .sites
                    .get(&system.0)
                    .and_then(|v| v.get(idx))
                    .map(|s| s.kind)
                {
                    return Err(Rejected("you must land on a planet, not strip it"));
                }
                self.expedition.activity = Some(Activity::ResolveSite {
                    system,
                    site_idx: idx,
                    ticks: tuning.site_resolve_ticks,
                });
                Ok(())
            }
            Action::Jump(target) => {
                if self.expedition.activity.is_some() {
                    return Err(Rejected("the ship is already busy"));
                }
                if target == self.expedition.location {
                    return Err(Rejected("already there"));
                }
                if !self.galaxy_state.revealed.contains(&target.0) {
                    return Err(Rejected("that system is not on the map"));
                }
                let galaxy = self.galaxy(world);
                let (Some(here), Some(dest)) =
                    (galaxy.get(self.expedition.location), galaxy.get(target))
                else {
                    return Err(Rejected("unknown system"));
                };
                let dist = distance(here, dest);
                if dist > self.ship.drive_range {
                    return Err(Rejected("beyond drive range"));
                }
                let cost = jump_cost(dist, tuning);
                if !self.resources.try_spend(ResourceKind::Fuel, cost) {
                    return Err(Rejected("not enough fuel to jump"));
                }
                let ticks =
                    tuning.transit_ticks_base + (dist * tuning.transit_ticks_factor).round() as u32;
                self.expedition.activity = Some(Activity::Transit { to: target, ticks });
                Ok(())
            }
            Action::Scavenge => {
                if self.expedition.activity.is_some() {
                    return Err(Rejected("the ship is already busy"));
                }
                if !self.is_stranded(world) {
                    return Err(Rejected("no need to scavenge"));
                }
                self.expedition.activity = Some(Activity::Scavenge {
                    ticks: tuning.scavenge_ticks,
                });
                Ok(())
            }
            Action::Land(idx) => {
                if !self.ship.built {
                    return Err(Rejected("no ship to land with"));
                }
                if self.expedition.activity.is_some() || self.landed() {
                    return Err(Rejected("the ship is already busy"));
                }
                let loc = self.expedition.location;
                let Some(SiteKind::Planet(biome)) = self
                    .galaxy_state
                    .sites
                    .get(&loc.0)
                    .and_then(|v| v.get(idx))
                    .map(|s| s.kind)
                else {
                    return Err(Rejected("that site is not a planet"));
                };
                self.expedition.landing = Some(Landing {
                    system: loc,
                    site_idx: idx,
                    biome,
                    node: 0,
                    suit: tuning.suit_max,
                    satchel: std::collections::BTreeMap::new(),
                    pending_choice: false,
                    traverse: 0,
                });
                self.mark_resolved(loc, idx, 0);
                self.log.push(
                    self.tick,
                    "Dropship down. Your boots find a world.",
                    tuning.log_capacity,
                );
                self.note_once(
                    NoteId::FirstLanding,
                    "Someone Else's Sky",
                    "The air here has its own colour, its own weight. You are the first thing from your world to stand in it — and you notice, almost before anything else, that you have already begun to change it.",
                    tuning,
                );
                Ok(())
            }
            Action::Extract => {
                let Some(landing) = self.expedition.landing.take() else {
                    return Err(Rejected("you are not on a surface"));
                };
                for (k, amt) in landing.satchel {
                    let space = (self.ship.cargo - self.expedition.cargo_total()).max(0.0);
                    let take = amt.min(space);
                    if take > 0.0 {
                        *self.expedition.cargo.entry(k).or_insert(0.0) += take;
                    }
                }
                self.log.push(
                    self.tick,
                    "You burn back to orbit with what you could carry.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::MoveToNode(target) => {
                let Some(landing) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if landing.pending_choice {
                    return Err(Rejected("resolve the encounter first"));
                }
                if landing.traverse > 0 {
                    return Err(Rejected("already moving"));
                }
                let site =
                    self.surface_site(world, landing.system, landing.site_idx, landing.biome);
                if !site.is_edge(landing.node, target) {
                    return Err(Rejected("no route to that node"));
                }
                self.expedition.landing.as_mut().unwrap().traverse = tuning.traverse_ticks;
                self.expedition.landing.as_mut().unwrap().node = target;
                Ok(())
            }
            Action::HarvestLife => {
                let Some(l) = self.expedition.landing.as_mut() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if !l.pending_choice {
                    return Err(Rejected("nothing to harvest"));
                }
                let (system, site_idx, biome, node) = (l.system, l.site_idx, l.biome, l.node);
                let depth = self
                    .surface_site(world, system, site_idx, biome)
                    .get(node)
                    .map(|n| n.depth)
                    .unwrap_or(0);
                let amt = tuning.surface_life_harvest
                    * (1.0 + depth as f64 * tuning.surface_depth_reward);
                self.satchel_add(ResourceKind::Rations, amt);
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.pending_choice = false;
                }
                self.mark_resolved(system, site_idx, node);
                self.note_once(
                    NoteId::FirstLifeTaken,
                    "What You Took",
                    "You process what you need and leave the rest. The record shows a gain in Rations. It does not show what was here before you, or that it will not be here again.",
                    tuning,
                );
                Ok(())
            }
            Action::LeaveLife => {
                let Some(l) = self.expedition.landing.as_mut() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if !l.pending_choice {
                    return Err(Rejected("nothing to leave"));
                }
                let (system, site_idx, node) = (l.system, l.site_idx, l.node);
                self.satchel_add(ResourceKind::Data, tuning.surface_life_leave_data);
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.pending_choice = false;
                }
                self.mark_resolved(system, site_idx, node);
                self.note_once(
                    NoteId::LifeSpared,
                    "What You Left",
                    "You note the organism and move on. You tell yourself it was a practical decision — data over calories, scarcity of suit time. You do not examine how long you stood there before choosing.",
                    tuning,
                );
                Ok(())
            }
        }
    }

    /// Advance the single expedition activity by one tick; apply its effect on
    /// completion (mutate-on-completion). Deterministic: one decrement per tick.
    fn tick_expedition(&mut self, world: &World) {
        let tuning = world.tuning;
        let remaining = match &mut self.expedition.activity {
            None => return,
            Some(Activity::Transit { ticks, .. })
            | Some(Activity::Scan { ticks, .. })
            | Some(Activity::ResolveSite { ticks, .. })
            | Some(Activity::Scavenge { ticks }) => {
                if *ticks > 0 {
                    *ticks -= 1;
                }
                *ticks
            }
        };
        if remaining > 0 {
            return;
        }
        let finished = self.expedition.activity.take().expect("activity present");
        match finished {
            Activity::Transit { to, .. } => {
                self.expedition.location = to;
                let galaxy = self.galaxy(world);
                reveal_from(
                    &galaxy,
                    to,
                    tuning.galaxy_reveal_radius,
                    &mut self.galaxy_state.revealed,
                );
                if let std::collections::btree_map::Entry::Vacant(e) =
                    self.galaxy_state.sites.entry(to.0)
                    && let Some(sys) = galaxy.get(to)
                {
                    e.insert(materialize_sites(
                        self.world_seed,
                        to.0,
                        sys.star_type,
                        tuning,
                    ));
                }
                if to == HOME && !self.expedition.cargo.is_empty() {
                    for (k, amt) in std::mem::take(&mut self.expedition.cargo) {
                        self.resources.add(k, amt);
                    }
                    self.log.push(
                        self.tick,
                        "You unload the haul into the station's holds.",
                        tuning.log_capacity,
                    );
                }
                self.log.push(
                    self.tick,
                    "The drive cuts out. A new star fills the port.",
                    tuning.log_capacity,
                );
            }
            Activity::Scan { target, .. } => {
                self.galaxy_state.scanned.insert(target.0);
                let galaxy = self.galaxy(world);
                if let std::collections::btree_map::Entry::Vacant(e) =
                    self.galaxy_state.sites.entry(target.0)
                    && let Some(sys) = galaxy.get(target)
                {
                    e.insert(materialize_sites(
                        self.world_seed,
                        target.0,
                        sys.star_type,
                        tuning,
                    ));
                }
                self.log.push(
                    self.tick,
                    "The scan resolves. You see what waits there now.",
                    tuning.log_capacity,
                );
            }
            Activity::ResolveSite {
                system, site_idx, ..
            } => {
                let galaxy = self.galaxy(world);
                let home = galaxy.get(HOME);
                let here = galaxy.get(system);
                let grad = match (home, here) {
                    (Some(h), Some(s)) => 1.0 + distance(h, s) * tuning.reward_gradient,
                    _ => 1.0,
                };
                let kind = self
                    .galaxy_state
                    .sites
                    .get(&system.0)
                    .and_then(|v| v.get(site_idx))
                    .map(|s| s.kind);
                if let Some(kind) = kind {
                    let yields: Vec<(ResourceKind, f64)> = match kind {
                        crate::galaxy::SiteKind::AsteroidField => vec![
                            (ResourceKind::Isotopes, tuning.site_asteroid_isotopes * grad),
                            (ResourceKind::Fuel, tuning.site_asteroid_fuel * grad),
                        ],
                        crate::galaxy::SiteKind::Derelict => vec![
                            (
                                ResourceKind::Components,
                                tuning.site_derelict_components * grad,
                            ),
                            (ResourceKind::Data, tuning.site_derelict_data * grad),
                        ],
                        crate::galaxy::SiteKind::Planet(_) => vec![],
                    };
                    for (k, amt) in yields {
                        let space = (self.ship.cargo - self.expedition.cargo_total()).max(0.0);
                        let take = amt.min(space);
                        if take > 0.0 {
                            *self.expedition.cargo.entry(k).or_insert(0.0) += take;
                        }
                    }
                    if let Some(site) = self
                        .galaxy_state
                        .sites
                        .get_mut(&system.0)
                        .and_then(|v| v.get_mut(site_idx))
                    {
                        site.depleted = true;
                    }
                    if kind == crate::galaxy::SiteKind::Derelict {
                        self.note_first_derelict(tuning);
                    }
                    self.log.push(
                        self.tick,
                        "You strip what you can carry and move on.",
                        tuning.log_capacity,
                    );
                }
            }
            Activity::Scavenge { .. } => {
                self.resources.add(ResourceKind::Fuel, tuning.scavenge_fuel);
                self.log.push(
                    self.tick,
                    "You bleed fuel from cold tanks and wreckage. Enough to limp on.",
                    tuning.log_capacity,
                );
            }
        }
    }

    fn tick_landing(&mut self, world: &World) {
        let tuning = world.tuning;
        let Some(landing) = self.expedition.landing.as_mut() else {
            return;
        };
        landing.suit -= tuning.suit_drain_per_tick;
        if landing.suit <= 0.0 {
            self.expedition.landing = None;
            self.log.push(
                self.tick,
                "Suit critical. You scramble back empty-handed.",
                tuning.log_capacity,
            );
            return;
        }
        if landing.traverse > 0 {
            landing.traverse -= 1;
            if landing.traverse == 0 {
                // Arrived at `landing.node`: resolve its encounter once.
                let (system, site_idx, biome, node) = (
                    landing.system,
                    landing.site_idx,
                    landing.biome,
                    landing.node,
                );
                self.resolve_node(world, system, site_idx, biome, node);
                // A fatal hazard this tick must force the retreat NOW — not at the next
                // tick's top — so the player can't Extract on a blown Suit and keep the haul.
                if self
                    .expedition
                    .landing
                    .as_ref()
                    .is_some_and(|l| l.suit <= 0.0)
                {
                    self.expedition.landing = None;
                    self.log.push(
                        self.tick,
                        "Suit critical. You scramble back empty-handed.",
                        tuning.log_capacity,
                    );
                }
            }
        }
    }

    /// Resolve the encounter at `node` once (records it in the site overlay).
    fn resolve_node(
        &mut self,
        world: &World,
        system: SystemId,
        site_idx: usize,
        biome: Biome,
        node: u32,
    ) {
        let tuning = world.tuning;
        // Already resolved? (entry/empty included via overlay)
        let already = self
            .galaxy_state
            .sites
            .get(&system.0)
            .and_then(|v| v.get(site_idx))
            .map(|s| s.resolved_nodes.contains(&node))
            .unwrap_or(true);
        if already {
            return;
        }
        let site = self.surface_site(world, system, site_idx, biome);
        let Some(n) = site.get(node) else { return };
        let kind = n.kind;
        let depth = n.depth;
        // Frontier gradient: richer the deeper the system is from home.
        let galaxy = self.galaxy(world);
        let grad = match (galaxy.get(crate::galaxy::HOME), galaxy.get(system)) {
            (Some(h), Some(s)) => 1.0 + crate::galaxy::distance(h, s) * tuning.reward_gradient,
            _ => 1.0,
        };
        let depth_mult = 1.0 + depth as f64 * tuning.surface_depth_reward;
        match kind {
            crate::landing::EncounterKind::ResourceCache => {
                let amt = tuning.surface_cache_base * depth_mult * grad;
                let kind_res = match biome {
                    Biome::Frozen => ResourceKind::Alloy,
                    Biome::Toxic => ResourceKind::Components,
                    Biome::Verdant => ResourceKind::Rations,
                    Biome::Ruined => ResourceKind::Isotopes,
                };
                self.satchel_add(kind_res, amt);
                self.log
                    .push(self.tick, "A cache. You pry it loose.", tuning.log_capacity);
            }
            crate::landing::EncounterKind::Hazard => {
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.suit -= tuning.suit_hazard_damage * depth_mult;
                }
                self.log.push(
                    self.tick,
                    "The world bites back. Suit screaming.",
                    tuning.log_capacity,
                );
            }
            crate::landing::EncounterKind::Ruin => {
                self.satchel_add(ResourceKind::Data, tuning.surface_ruin_data * depth_mult);
                self.log.push(
                    self.tick,
                    "Wreckage of something that came before.",
                    tuning.log_capacity,
                );
                if depth >= tuning.surface_depth_max {
                    self.note_once(
                        NoteId::RuinRhymes,
                        "It Rhymes",
                        "The layout is wrong for this world — the struts are in the wrong places, the load-bearing members too familiar. You measure the angles. They match your own station's frame exactly, down to the tolerances. You stand in the ruin of something that had not yet been built when you left.",
                        tuning,
                    );
                }
            }
            crate::landing::EncounterKind::Life => {
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.pending_choice = true;
                }
                self.log.push(
                    self.tick,
                    "Something living. It has not noticed you yet.",
                    tuning.log_capacity,
                );
                // Do NOT mark resolved yet — the choice (Task 6) resolves it.
                return;
            }
            crate::landing::EncounterKind::Empty => {}
        }
        self.mark_resolved(system, site_idx, node);
    }

    fn satchel_add(&mut self, k: ResourceKind, amt: f64) {
        if let Some(l) = self.expedition.landing.as_mut() {
            let space = (self.ship.cargo - l.satchel_total()).max(0.0);
            let take = amt.min(space);
            if take > 0.0 {
                *l.satchel.entry(k).or_insert(0.0) += take;
            }
        }
    }

    fn mark_resolved(&mut self, system: SystemId, site_idx: usize, node: u32) {
        if let Some(site) = self
            .galaxy_state
            .sites
            .get_mut(&system.0)
            .and_then(|v| v.get_mut(site_idx))
        {
            site.resolved_nodes.insert(node);
        }
    }

    pub(crate) fn log_flavor(&mut self, key: &str, tuning: &Tuning) {
        let line = self.origin.flavor(key).to_string();
        if !line.is_empty() {
            self.log.push(self.tick, line, tuning.log_capacity);
        }
    }

    pub(crate) fn update_disclosure_flags(&mut self, world: &World) {
        let tuning = world.tuning;
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
        // M2 disclosure.
        if self.flags.contains(&Flag::CoreEstablished) {
            self.flags.insert(Flag::BuildMenuUnlocked);
        }
        // A built module that unlocks a job means the economy is open.
        let economy_open = self.built.keys().any(|id| {
            world
                .modules
                .get(*id)
                .map(|mdef| mdef.unlocks.job.is_some())
                .unwrap_or(false)
        });
        if economy_open {
            self.flags.insert(Flag::EconomyUnlocked);
        }
        if self.telemetry.consumer_draw > 0.0 {
            self.flags.insert(Flag::PowerBudgetVisible);
        }
        if self.resources.data > 0.0 {
            self.flags.insert(Flag::Tier2Unlocked);
        }
        // M3 disclosure.
        if self.has_module(ModuleId::Lab) {
            self.flags.insert(Flag::ResearchUnlocked);
        }
        if !self.codex.entries.is_empty() {
            self.flags.insert(Flag::CodexUnlocked);
        }
    }

    fn apply_grant(&mut self, grant: &Grant) {
        match grant {
            Grant::ReactorMaxOutput(v) => self.reactor.max_output += v,
            Grant::ReactorCapacity(v) => self.reactor.capacity += v,
            Grant::ResourceCapBonus(v) => {
                let c = self.resources.cap + v;
                self.resources.set_cap(c);
            }
            Grant::JobOutputMult { job, mult } => {
                let e = self.research_output_mult.entry(*job).or_insert(1.0);
                *e *= mult;
            }
            Grant::SetFlag(f) => {
                self.flags.insert(*f);
            }
        }
    }

    fn note_once(&mut self, id: NoteId, title: &str, text: &str, tuning: &Tuning) {
        if self.codex.add(Discovery {
            id: DiscoveryId::Note(id),
            title: title.into(),
            text: text.into(),
            tick: self.tick,
        }) {
            self.log.push(
                self.tick,
                "Something in you takes note.",
                tuning.log_capacity,
            );
        }
    }

    fn note_first_derelict(&mut self, tuning: &Tuning) {
        let added = self.codex.add(Discovery {
            id: DiscoveryId::Note(NoteId::FirstDerelict),
            title: "Someone Was Here".into(),
            text: "The derelict's logs are in your own format. Not similar — identical. You tell yourself it is a coincidence, a convergence, anything. The dark keeps the silence it always keeps.".into(),
            tick: self.tick,
        });
        if added {
            self.log.push(
                self.tick,
                "The hull's logs read like your own hand wrote them.",
                tuning.log_capacity,
            );
        }
    }

    fn note_first_crew_death(&mut self, tuning: &Tuning) {
        let added = self.codex.add(Discovery {
            id: DiscoveryId::Note(NoteId::FirstCrewDeath),
            title: "The First Loss".into(),
            text: "You kept the work going and the air ran out anyway. Whoever they were, the dark has them now. You note the gap in the roster and move on. There is always more to do.".into(),
            tick: self.tick,
        });
        if added {
            self.log.push(
                self.tick,
                "Something in you goes quiet.",
                tuning.log_capacity,
            );
        }
    }

    /// State-checked uneasy notes (JumpTheory glimpse, heavy extraction). Each
    /// fires exactly once via `Codex::add` dedup. Called once per tick.
    fn check_triggered_notes(&mut self, tuning: &Tuning) {
        if self.flags.contains(&Flag::JumpTheoryKnown)
            && self.codex.add(Discovery {
                id: DiscoveryId::Note(NoteId::JumpTheoryGlimpse),
                title: "A Way Out".into(),
                text: "The route resolves into something you could actually fly. You catch yourself wanting it badly, the way a thing wants to spread. You wonder, briefly, whether the wanting is yours.".into(),
                tick: self.tick,
            })
        {
            self.log.push(self.tick, "The dark has an other side, and you ache toward it.", tuning.log_capacity);
        }
        if self.total_scrap_gained >= HEAVY_CONSUMPTION_SCRAP
            && self.codex.add(Discovery {
                id: DiscoveryId::Note(NoteId::HeavyConsumption),
                title: "What It Costs".into(),
                text: "You tally what you have torn loose to get this far. The moon is lighter for your being here. You tell yourself it was only scrap, only rock. You are good at telling yourself things.".into(),
                tick: self.tick,
            })
        {
            self.log.push(self.tick, "The pile of what you've taken has its own weight.", tuning.log_capacity);
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
                description: "test".into(),
                tier: 1,
                cost: vec![(ResourceKind::Scrap, 20.0)],
                cost_scaling: 1.0,
                repeatable: false,
                requires_modules: vec![],
                requires_flag: None,
                unlocks: ModuleUnlocks {
                    job: Some(JobId::Refiner),
                    ..Default::default()
                },
            },
            Module {
                id: ModuleId::Quarters,
                name: "Quarters".into(),
                description: "test".into(),
                tier: 1,
                cost: vec![(ResourceKind::Scrap, 10.0)],
                cost_scaling: 1.6,
                repeatable: true,
                requires_modules: vec![],
                requires_flag: None,
                unlocks: ModuleUnlocks {
                    housing: 4,
                    ..Default::default()
                },
            },
            Module {
                id: ModuleId::Lab,
                name: "Lab".into(),
                description: "test".into(),
                tier: 2,
                cost: vec![(ResourceKind::Scrap, 20.0)],
                cost_scaling: 1.0,
                repeatable: false,
                requires_modules: vec![],
                requires_flag: None,
                unlocks: ModuleUnlocks {
                    job: Some(JobId::Scientist),
                    ..Default::default()
                },
            },
            Module {
                id: ModuleId::Atmosphere,
                name: "Atmosphere".into(),
                description: "test".into(),
                tier: 1,
                cost: vec![(ResourceKind::Scrap, 10.0)],
                cost_scaling: 1.0,
                repeatable: false,
                requires_modules: vec![],
                requires_flag: None,
                unlocks: ModuleUnlocks::default(),
            },
            Module {
                id: ModuleId::Thermal,
                name: "Thermal".into(),
                description: "test".into(),
                tier: 1,
                cost: vec![(ResourceKind::Scrap, 10.0)],
                cost_scaling: 1.0,
                repeatable: false,
                requires_modules: vec![],
                requires_flag: None,
                unlocks: ModuleUnlocks::default(),
            },
            Module {
                id: ModuleId::Shipyard,
                name: "Shipyard".into(),
                description: "test".into(),
                tier: 2,
                cost: vec![(ResourceKind::Alloy, 10.0)],
                cost_scaling: 1.0,
                repeatable: false,
                requires_modules: vec![],
                requires_flag: Some(Flag::JumpTheoryKnown),
                unlocks: ModuleUnlocks::default(),
            },
            Module {
                id: ModuleId::SensorArray,
                name: "Sensor Array".into(),
                description: "test".into(),
                tier: 2,
                cost: vec![(ResourceKind::Alloy, 10.0)],
                cost_scaling: 1.0,
                repeatable: false,
                requires_modules: vec![],
                requires_flag: Some(Flag::JumpTheoryKnown),
                unlocks: ModuleUnlocks::default(),
            },
        ])
    }
    fn techs() -> crate::research::TechRegistry {
        use crate::research::{Grant, Tech, TechId};
        crate::research::TechRegistry(vec![
            Tech {
                id: TechId::CoreTuning,
                name: "Core Tuning".into(),
                blurb: "".into(),
                data_cost: 5.0,
                research_ticks: 3,
                requires: vec![],
                grant: Grant::ReactorMaxOutput(2.0),
                lore: "lore".into(),
            },
            Tech {
                id: TechId::RefinedProcesses,
                name: "Refined".into(),
                blurb: "".into(),
                data_cost: 6.0,
                research_ticks: 2,
                requires: vec![],
                grant: Grant::JobOutputMult {
                    job: JobId::Salvager,
                    mult: 2.0,
                },
                lore: "lore".into(),
            },
        ])
    }
    fn world<'a>(
        t: &'a Tuning,
        j: &'a JobRegistry,
        m: &'a ModuleRegistry,
        tc: &'a crate::research::TechRegistry,
    ) -> World<'a> {
        World {
            tuning: t,
            jobs: j,
            modules: m,
            techs: tc,
        }
    }

    fn sim_home() -> crate::galaxy::SystemId {
        crate::galaxy::HOME
    }

    /// Find a revealed, reachable, non-home system for a given seed (drive range
    /// is generous vs reveal radius in defaults, so a revealed neighbor is reachable).
    fn a_reachable_target(s: &GameState, w: &World) -> crate::galaxy::SystemId {
        let g = s.galaxy(w);
        let here = g.get(s.expedition.location).unwrap();
        s.galaxy_state
            .revealed
            .iter()
            .map(|&id| crate::galaxy::SystemId(id))
            .filter(|&id| id != s.expedition.location)
            .find(|&id| crate::galaxy::distance(here, g.get(id).unwrap()) <= s.ship.drive_range)
            .expect("a reachable revealed neighbor exists for this seed")
    }

    #[test]
    fn jump_costs_fuel_transits_then_arrives() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        s.resources.add(ResourceKind::Fuel, 100.0);
        let target = a_reachable_target(&s, &w);
        let g = s.galaxy(&w);
        let dist =
            crate::galaxy::distance(g.get(crate::galaxy::HOME).unwrap(), g.get(target).unwrap());
        let cost = crate::galaxy::jump_cost(dist, &t);
        let fuel_before = s.resources.amount(ResourceKind::Fuel);
        s.apply(&w, Action::Jump(target)).unwrap();
        // Fuel deducted immediately; still at home, now in transit.
        assert!((fuel_before - s.resources.amount(ResourceKind::Fuel) - cost).abs() < 1e-9);
        assert_eq!(s.expedition.location, crate::galaxy::HOME);
        assert!(s.away());
        let ticks = t.transit_ticks_base + (dist * t.transit_ticks_factor).round() as u32;
        s.advance(&w, (ticks - 1) as u64);
        assert_ne!(s.expedition.location, target); // not yet
        s.advance(&w, 1);
        assert_eq!(s.expedition.location, target); // arrived
        assert!(s.expedition.activity.is_none());
        assert!(s.galaxy_state.sites.contains_key(&target.0)); // materialized on arrival
    }

    #[test]
    fn jump_rejected_when_unreachable_or_unaffordable() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        let target = a_reachable_target(&s, &w);
        // No fuel.
        assert_eq!(
            s.apply(&w, Action::Jump(target)),
            Err(Rejected("not enough fuel to jump"))
        );
    }

    #[test]
    fn new_game_initializes_m4_state() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let s = GameState::new(&w, 77);
        assert_eq!(s.world_seed, 77);
        assert!(!s.ship.built);
        assert_eq!(s.ship.drive_range, t.ship_base_drive_range);
        assert_eq!(s.expedition.location, sim_home());
        assert!(s.expedition.activity.is_none());
        assert!(!s.away());
        // Home is revealed at the start; the galaxy is regenerable, not stored.
        assert!(s.galaxy_state.revealed.contains(&0));
        assert_eq!(s.galaxy(&w).systems.len(), t.galaxy_system_count as usize);
    }

    #[test]
    fn new_game_is_versioned_v7() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let s = GameState::new(&world(&t, &j, &m, &tc), 1);
        assert_eq!(s.version, 7);
        assert_eq!(s.resources.cap, t.resource_base_cap);
    }

    #[test]
    fn advance_n_equals_n_times_advance_1() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let mut bulk = GameState::new(&world(&t, &j, &m, &tc), 1);
        let mut step = GameState::new(&world(&t, &j, &m, &tc), 1);
        bulk.advance(&world(&t, &j, &m, &tc), 25);
        for _ in 0..25 {
            step.advance(&world(&t, &j, &m, &tc), 1);
        }
        assert_eq!(bulk, step);
    }

    #[test]
    fn build_refinery_unlocks_refiner_assignment() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
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
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
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
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
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
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
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

    fn bank(s: &mut GameState, w: &World, tn: &Tuning, times: u32) {
        for _ in 0..times {
            s.apply(w, Action::Salvage).unwrap();
            s.advance(w, tn.salvage_cooldown_ticks as u64);
        }
    }

    #[test]
    fn advance_accumulates_ticks() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.advance(&w, 5);
        assert_eq!(s.tick, 5);
    }

    #[test]
    fn ignite_only_from_cold() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let origin = OriginDef {
            start: crate::origin::OriginStart {
                scrap: 0.0,
                reactor_capacity: 60.0,
                reactor_charge: 0.0,
            },
            ..default_origin_def(&t)
        };
        let mut s = GameState::new_with_origin(&origin, &w, 1);
        assert_eq!(s.reactor.status, ReactorStatus::Cold);
        assert!(s.apply(&w, Action::Ignite).is_ok());
        assert_eq!(s.reactor.status, ReactorStatus::Igniting);
        assert!(s.flags.contains(&Flag::SalvageUnlocked));
        assert_eq!(
            s.apply(&w, Action::Ignite),
            Err(Rejected("the core is already lit"))
        );
    }

    #[test]
    fn feed_rejected_without_scrap() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        assert_eq!(s.resources.scrap, 0.0);
        assert_eq!(
            s.apply(&w, Action::FeedCore),
            Err(Rejected("not enough scrap to feed the core"))
        );
    }

    #[test]
    fn salvage_sets_cooldown_and_blocks_repeat() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        assert!(s.apply(&w, Action::Salvage).is_ok());
        assert_eq!(s.salvage_cooldown, t.salvage_cooldown_ticks);
        assert_eq!(
            s.apply(&w, Action::Salvage),
            Err(Rejected("salvage is on cooldown"))
        );
        s.advance(&w, t.salvage_cooldown_ticks as u64);
        assert_eq!(s.salvage_cooldown, 0);
        assert!(s.apply(&w, Action::Salvage).is_ok());
    }

    #[test]
    fn crew_source_requires_availability_and_scrap() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        assert_eq!(
            s.apply(&w, Action::ActivateCrewSource),
            Err(Rejected("the crew-source is not available yet"))
        );
        bank(
            &mut s,
            &w,
            &t,
            (t.crew_source_cost / t.salvage_yield) as u32,
        );
        assert!(s.flags.contains(&Flag::CrewSourceAvailable));
        assert!(s.apply(&w, Action::ActivateCrewSource).is_ok());
        assert_eq!(
            s.apply(&w, Action::ActivateCrewSource),
            Err(Rejected("the crew-source is already active"))
        );
    }

    #[test]
    fn hot_start_below_establish_is_igniting_and_unlocked() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let origin = OriginDef {
            start: crate::origin::OriginStart {
                scrap: 15.0,
                reactor_capacity: 140.0,
                reactor_charge: 35.0,
            },
            ..default_origin_def(&t)
        };
        let s = GameState::new_with_origin(&origin, &w, 1);
        assert_eq!(s.reactor.status, ReactorStatus::Igniting);
        assert!(!s.reactor.established);
        assert!(s.flags.contains(&Flag::SalvageUnlocked));
        assert!(!s.flags.contains(&Flag::CoreEstablished));
    }

    #[test]
    fn telemetry_records_last_tick() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.crew.total = 2;
        s.resources.rations = 1.0; // ensure crew are not starving (upkeep = 2 × 0.1 = 0.2)
        s.apply(
            &w,
            Action::AssignCrew {
                job: JobId::Salvager,
                delta: 2,
            },
        )
        .unwrap();
        s.advance(&w, 1);
        // 2 salvagers produced +2 scrap this tick.
        assert_eq!(
            s.telemetry
                .net
                .get(&ResourceKind::Scrap)
                .copied()
                .unwrap_or(0.0),
            2.0
        );
        assert!(s.telemetry.ran.contains(&JobId::Salvager));
        // Salvager draws no power, so consumer_draw is 0 and nothing is paused.
        assert_eq!(s.telemetry.consumer_draw, 0.0);
        assert!(s.telemetry.paused.is_empty());
    }

    #[test]
    fn telemetry_is_deterministic_across_batching() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let mut bulk = GameState::new(&world(&t, &j, &m, &tc), 1);
        let mut step = GameState::new(&world(&t, &j, &m, &tc), 1);
        bulk.advance(&world(&t, &j, &m, &tc), 10);
        for _ in 0..10 {
            step.advance(&world(&t, &j, &m, &tc), 1);
        }
        assert_eq!(bulk.telemetry, step.telemetry);
        assert_eq!(bulk, step);
    }

    #[test]
    fn m2_disclosure_flags_set_at_milestones() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1); // full-charge default origin -> established
        // BuildMenuUnlocked is set once the core is established.
        assert!(s.flags.contains(&Flag::BuildMenuUnlocked));
        // No economy / power / tier-2 yet.
        assert!(!s.flags.contains(&Flag::EconomyUnlocked));
        assert!(!s.flags.contains(&Flag::PowerBudgetVisible));
        assert!(!s.flags.contains(&Flag::Tier2Unlocked));

        // Build a Refinery (job-unlocking module) -> EconomyUnlocked.
        s.resources.add(ResourceKind::Scrap, 30.0);
        s.apply(&w, Action::Build(ModuleId::Refinery)).unwrap();
        assert!(s.flags.contains(&Flag::EconomyUnlocked));

        // Assign a Refiner with scrap + crew and advance -> a power-drawing job
        // runs -> PowerBudgetVisible.
        s.crew.total = 1;
        s.resources.add(ResourceKind::Scrap, 20.0);
        s.apply(
            &w,
            Action::AssignCrew {
                job: JobId::Refiner,
                delta: 1,
            },
        )
        .unwrap();
        s.advance(&w, 1);
        assert!(s.flags.contains(&Flag::PowerBudgetVisible));

        // Data present -> Tier2Unlocked.
        s.resources.add(ResourceKind::Data, 1.0);
        s.advance(&w, 1);
        assert!(s.flags.contains(&Flag::Tier2Unlocked));
    }

    #[test]
    fn m2_flag_graph_is_reachable_from_a_running_core() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        // A full-charge default origin is already established (BuildMenuUnlocked).
        let mut s = GameState::new(&w, 1);
        assert!(s.flags.contains(&Flag::BuildMenuUnlocked));

        // BuildMenu is reachable -> build a Refinery (Scrap-only, no prereqs).
        s.resources.add(ResourceKind::Scrap, 30.0);
        s.apply(&w, Action::Build(ModuleId::Refinery)).unwrap();
        assert!(s.flags.contains(&Flag::EconomyUnlocked));

        // Economy reachable -> assign a Refiner; feed it scrap + rations; advance.
        s.crew.total = 1;
        s.resources.add(ResourceKind::Scrap, 20.0);
        s.resources.rations = 5.0;
        s.apply(
            &w,
            Action::AssignCrew {
                job: JobId::Refiner,
                delta: 1,
            },
        )
        .unwrap();
        s.advance(&w, 1);
        // A power-drawing job ran -> PowerBudgetVisible; it produced Fuel/Data? Refiner makes Fuel.
        assert!(s.flags.contains(&Flag::PowerBudgetVisible));

        // Reaching Data unlocks Tier2 (simulate Data arriving).
        s.resources.add(ResourceKind::Data, 1.0);
        s.advance(&w, 1);
        assert!(s.flags.contains(&Flag::Tier2Unlocked));

        // The UI dashboard outer-gate condition (any panel flag) is satisfiable.
        assert!(
            s.flags.contains(&Flag::BuildMenuUnlocked)
                || s.flags.contains(&Flag::EconomyUnlocked)
                || s.flags.contains(&Flag::PowerBudgetVisible)
                || s.flags.contains(&Flag::SalvageUnlocked)
        );
    }

    #[test]
    fn unfuelled_core_drains_to_failing_then_cold_with_log() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let mut flavor = std::collections::BTreeMap::new();
        flavor.insert("failing".to_string(), "FAILING".to_string());
        flavor.insert("cold".to_string(), "COLD".to_string());
        let origin = OriginDef {
            flavor,
            start: crate::origin::OriginStart {
                scrap: 0.0,
                reactor_capacity: 100.0,
                reactor_charge: 100.0,
            },
            ..default_origin_def(&t)
        };
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new_with_origin(&origin, &w, 1);
        // No fuel, no jobs: the baseline reactor load drains charge to Cold.
        s.advance(&w, 130);
        let texts: Vec<_> = s.log.entries().iter().map(|e| e.text.clone()).collect();
        assert!(texts.iter().any(|x| x == "FAILING"));
        assert!(texts.iter().any(|x| x == "COLD"));
    }

    #[test]
    fn research_completes_and_applies_grant() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.apply(&w, Action::Build(ModuleId::Lab))
            .expect_err("no scrap"); // (built later)
        // Give scrap to build Lab, and data to research.
        s.resources.add(ResourceKind::Scrap, 50.0);
        s.apply(&w, Action::Build(ModuleId::Lab)).unwrap();
        s.resources.add(ResourceKind::Data, 10.0);
        let before_out = s.reactor.max_output;
        s.apply(&w, Action::StartResearch(TechId::CoreTuning))
            .unwrap();
        assert!(s.research.active.is_some());
        // research_ticks = 3 in the test registry.
        s.advance(&w, 3);
        assert!(s.research.is_completed(TechId::CoreTuning));
        assert_eq!(s.reactor.max_output, before_out + 2.0);
        assert!(
            s.codex
                .has(crate::codex::DiscoveryId::Tech(TechId::CoreTuning))
        );
        assert!(s.flags.contains(&Flag::CodexUnlocked));
    }

    #[test]
    fn research_one_slot_and_prereq_and_data() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.resources.add(ResourceKind::Scrap, 50.0);
        s.apply(&w, Action::Build(ModuleId::Lab)).unwrap();
        // No data yet.
        assert_eq!(
            s.apply(&w, Action::StartResearch(TechId::CoreTuning)),
            Err(Rejected("not enough data"))
        );
        s.resources.add(ResourceKind::Data, 100.0);
        s.apply(&w, Action::StartResearch(TechId::CoreTuning))
            .unwrap();
        // One slot.
        assert_eq!(
            s.apply(&w, Action::StartResearch(TechId::RefinedProcesses)),
            Err(Rejected("research already in progress"))
        );
    }

    #[test]
    fn job_output_mult_grant_boosts_economy() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.resources.add(ResourceKind::Scrap, 50.0);
        s.apply(&w, Action::Build(ModuleId::Lab)).unwrap();
        s.resources.add(ResourceKind::Data, 100.0);
        s.apply(&w, Action::StartResearch(TechId::RefinedProcesses))
            .unwrap();
        s.advance(&w, 2); // completes; Salvager mult x2.0 in test registry
        assert_eq!(
            s.research_output_mult.get(&JobId::Salvager).copied(),
            Some(2.0)
        );
        s.crew.total = 1;
        s.resources.add(ResourceKind::Rations, 10.0); // prevent starving efficiency
        s.apply(
            &w,
            Action::AssignCrew {
                job: JobId::Salvager,
                delta: 1,
            },
        )
        .unwrap();
        let before = s.resources.amount(ResourceKind::Scrap);
        s.advance(&w, 1);
        // 1 salvager x1 base x mult 2.0 = +2 scrap this tick.
        assert!((s.resources.amount(ResourceKind::Scrap) - before - 2.0).abs() < 1e-9);
    }

    /// Regression guard: first crew loss lands exactly `crew_loss_interval` ticks
    /// after the atmosphere grace clock hits 0 — not interval+1.
    ///
    /// Timing walkthrough (grace_max=G, interval=I):
    ///   Ticks 1..G   : grace counts down from G to 1.
    ///   Tick  G      : grace hits 0 → crew_loss_timer = I, warning log fires.
    ///   Ticks G+1..  : else branch; each tick decrements timer.
    ///   Tick  G+I    : timer decrements to 0 → lose_one() fires. ✓ exactly I ticks after G.
    #[test]
    fn atmosphere_crew_loss_first_fires_at_exactly_interval_ticks_after_grace_expires() {
        use crate::module::{Module, ModuleUnlocks};

        let mut t = Tuning::default();
        // Use small values so the test runs in few ticks.
        t.atmosphere_grace_max = 3;
        t.crew_loss_interval = 5;
        // Ensure no fuel / charge so the reactor stays Cold immediately.
        t.reactor_capacity = 100.0;
        t.reactor_baseline_draw = 0.0; // prevent incidental drain
        t.reactor_drain_per_tick = 0.0;

        let j = jobs();
        // Build a module registry that includes Atmosphere (zero cost for direct injection).
        let m = ModuleRegistry(vec![Module {
            id: ModuleId::Atmosphere,
            name: "Atmosphere".into(),
            description: "test".into(),
            tier: 1,
            cost: vec![],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: None,
            unlocks: ModuleUnlocks::default(),
        }]);
        let tc = techs();
        let w = world(&t, &j, &m, &tc);

        // Start with a Cold reactor (zero charge).
        let origin = OriginDef {
            start: crate::origin::OriginStart {
                scrap: 0.0,
                reactor_capacity: 100.0,
                reactor_charge: 0.0,
            },
            ..default_origin_def(&t)
        };
        let mut s = GameState::new_with_origin(&origin, &w, 1);
        assert_eq!(s.reactor.status, ReactorStatus::Cold);

        // Inject crew and Atmosphere module directly (no scrap cost needed).
        s.crew.total = 3;
        *s.built.entry(ModuleId::Atmosphere).or_insert(0) += 1;
        // Grace starts at atmosphere_grace_max.
        assert_eq!(s.atmosphere_grace, t.atmosphere_grace_max);

        let crew_before = s.crew.total;

        // Advance through the grace period. At tick grace_max, grace hits 0 and
        // crew_loss_timer is set to crew_loss_interval; no loss yet.
        s.advance(&w, t.atmosphere_grace_max as u64);
        assert_eq!(s.atmosphere_grace, 0, "grace should be exhausted");
        assert_eq!(s.crew.total, crew_before, "no crew lost during grace");

        // Advance (interval - 1) more ticks: timer counting down, no loss yet.
        s.advance(&w, (t.crew_loss_interval - 1) as u64);
        assert_eq!(
            s.crew.total, crew_before,
            "no crew lost before the interval is fully elapsed"
        );

        // One more tick: timer hits 0, first crew lost.
        s.advance(&w, 1);
        assert_eq!(
            s.crew.total,
            crew_before - 1,
            "exactly one crew lost at crew_loss_interval ticks after grace expired"
        );
    }

    #[test]
    fn atmosphere_failure_drains_clock_then_kills_crew() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        // Build Atmosphere, give crew, then force a Cold reactor (no ignite) so
        // atmosphere is failing every tick.
        let mut s = GameState::new(&w, 1);
        s.resources.add(ResourceKind::Scrap, 100.0);
        s.apply(&w, Action::Build(ModuleId::Atmosphere)).unwrap();
        s.crew.total = 2;
        // Drain the core to Cold: with no fuel the baseline load empties it.
        s.advance(&w, 200);
        assert_eq!(s.reactor.status, ReactorStatus::Cold);
        // After grace + at least one loss interval, crew should have died.
        assert!(
            s.crew.total < 2,
            "atmosphere failure should kill crew when Cold"
        );
        assert!(s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstCrewDeath
        )));
    }

    #[test]
    fn atmosphere_clock_resets_when_powered() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1); // default origin: established, powered
        s.resources.add(ResourceKind::Scrap, 100.0);
        s.apply(&w, Action::Build(ModuleId::Atmosphere)).unwrap();
        s.crew.total = 2;
        s.advance(&w, 5); // powered (charge high, fuel? baseline only) -> grace stays max
        assert_eq!(s.atmosphere_grace, t.atmosphere_grace_max);
        assert_eq!(s.crew.total, 2);
    }

    #[test]
    fn thermal_failure_accelerates_drain() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        // Two identical games; one builds Thermal (which has no fuel to stay
        // powered, so it fails and drains faster).
        let mut with = GameState::new(&w, 1);
        let mut without = GameState::new(&w, 1);
        with.resources.add(ResourceKind::Scrap, 100.0);
        with.apply(&w, Action::Build(ModuleId::Thermal)).unwrap();
        with.advance(&w, 5);
        without.advance(&w, 5);
        assert!(
            with.reactor.charge < without.reactor.charge,
            "thermal failure should drain charge faster"
        );
    }

    #[test]
    fn upgrade_drive_needs_navigation_and_isotopes() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.resources.add(ResourceKind::Isotopes, 100.0);
        // Navigation not researched yet.
        assert_eq!(
            s.apply(&w, Action::UpgradeDrive),
            Err(Rejected("navigation is not yet understood"))
        );
        s.flags.insert(Flag::NavigationKnown);
        let before = s.ship.drive_range;
        assert!(s.apply(&w, Action::UpgradeDrive).is_ok());
        assert_eq!(s.ship.drive_range, before + t.drive_upgrade_step);
        assert_eq!(
            s.resources.amount(ResourceKind::Isotopes),
            100.0 - t.drive_upgrade_isotopes
        );
    }

    #[test]
    fn scan_needs_sensor_costs_data_and_reveals_contents() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        let target = a_reachable_target(&s, &w);
        s.resources.add(ResourceKind::Data, 100.0);
        // No sensor array yet.
        assert_eq!(
            s.apply(&w, Action::ScanSystem(target)),
            Err(Rejected("no sensor array to scan with"))
        );
        s.flags.insert(Flag::JumpTheoryKnown);
        s.resources.add(ResourceKind::Alloy, 100.0);
        s.apply(&w, Action::Build(ModuleId::SensorArray)).unwrap();
        let data_before = s.resources.amount(ResourceKind::Data);
        s.apply(&w, Action::ScanSystem(target)).unwrap();
        assert!(
            (data_before - s.resources.amount(ResourceKind::Data) - t.scan_data_cost).abs() < 1e-9
        );
        s.advance(&w, t.scan_ticks as u64);
        assert!(s.galaxy_state.scanned.contains(&target.0));
        assert!(s.galaxy_state.sites.contains_key(&target.0));
        assert!(s.expedition.activity.is_none());
    }

    #[test]
    fn resolve_site_fills_cargo_and_deposits_on_return() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        s.resources.add(ResourceKind::Fuel, 200.0);
        let target = a_reachable_target(&s, &w);
        s.apply(&w, Action::Jump(target)).unwrap();
        // Arrive.
        s.advance(&w, 64);
        assert_eq!(s.expedition.location, target);
        let sites = s
            .galaxy_state
            .sites
            .get(&target.0)
            .cloned()
            .unwrap_or_default();
        if sites.is_empty() {
            return; // seed 4's neighbor is empty; covered by the deterministic seed-search test below
        }
        s.apply(&w, Action::ResolveSite(target, 0)).unwrap();
        s.advance(&w, t.site_resolve_ticks as u64);
        assert!(s.expedition.cargo_total() > 0.0);
        assert!(s.galaxy_state.sites.get(&target.0).unwrap()[0].depleted);
        // Jump home; on arrival cargo deposits into station resources.
        let iso_home_before = s.resources.amount(ResourceKind::Isotopes);
        s.apply(&w, Action::Jump(crate::galaxy::HOME)).unwrap();
        s.advance(&w, 64);
        assert_eq!(s.expedition.location, crate::galaxy::HOME);
        assert!(s.expedition.cargo.is_empty());
        // Something was deposited (Isotopes or Components, depending on site kind).
        let deposited = s.resources.amount(ResourceKind::Isotopes) > iso_home_before
            || s.resources.amount(ResourceKind::Components) > 0.0;
        assert!(deposited);
    }

    #[test]
    fn resolving_an_asteroid_field_yields_isotopes() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        // Inject a known asteroid site at home (test-only) and resolve it in place.
        s.galaxy_state.sites.insert(
            0,
            vec![crate::galaxy::Site::new(
                crate::galaxy::SiteKind::AsteroidField,
            )],
        );
        s.apply(&w, Action::ResolveSite(crate::galaxy::HOME, 0))
            .unwrap();
        s.advance(&w, t.site_resolve_ticks as u64);
        assert!(
            s.expedition
                .cargo
                .get(&ResourceKind::Isotopes)
                .copied()
                .unwrap_or(0.0)
                > 0.0
        );
    }

    #[test]
    fn resolving_a_derelict_records_the_uneasy_note() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.galaxy_state.sites.insert(
            0,
            vec![crate::galaxy::Site::new(crate::galaxy::SiteKind::Derelict)],
        );
        s.apply(&w, Action::ResolveSite(crate::galaxy::HOME, 0))
            .unwrap();
        s.advance(&w, t.site_resolve_ticks as u64);
        assert!(s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstDerelict
        )));
    }

    #[test]
    fn shipyard_is_flag_gated_and_builds_the_ship() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.resources.add(ResourceKind::Alloy, 100.0);
        // Without JumpTheoryKnown the build is refused.
        assert_eq!(
            s.apply(&w, Action::Build(ModuleId::Shipyard)),
            Err(Rejected("jump theory is not yet understood"))
        );
        s.flags.insert(Flag::JumpTheoryKnown);
        assert!(s.apply(&w, Action::Build(ModuleId::Shipyard)).is_ok());
        assert!(s.ship.built);
        assert!(s.flags.contains(&Flag::ShipyardComplete));
    }

    #[test]
    fn station_management_locks_while_away() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        s.resources.add(ResourceKind::Fuel, 200.0);
        let target = a_reachable_target(&s, &w);
        s.apply(&w, Action::Jump(target)).unwrap();
        s.advance(&w, 64); // arrive — now away from home
        assert!(s.away());
        // Station actions are refused while away.
        s.resources.add(ResourceKind::Scrap, 100.0);
        assert_eq!(
            s.apply(&w, Action::Salvage),
            Err(Rejected("you are away from the station"))
        );
        assert_eq!(
            s.apply(&w, Action::Build(ModuleId::Refinery)),
            Err(Rejected("you are away from the station"))
        );
        // Jumping home is allowed while away.
        assert!(s.apply(&w, Action::Jump(crate::galaxy::HOME)).is_ok());
    }

    #[test]
    fn scavenge_only_when_stranded_and_refuels_home() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        s.resources.add(ResourceKind::Fuel, 200.0);
        let target = a_reachable_target(&s, &w);
        s.apply(&w, Action::Jump(target)).unwrap();
        s.advance(&w, 64); // arrive
        // Drain fuel to strand the ship.
        s.resources
            .try_spend(ResourceKind::Fuel, s.resources.amount(ResourceKind::Fuel));
        // At home, scavenge is not offered (you're not stranded there). Test the
        // away+stranded path: scavenge is accepted and yields fuel.
        let fuel_before = s.resources.amount(ResourceKind::Fuel);
        s.apply(&w, Action::Scavenge).unwrap();
        s.advance(&w, t.scavenge_ticks as u64);
        assert!(
            (s.resources.amount(ResourceKind::Fuel) - fuel_before - t.scavenge_fuel).abs() < 1e-9
        );
    }

    #[test]
    fn scavenge_refused_when_not_stranded() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        s.resources.add(ResourceKind::Fuel, 200.0);
        let target = a_reachable_target(&s, &w);
        s.apply(&w, Action::Jump(target)).unwrap();
        s.advance(&w, 64); // arrive, still fuel-rich
        assert_eq!(
            s.apply(&w, Action::Scavenge),
            Err(Rejected("no need to scavenge"))
        );
    }

    impl GameState {
        #[cfg(test)]
        fn surface_for_test(&self, world: &World) -> crate::landing::SurfaceSite {
            let l = self.expedition.landing.as_ref().unwrap();
            self.surface_site(world, l.system, l.site_idx, l.biome)
        }

        /// Scan the regenerated surface for the first Life node (any depth) and
        /// return its id, or None if no Life node exists on this surface.
        #[cfg(test)]
        pub fn find_life_node_for_test(&self, world: &World) -> Option<u32> {
            let l = self.expedition.landing.as_ref()?;
            let site = self.surface_site(world, l.system, l.site_idx, l.biome);
            site.nodes
                .iter()
                .find(|n| n.kind == crate::landing::EncounterKind::Life)
                .map(|n| n.id)
        }

        /// Teleport the current landing to `node` and advance 1 tick so
        /// `resolve_node` fires, leaving `pending_choice` set for a Life node.
        #[cfg(test)]
        pub fn walk_to_for_test(&mut self, world: &World, node: u32) {
            if let Some(l) = self.expedition.landing.as_mut() {
                l.node = node;
                l.traverse = 1;
            }
            self.advance(world, 1);
        }
    }

    /// Inject a Planet site at the current location and return its index.
    fn plant_planet(s: &mut GameState, biome: crate::galaxy::Biome) -> usize {
        let loc = s.expedition.location.0;
        let v = s.galaxy_state.sites.entry(loc).or_default();
        v.push(crate::galaxy::Site::new(crate::galaxy::SiteKind::Planet(
            biome,
        )));
        v.len() - 1
    }

    #[test]
    fn fatal_hazard_forces_retreat_same_tick_no_extract() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        // Find a seed whose Toxic surface has a Hazard node directly off the entry.
        for seed in 1..80u64 {
            let mut s = GameState::new(&w, seed);
            s.ship.built = true;
            s.expedition.location = crate::galaxy::SystemId(1);
            let idx = plant_planet(&mut s, crate::galaxy::Biome::Toxic);
            s.apply(&w, Action::Land(idx)).unwrap();
            let site = s.surface_for_test(&w);
            let hazard = site.nodes[0].edges.iter().copied().find(|&e| {
                site.get(e).map(|n| n.kind) == Some(crate::landing::EncounterKind::Hazard)
            });
            let Some(hz) = hazard else { continue };
            // Suit survives the ambient drain to arrival, then the hazard hit (>=18) blows it.
            s.expedition.landing.as_mut().unwrap().suit = 10.0;
            s.apply(&w, Action::MoveToNode(hz)).unwrap();
            s.advance(&w, t.traverse_ticks as u64); // arrive + resolve hazard this tick
            assert!(
                !s.landed(),
                "a fatal hazard must force retreat the same tick (seed {seed})"
            );
            assert!(
                s.expedition.cargo.is_empty(),
                "no haul banked on a blown suit"
            );
            return;
        }
        panic!("no Toxic seed with a hazard adjacent to the entry in 1..80");
    }

    #[test]
    fn advance_n_equals_n_times_advance_1_during_a_landing() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let setup = |s: &mut GameState| {
            s.ship.built = true;
            s.expedition.location = crate::galaxy::SystemId(1);
            let idx = plant_planet(s, crate::galaxy::Biome::Verdant);
            s.apply(&w, Action::Land(idx)).unwrap();
            // Begin a traverse so a move + resolve happens during advance.
            let site = s.surface_for_test(&w);
            let target = site.nodes[0].edges[0];
            s.apply(&w, Action::MoveToNode(target)).unwrap();
        };
        let mut bulk = GameState::new(&w, 1);
        setup(&mut bulk);
        let mut step = GameState::new(&w, 1);
        setup(&mut step);
        bulk.advance(&w, 20);
        for _ in 0..20 {
            step.advance(&w, 1);
        }
        assert_eq!(bulk, step);
    }

    #[test]
    fn land_then_extract_banks_nothing_when_satchel_empty() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        // Move the player to a non-home system so they're "away".
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Frozen);
        assert!(s.apply(&w, Action::Land(idx)).is_ok());
        assert!(s.landed());
        assert_eq!(s.expedition.landing.as_ref().unwrap().suit, t.suit_max);
        assert!(s.apply(&w, Action::Extract).is_ok());
        assert!(!s.landed());
    }

    #[test]
    fn suit_drains_and_zero_forces_retreat_losing_satchel() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Toxic);
        s.apply(&w, Action::Land(idx)).unwrap();
        // Put something in the satchel to prove it's lost on retreat.
        s.expedition
            .landing
            .as_mut()
            .unwrap()
            .satchel
            .insert(ResourceKind::Data, 9.0);
        let ticks = (t.suit_max / t.suit_drain_per_tick).ceil() as u64 + 1;
        s.advance(&w, ticks);
        assert!(!s.landed(), "suit ran out -> forced retreat");
        assert!(s.expedition.cargo.is_empty(), "satchel lost, not banked");
    }

    #[test]
    fn station_and_jump_actions_locked_while_landed() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
        s.apply(&w, Action::Land(idx)).unwrap();
        assert!(s.apply(&w, Action::Jump(crate::galaxy::HOME)).is_err());
        assert!(
            s.apply(&w, Action::ScanSystem(crate::galaxy::SystemId(2)))
                .is_err()
        );
    }

    #[test]
    fn life_harvest_and_leave_resolve_with_notes() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Verdant);
        s.apply(&w, Action::Land(idx)).unwrap();
        assert!(s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstLanding
        )));
        // Drive the landing onto a Life node, then harvest.
        // Seed 1 + SystemId(1)/site_idx=0 + Verdant is verified to contain a Life node.
        let node = s
            .find_life_node_for_test(&w)
            .expect("verdant surface (seed 1, system 1, site 0) must contain a life node");
        s.walk_to_for_test(&w, node); // helper: set node + resolve
        assert!(s.expedition.landing.as_ref().unwrap().pending_choice);
        let before = s.expedition.landing.as_ref().unwrap().satchel_total();
        s.apply(&w, Action::HarvestLife).unwrap();
        assert!(s.expedition.landing.as_ref().unwrap().satchel_total() > before);
        assert!(!s.expedition.landing.as_ref().unwrap().pending_choice);
        assert!(s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstLifeTaken
        )));
    }

    #[test]
    fn move_traverses_then_resolves_cache_into_satchel() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Frozen);
        s.apply(&w, Action::Land(idx)).unwrap();
        // Find a reachable node from entry and move to it.
        let site = s.surface_for_test(&w);
        let target = site.nodes[0].edges[0];
        assert!(s.apply(&w, Action::MoveToNode(target)).is_ok());
        // Non-neighbor rejected.
        assert!(s.apply(&w, Action::MoveToNode(999)).is_err());
        s.advance(&w, t.traverse_ticks as u64);
        assert_eq!(s.expedition.landing.as_ref().unwrap().node, target);
        // Node is now resolved (recorded in overlay).
        let resolved = s.galaxy_state.sites.get(&1).unwrap()[idx]
            .resolved_nodes
            .contains(&target);
        assert!(resolved);
    }
}
