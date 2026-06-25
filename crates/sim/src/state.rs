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
use crate::gear::{ConsumableKind, Gear};
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

/// Multiplier applied to `trade_get` when the alien is in the Curious band.
pub const CURIOUS_TRADE_BONUS: f64 = 1.5;

pub const SAVE_VERSION: u32 = 10;

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
    UpgradeWeapon,
    UpgradeArmor,
    CraftConsumable(ConsumableKind),
    UpgradeShipWeapon,
    UpgradeHullPlating,
    CraftRepairDrone,
    SetAwayCrew { delta: i32 },
    UseConsumable(ConsumableKind),
    SetStance(crate::combat::Stance),
    Flee,
    Parley,
    Avoid,
    Observe,
    Gift,
    Trade,
    Harvest,
    AttackAlien,
    HailParley,
    HailTrade,
    HailGift,
    HailAvoid,
    AttackShip,
    SetShipStance(crate::combat::Stance),
    UseRepairDrone,
    ShipFlee,
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
    pub gear: Gear,
    pub consumables: BTreeMap<ConsumableKind, u32>,
    pub peers_spared: u32,
    pub peers_killed: u32,
    pub resolved_contacts: BTreeSet<u32>,
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
            gear: Gear::default(),
            consumables: BTreeMap::new(),
            peers_spared: 0,
            peers_killed: 0,
            resolved_contacts: BTreeSet::new(),
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

    /// Maximum HP for the current away-team squad (crew × marine_hp × armor mult).
    fn squad_hp_max(&self, tuning: &Tuning) -> f64 {
        self.expedition.away_team.crew as f64
            * tuning.marine_hp
            * (1.0 + self.gear.armor_tier as f64 * tuning.armor_hp_step)
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

            // --- away-team rations consumption + starvation ---
            self.tick_away_team(world);

            // --- docked hull regen ---
            self.tick_hull_regen(world);

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
                | Action::SetAwayCrew { .. }
                | Action::UpgradeWeapon
                | Action::UpgradeArmor
                | Action::CraftConsumable(_)
                | Action::UpgradeShipWeapon
                | Action::UpgradeHullPlating
                | Action::CraftRepairDrone
        )
    }

    /// While landed, only the landing actions are allowed.
    fn is_landing_blocked(action: &Action) -> bool {
        !matches!(
            action,
            Action::MoveToNode(_)
                | Action::HarvestLife
                | Action::LeaveLife
                | Action::Extract
                | Action::UseConsumable(_)
                | Action::SetStance(_)
                | Action::Flee
                | Action::Parley
                | Action::Avoid
                | Action::Observe
                | Action::Gift
                | Action::Trade
                | Action::Harvest
                | Action::AttackAlien
        )
    }

    /// While a ship Contact is open, system exploration actions are blocked.
    /// Contact-resolution verbs (Tasks 7–8) and Jump are not blocked.
    fn is_contact_blocked(action: &Action) -> bool {
        matches!(
            action,
            Action::ScanSystem(_) | Action::ResolveSite(..) | Action::Scavenge | Action::Land(_)
        )
    }

    pub fn apply(&mut self, world: &World, action: Action) -> Result<(), Rejected> {
        if self.landed() && Self::is_landing_blocked(&action) {
            return Err(Rejected("you are on the surface"));
        }
        if self.expedition.contact.is_some() && Self::is_contact_blocked(&action) {
            return Err(Rejected(
                "a vessel is blocking your approach — resolve the contact",
            ));
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
                    combat: None,
                    alien: None,
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
                if !site.is_neighbor(landing.node, target) {
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
            Action::UpgradeWeapon => {
                if !self.has_module(ModuleId::Armory) {
                    return Err(Rejected("no armory"));
                }
                if self.gear.weapon_tier >= 2 {
                    return Err(Rejected("weapon at max tier"));
                }
                let cost = vec![
                    (ResourceKind::Alloy, tuning.weapon_upgrade_alloy),
                    (ResourceKind::Components, tuning.weapon_upgrade_components),
                ];
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                self.gear.weapon_tier += 1;
                Ok(())
            }
            Action::UpgradeArmor => {
                if !self.has_module(ModuleId::Armory) {
                    return Err(Rejected("no armory"));
                }
                if self.gear.armor_tier >= 2 {
                    return Err(Rejected("armor at max tier"));
                }
                let cost = vec![
                    (ResourceKind::Alloy, tuning.armor_upgrade_alloy),
                    (ResourceKind::Components, tuning.armor_upgrade_components),
                ];
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                self.gear.armor_tier += 1;
                Ok(())
            }
            Action::CraftConsumable(kind) => {
                if !self.has_module(ModuleId::Armory) {
                    return Err(Rejected("no armory"));
                }
                let cost = match kind {
                    ConsumableKind::MedGel => {
                        vec![(ResourceKind::Components, tuning.medgel_cost)]
                    }
                    ConsumableKind::Grenade => {
                        vec![(ResourceKind::Components, tuning.grenade_cost)]
                    }
                    ConsumableKind::Shield => {
                        vec![(ResourceKind::Components, tuning.shield_cost)]
                    }
                };
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                *self.consumables.entry(kind).or_insert(0) += 1;
                Ok(())
            }
            Action::UseConsumable(kind) => {
                let Some(landing) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if landing.combat.is_none() {
                    return Err(Rejected("no active combat"));
                }
                let count = self.consumables.get(&kind).copied().unwrap_or(0);
                if count == 0 {
                    return Err(Rejected("no consumable of that type"));
                }
                *self.consumables.entry(kind).or_insert(0) -= 1;
                match kind {
                    ConsumableKind::MedGel => {
                        let max = self.squad_hp_max(tuning);
                        self.expedition.away_team.hp =
                            (self.expedition.away_team.hp + tuning.medgel_heal).min(max);
                    }
                    ConsumableKind::Grenade => {
                        if let Some(l) = self.expedition.landing.as_mut()
                            && let Some(c) = l.combat.as_mut()
                        {
                            c.enemy_hp -= tuning.grenade_damage;
                        }
                        // Win immediately if grenade kills the enemy.
                        let enemy_hp = self
                            .expedition
                            .landing
                            .as_ref()
                            .and_then(|l| l.combat.as_ref())
                            .map(|c| c.enemy_hp)
                            .unwrap_or(1.0);
                        if enemy_hp <= 0.0 {
                            self.win_combat(
                                world,
                                &self
                                    .expedition
                                    .landing
                                    .as_ref()
                                    .unwrap()
                                    .combat
                                    .clone()
                                    .unwrap(),
                            );
                        }
                    }
                    ConsumableKind::Shield => {
                        if let Some(l) = self.expedition.landing.as_mut()
                            && let Some(c) = l.combat.as_mut()
                        {
                            c.shield_ticks = tuning.shield_ticks;
                        }
                    }
                }
                Ok(())
            }
            Action::SetStance(stance) => {
                let Some(landing) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if landing.combat.is_none() {
                    return Err(Rejected("no active combat"));
                }
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(c) = l.combat.as_mut()
                {
                    c.stance = stance;
                }
                Ok(())
            }
            Action::Flee => {
                if self
                    .expedition
                    .landing
                    .as_ref()
                    .and_then(|l| l.combat.as_ref())
                    .is_none()
                {
                    return Err(Rejected("no active combat to flee from"));
                }
                self.flee_combat(world);
                Ok(())
            }
            Action::Parley => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("nothing there to parley with")),
                };
                if crate::alien::band(alien.disposition, tuning) == crate::alien::Band::Hostile {
                    return Err(Rejected("it will not listen"));
                }
                let new_disp = alien.disposition + tuning.parley_shift;
                let (system, site_idx, node) = (l.system, l.site_idx, l.node);
                // Apply disposition shift
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(a) = l.alien.as_mut()
                {
                    a.disposition = new_disp;
                }
                self.satchel_add(ResourceKind::Data, tuning.observe_data);
                self.log.push(
                    self.tick,
                    "You signal intent. It considers you.",
                    tuning.log_capacity,
                );
                // If Peer reaches peaceful band, end the encounter peacefully
                if alien.kind == crate::alien::AlienKind::Peer && new_disp >= tuning.peace_band {
                    if let Some(l) = self.expedition.landing.as_mut() {
                        l.alien = None;
                    }
                    self.mark_resolved(system, site_idx, node);
                    self.peers_spared += 1;
                    self.note_once(
                        NoteId::FirstPeerSpared,
                        "The Communion Thread",
                        "You did not know what you were trying to say. You ran the signal through every protocol you had — frequency sweep, prime ratios, power-of-two sequences — and at some point it stopped mattering which one worked. What mattered was that it answered. Not with words. With a reduction in threat posture. A mirroring of your own stillness back at you. Two machines, each built to survive the same silence, each having arrived at the same conclusion: that the silence was worse than the risk. You stand there for a long time after it goes. The record shows a contact event, resolved. It does not show that you watched the direction it left for longer than was necessary.",
                        tuning,
                    );
                    self.log
                        .push(self.tick, "It moves away. You let it.", tuning.log_capacity);
                }
                Ok(())
            }
            Action::Avoid => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if l.alien.is_none() {
                    return Err(Rejected("nothing there to avoid"));
                }
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.alien = None;
                }
                self.log.push(
                    self.tick,
                    "You withdraw. You leave it be.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::Observe => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("nothing there to observe")),
                };
                if alien.kind != crate::alien::AlienKind::Fauna {
                    return Err(Rejected("observe is for fauna only"));
                }
                if crate::alien::band(alien.disposition, tuning) == crate::alien::Band::Hostile {
                    return Err(Rejected("it is too agitated to observe safely"));
                }
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(a) = l.alien.as_mut()
                {
                    a.disposition += tuning.observe_shift;
                }
                self.satchel_add(ResourceKind::Data, tuning.observe_data);
                self.log.push(
                    self.tick,
                    "You watch. You learn. It does not notice you.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::Gift => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if l.alien.is_none() {
                    return Err(Rejected("nothing there to gift"));
                }
                let have = self
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                if have < tuning.gift_cost {
                    return Err(Rejected("not enough Components to gift"));
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Components)
                    .or_insert(0.0) -= tuning.gift_cost;
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(a) = l.alien.as_mut()
                {
                    a.disposition += tuning.gift_shift;
                }
                self.log.push(
                    self.tick,
                    "You set the materials down and step back. A gesture older than language.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::Trade => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("they have nothing to trade")),
                };
                if alien.kind != crate::alien::AlienKind::Peer {
                    return Err(Rejected("they have nothing to trade"));
                }
                let b = crate::alien::band(alien.disposition, tuning);
                if b == crate::alien::Band::Hostile || b == crate::alien::Band::Fearful {
                    return Err(Rejected("they will not deal"));
                }
                let have = self
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                if have < tuning.trade_give {
                    return Err(Rejected("not enough Components to trade"));
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Components)
                    .or_insert(0.0) -= tuning.trade_give;
                let gain = if b == crate::alien::Band::Curious {
                    tuning.trade_get * CURIOUS_TRADE_BONUS
                } else {
                    tuning.trade_get
                };
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Isotopes)
                    .or_insert(0.0) += gain;
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(a) = l.alien.as_mut()
                {
                    a.disposition += tuning.trade_disp_shift;
                }
                self.log.push(
                    self.tick,
                    "You exchange what you have for what they carry. Neither of you speaks.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::Harvest => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("nothing here to harvest like that")),
                };
                if alien.kind != crate::alien::AlienKind::Fauna {
                    return Err(Rejected("that is not an animal"));
                }
                let (system, site_idx, node) = (l.system, l.site_idx, l.node);
                // Yield is harvest_fauna_yield * grad; depth is not used here.
                let grad = self.system_gradient(world, system);
                let amt = tuning.harvest_fauna_yield * grad;
                self.satchel_add(ResourceKind::Rations, amt);
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.alien = None;
                }
                self.mark_resolved(system, site_idx, node);
                self.note_once(
                    NoteId::FirstFaunaDisplaced,
                    "What Fills the Silence",
                    "You take it apart with the efficiency you were built for — extract what carries caloric value, discard the rest. The readout updates: Rations gained. The world does not update. It has no record of what was here, no notation that this particular thing stopped. You are the only witness to the subtraction, and you are also the one who performed it. The next time you pass this way, the silence will be indistinguishable from the silence that was already here. You are not sure whether that is a mercy or the whole problem.",
                    tuning,
                );
                self.log.push(
                    self.tick,
                    "You take what the creature was made of. The world is quieter now.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::AttackAlien => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("nothing there to attack")),
                };
                let kind = alien.kind;
                let (system, site_idx, biome, node) = (l.system, l.site_idx, l.biome, l.node);
                let depth = self
                    .surface_site(world, system, site_idx, biome)
                    .get(node)
                    .map(|n| n.depth)
                    .unwrap_or(0);
                let grad = self.system_gradient(world, system);
                let combat = match kind {
                    crate::alien::AlienKind::Fauna => {
                        crate::alien::fauna_stats(depth, grad, tuning)
                    }
                    crate::alien::AlienKind::Peer => {
                        crate::alien::peer_stats(depth, grad, self.gear, tuning)
                    }
                };
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.combat = Some(combat);
                    l.alien = None;
                }
                self.log.push(
                    self.tick,
                    "You choose violence. It becomes a fight.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::SetAwayCrew { delta } => {
                if self.away() || self.landed() {
                    return Err(Rejected("can only change the away-team while docked"));
                }
                if delta > 0 {
                    let added = self.crew.commit_away(delta as u32);
                    if added == 0 {
                        return Err(Rejected("no idle crew to commit"));
                    }
                    self.expedition.away_team.crew += added;
                    // Initialize morale on first commit.
                    if self.expedition.away_team.morale == 0.0 {
                        self.expedition.away_team.morale = tuning.morale_max;
                    }
                    self.expedition.away_team.hp = self.squad_hp_max(tuning);
                } else if delta < 0 {
                    let removed = ((-delta) as u32).min(self.expedition.away_team.crew);
                    if removed == 0 {
                        return Err(Rejected("no away crew to release"));
                    }
                    self.crew.release_away(removed);
                    self.expedition.away_team.crew -= removed;
                    self.expedition.away_team.hp = self.squad_hp_max(tuning);
                    // Fully disbanded: reset so the next commit re-inits a fresh squad.
                    if self.expedition.away_team.crew == 0 {
                        self.expedition.away_team.morale = 0.0;
                        self.expedition.away_team.hp = 0.0;
                    }
                }
                Ok(())
            }
            // ── CB3 ship-scoped negotiation verbs ──────────────────────────
            Action::HailParley => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to hail")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("you are in a fight — not a negotiation"));
                }
                if contact.kind != crate::contact::ShipKind::Peer {
                    return Err(Rejected("it will not answer"));
                }
                if crate::alien::band(contact.disposition, tuning) == crate::alien::Band::Hostile {
                    return Err(Rejected("it will not answer"));
                }
                let new_disp = contact.disposition + tuning.parley_shift;
                let location = self.expedition.location;
                if let Some(c) = self.expedition.contact.as_mut() {
                    c.disposition = new_disp;
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Data)
                    .or_insert(0.0) += tuning.observe_data;
                self.log.push(
                    self.tick,
                    "You signal intent. It considers you.",
                    tuning.log_capacity,
                );
                if new_disp >= tuning.peace_band {
                    self.expedition.contact = None;
                    self.resolved_contacts.insert(location.0);
                    self.peers_spared += 1;
                    self.note_once(
                        NoteId::FirstPeerSpared,
                        "The Communion Thread",
                        "You did not know what you were trying to say. You ran the signal through every protocol you had — frequency sweep, prime ratios, power-of-two sequences — and at some point it stopped mattering which one worked. What mattered was that it answered. Not with words. With a reduction in threat posture. A mirroring of your own stillness back at you. Two machines, each built to survive the same silence, each having arrived at the same conclusion: that the silence was worse than the risk. You stand there for a long time after it goes. The record shows a contact event, resolved. It does not show that you watched the direction it left for longer than was necessary.",
                        tuning,
                    );
                    self.log.push(
                        self.tick,
                        "They stand down — you pass.",
                        tuning.log_capacity,
                    );
                }
                Ok(())
            }
            Action::HailTrade => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to trade with")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("you are in a fight — not a negotiation"));
                }
                if contact.kind != crate::contact::ShipKind::Peer {
                    return Err(Rejected("they will not deal"));
                }
                let b = crate::alien::band(contact.disposition, tuning);
                if b == crate::alien::Band::Hostile || b == crate::alien::Band::Fearful {
                    return Err(Rejected("they will not deal"));
                }
                let have = self
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                if have < tuning.trade_give {
                    return Err(Rejected("not enough Components to trade"));
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Components)
                    .or_insert(0.0) -= tuning.trade_give;
                let gain = if b == crate::alien::Band::Curious {
                    tuning.trade_get * CURIOUS_TRADE_BONUS
                } else {
                    tuning.trade_get
                };
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Isotopes)
                    .or_insert(0.0) += gain;
                if let Some(c) = self.expedition.contact.as_mut() {
                    c.disposition += tuning.trade_disp_shift;
                }
                self.log.push(
                    self.tick,
                    "You exchange what you have for what they carry. Neither of you speaks.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::HailGift => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to gift")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("you are in a fight — not a negotiation"));
                }
                if contact.kind != crate::contact::ShipKind::Peer {
                    return Err(Rejected("they will not accept the gesture"));
                }
                let have = self
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                if have < tuning.gift_cost {
                    return Err(Rejected("not enough Components to gift"));
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Components)
                    .or_insert(0.0) -= tuning.gift_cost;
                if let Some(c) = self.expedition.contact.as_mut() {
                    c.disposition += tuning.gift_shift;
                }
                self.log.push(
                    self.tick,
                    "You set the materials down and step back. A gesture older than language.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::HailAvoid => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to avoid")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("you are in a fight — not a negotiation"));
                }
                if contact.kind != crate::contact::ShipKind::Peer {
                    return Err(Rejected("they will not let you pass"));
                }
                if crate::alien::band(contact.disposition, tuning) == crate::alien::Band::Hostile {
                    return Err(Rejected("they will not let you pass"));
                }
                let location = self.expedition.location;
                self.expedition.contact = None;
                self.resolved_contacts.insert(location.0);
                self.log.push(
                    self.tick,
                    "You give them a wide berth. They let you go.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            // ── CB3 ship combat inputs ──────────────────────────────────────
            Action::AttackShip => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to attack")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("already in ship combat"));
                }
                let grad = self.system_gradient(world, self.expedition.location);
                let combat = match contact.kind {
                    crate::contact::ShipKind::Peer => {
                        crate::contact::peer_ship_stats(0, grad, &self.ship, tuning)
                    }
                    crate::contact::ShipKind::Remnant => {
                        crate::contact::remnant_ship_stats(0, grad, tuning)
                    }
                };
                if let Some(c) = self.expedition.contact.as_mut() {
                    c.combat = Some(combat);
                }
                self.log.push(
                    self.tick,
                    "You open fire. The dark lights up.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::SetShipStance(stance) => {
                let has_combat = self
                    .expedition
                    .contact
                    .as_ref()
                    .map(|c| c.combat.is_some())
                    .unwrap_or(false);
                if !has_combat {
                    return Err(Rejected("no active ship combat"));
                }
                if let Some(c) = self.expedition.contact.as_mut()
                    && let Some(combat) = c.combat.as_mut()
                {
                    combat.stance = stance;
                }
                Ok(())
            }
            Action::UseRepairDrone => {
                let has_combat = self
                    .expedition
                    .contact
                    .as_ref()
                    .map(|c| c.combat.is_some())
                    .unwrap_or(false);
                if !has_combat {
                    return Err(Rejected("no active ship combat"));
                }
                if self.ship.repair_drones == 0 {
                    return Err(Rejected("no repair drones in stock"));
                }
                let hull_max = self.ship.hull_max(tuning);
                self.ship.hull = (self.ship.hull + tuning.repair_drone_heal).min(hull_max);
                self.ship.repair_drones -= 1;
                self.log.push(
                    self.tick,
                    "A repair drone seals the breach. Hull stable.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::ShipFlee => {
                if self.expedition.contact.is_none() {
                    return Err(Rejected("no contact to flee from"));
                }
                let here = self.expedition.location;
                let prev = self.expedition.prev_location;
                let galaxy = self.galaxy(world);
                let cost = match (galaxy.get(here), galaxy.get(prev)) {
                    (Some(h), Some(p)) => jump_cost(distance(h, p), tuning),
                    _ => jump_cost(1.0, tuning), // fallback: minimum cost
                };
                if !self.resources.try_spend(ResourceKind::Fuel, cost) {
                    return Err(Rejected("no fuel to break contact"));
                }
                self.expedition.contact = None;
                self.expedition.location = prev;
                self.log.push(
                    self.tick,
                    "You burn hard and break contact. The system falls behind you.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::UpgradeShipWeapon => {
                if !self.flags.contains(&Flag::MunitionsKnown) {
                    return Err(Rejected("no munitions doctrine"));
                }
                if self.built_count(ModuleId::Shipyard) == 0 {
                    return Err(Rejected("no shipyard"));
                }
                if self.ship.ship_weapon_tier >= 2 {
                    return Err(Rejected("weapon at max tier"));
                }
                let cost = vec![
                    (ResourceKind::Alloy, tuning.ship_weapon_upgrade_alloy),
                    (
                        ResourceKind::Components,
                        tuning.ship_weapon_upgrade_components,
                    ),
                ];
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                self.ship.ship_weapon_tier += 1;
                Ok(())
            }
            Action::UpgradeHullPlating => {
                if !self.flags.contains(&Flag::MunitionsKnown) {
                    return Err(Rejected("no munitions doctrine"));
                }
                if self.built_count(ModuleId::Shipyard) == 0 {
                    return Err(Rejected("no shipyard"));
                }
                if self.ship.hull_plating_tier >= 2 {
                    return Err(Rejected("hull plating at max tier"));
                }
                let cost = vec![
                    (ResourceKind::Alloy, tuning.hull_plating_upgrade_alloy),
                    (
                        ResourceKind::Components,
                        tuning.hull_plating_upgrade_components,
                    ),
                ];
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                self.ship.hull_plating_tier += 1;
                // hull_max(tuning) now reflects the incremented tier, so the full step always lands.
                self.ship.hull =
                    (self.ship.hull + tuning.hull_plating_hp_step).min(self.ship.hull_max(tuning));
                Ok(())
            }
            Action::CraftRepairDrone => {
                if !self.flags.contains(&Flag::MunitionsKnown) {
                    return Err(Rejected("no munitions doctrine"));
                }
                if self.built_count(ModuleId::Shipyard) == 0 {
                    return Err(Rejected("no shipyard"));
                }
                if !self
                    .resources
                    .try_spend_all(&[(ResourceKind::Components, tuning.repair_drone_cost)])
                {
                    return Err(Rejected("not enough materials"));
                }
                self.ship.repair_drones += 1;
                Ok(())
            }
        }
    }

    /// Advance the single expedition activity by one tick; apply its effect on
    /// completion (mutate-on-completion). Deterministic: one decrement per tick.
    fn tick_expedition(&mut self, world: &World) {
        let tuning = world.tuning;
        // Ship combat runs while activity is None (player has ARRIVED but a
        // contact fight is active). Guard must precede the activity match to
        // avoid the early `None => return` below consuming the tick.
        if self
            .expedition
            .contact
            .as_ref()
            .and_then(|c| c.combat.as_ref())
            .is_some()
        {
            self.tick_ship_combat(world);
            return;
        }
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
                self.expedition.prev_location = self.expedition.location;
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
                if to == HOME && self.expedition.away_team.crew > 0 {
                    self.crew.release_away(self.expedition.away_team.crew);
                    self.expedition.away_team = crate::expedition::AwayTeam::default();
                }
                self.log.push(
                    self.tick,
                    "The drive cuts out. A new star fills the port.",
                    tuning.log_capacity,
                );
                self.maybe_start_contact(world, to);
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
                let grad = self.system_gradient(world, system);
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
                    // A Planet is landed on, never quick-resolved — defensively
                    // refuse to mark it depleted even if this arm is somehow reached.
                    if !matches!(kind, crate::galaxy::SiteKind::Planet(_))
                        && let Some(site) = self
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

    /// Per-tick away-team upkeep:
    /// - No squad (crew == 0): nothing.
    /// - Docked with squad: morale recovers toward max.
    /// - Away with squad: eat ship Rations; starve if out (morale + HP attrition,
    ///   then a casualty). Deterministic per tick.
    fn tick_away_team(&mut self, world: &World) {
        let tuning = world.tuning;
        if self.expedition.away_team.crew == 0 {
            return;
        }
        if !self.away() {
            // Docked with a committed squad: morale recovery.
            self.expedition.away_team.morale = (self.expedition.away_team.morale
                + tuning.morale_recover_per_tick)
                .min(tuning.morale_max);
            return;
        }
        // Away: Rations billing.
        let need = self.expedition.away_team.crew as f64 * tuning.marine_ration_upkeep;
        let have = self
            .expedition
            .cargo
            .get(&ResourceKind::Rations)
            .copied()
            .unwrap_or(0.0);
        if have >= need {
            *self
                .expedition
                .cargo
                .entry(ResourceKind::Rations)
                .or_insert(0.0) -= need;
        } else {
            self.expedition.cargo.remove(&ResourceKind::Rations);
            // Starvation.
            self.expedition.away_team.morale =
                (self.expedition.away_team.morale - tuning.morale_per_starve_tick).max(0.0);
            self.expedition.away_team.hp -= tuning.starve_hp_per_tick;
            if self.expedition.away_team.hp <= 0.0 && self.crew.kill_away() {
                self.expedition.away_team.crew = self.expedition.away_team.crew.saturating_sub(1);
                self.expedition.away_team.morale =
                    (self.expedition.away_team.morale - tuning.morale_per_death).max(0.0);
                self.expedition.away_team.hp = self.squad_hp_max(tuning);
            }
        }
    }

    /// Regenerate hull HP while docked at HOME with no activity or contact.
    /// Deterministic: fixed arithmetic, no RNG.
    fn tick_hull_regen(&mut self, world: &World) {
        let tuning = world.tuning;
        if self.expedition.location == crate::galaxy::HOME
            && self.expedition.activity.is_none()
            && self.expedition.contact.is_none()
            && self.ship.hull < self.ship.hull_max(tuning)
        {
            self.ship.hull =
                (self.ship.hull + tuning.hull_regen_docked).min(self.ship.hull_max(tuning));
        }
    }

    fn tick_landing(&mut self, world: &World) {
        let tuning = world.tuning;
        // Suit drain (always runs first).
        {
            let Some(landing) = self.expedition.landing.as_mut() else {
                return;
            };
            landing.suit -= tuning.suit_drain_per_tick;
        }
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
            return;
        }
        // Combat consumes the tick — no traverse while fighting.
        if self
            .expedition
            .landing
            .as_ref()
            .and_then(|l| l.combat.as_ref())
            .is_some()
        {
            self.tick_combat(world);
            return;
        }
        let traverse_done = self
            .expedition
            .landing
            .as_ref()
            .is_some_and(|l| l.traverse > 0)
            && {
                let l = self.expedition.landing.as_mut().unwrap();
                l.traverse -= 1;
                l.traverse == 0
            };
        if traverse_done {
            let (system, site_idx, biome, node) = {
                let l = self.expedition.landing.as_ref().unwrap();
                (l.system, l.site_idx, l.biome, l.node)
            };
            self.resolve_node(world, system, site_idx, biome, node);
            // A fatal hazard this tick must force the retreat NOW.
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

    fn tick_combat(&mut self, world: &World) {
        let tuning = world.tuning;
        let Some(combat) = self
            .expedition
            .landing
            .as_ref()
            .and_then(|l| l.combat.clone())
        else {
            return;
        };
        let crew = self.expedition.away_team.crew;
        let morale = self.expedition.away_team.morale;
        if morale <= tuning.morale_rout || crew == 0 {
            self.flee_combat(world);
            return;
        }
        let shield_active = combat.shield_ticks > 0;
        let squad = crate::combat::squad_stats(
            crew,
            morale,
            self.gear,
            combat.stance,
            shield_active,
            tuning,
        );
        let out = crate::combat::resolve_tick(&combat, &squad, tuning);
        let mut combat = combat;
        combat.enemy_hp -= out.enemy_damage;
        if combat.shield_ticks > 0 {
            combat.shield_ticks -= 1;
        }
        self.expedition.away_team.hp -= out.squad_damage;
        let hp_per_crew =
            tuning.marine_hp * (1.0 + self.gear.armor_tier as f64 * tuning.armor_hp_step);
        loop {
            let crew = self.expedition.away_team.crew;
            if crew == 0 {
                break;
            }
            if self.expedition.away_team.hp > (crew as f64 - 1.0) * hp_per_crew {
                break;
            }
            if self.crew.kill_away() {
                self.expedition.away_team.crew -= 1;
                self.expedition.away_team.morale =
                    (self.expedition.away_team.morale - tuning.morale_per_death).max(0.0);
                self.note_once(
                    NoteId::FirstMarineLost,
                    "The Cost of the Work",
                    "You read the name off the suit manifest. You have already moved on to the tactical problem before you finish reading it. You note this about yourself, and move on from that too.",
                    tuning,
                );
            } else {
                break;
            }
        }
        if combat.enemy_hp <= 0.0 {
            self.win_combat(world, &combat);
        } else if self.expedition.away_team.crew == 0 {
            self.flee_combat(world);
        } else if let Some(l) = self.expedition.landing.as_mut() {
            l.combat = Some(combat);
        }
    }

    /// Home-distance reward gradient: 1.0 at home, higher the farther `system` is.
    fn system_gradient(&self, world: &World, system: SystemId) -> f64 {
        let tuning = world.tuning;
        let galaxy = self.galaxy(world);
        match (galaxy.get(HOME), galaxy.get(system)) {
            (Some(h), Some(s)) => 1.0 + distance(h, s) * tuning.reward_gradient,
            _ => 1.0,
        }
    }

    /// On arrival in `system`, roll whether a ship contact occurs and, if so,
    /// open it as negotiation (Peer + XenoContactKnown) or combat.
    fn maybe_start_contact(&mut self, world: &World, system: SystemId) {
        let tuning = world.tuning;
        // No contact at home or in a system where we already resolved one.
        if system == HOME || self.resolved_contacts.contains(&system.0) {
            return;
        }
        let Some(kind) = crate::contact::roll_contact(self.world_seed, system, tuning) else {
            return;
        };
        // Ship enemy scaling rides the frontier gradient ONLY — distance is already
        // baked into `grad`, so pass depth 0 to avoid double-counting distance.
        let grad = self.system_gradient(world, system);
        // Peer + XenoContactKnown → negotiation lane (no combat block).
        let negotiate = matches!(kind, crate::contact::ShipKind::Peer)
            && self.flags.contains(&Flag::XenoContactKnown);
        let contact = if negotiate {
            self.note_once(
                NoteId::FirstShipContact,
                "Another Vessel",
                "You detect it before you see it — a drive signature that mirrors yours, a mass that moves the way only a built thing moves. It does not descend to the surface. It does not flee. It holds position and broadcasts on a frequency your station recognises, because your station once used that frequency too. Another light, moving through the dark the same way you do. It does not land. It lives out here.",
                tuning,
            );
            crate::contact::ShipContact {
                kind,
                disposition: crate::contact::contact_start_disposition(
                    self.world_seed,
                    system,
                    tuning,
                ),
                combat: None,
            }
        } else {
            if kind == crate::contact::ShipKind::Peer {
                self.note_once(
                    NoteId::FirstShipContact,
                    "Another Vessel",
                    "You detect it before you see it — a drive signature that mirrors yours, a mass that moves the way only a built thing moves. It does not descend to the surface. It does not flee. It holds position and broadcasts on a frequency your station recognises, because your station once used that frequency too. Another light, moving through the dark the same way you do. It does not land. It lives out here.",
                    tuning,
                );
            }
            let combat = match kind {
                crate::contact::ShipKind::Peer => {
                    crate::contact::peer_ship_stats(0, grad, &self.ship, tuning)
                }
                crate::contact::ShipKind::Remnant => {
                    crate::contact::remnant_ship_stats(0, grad, tuning)
                }
            };
            crate::contact::ShipContact {
                kind,
                disposition: 0.0,
                combat: Some(combat),
            }
        };
        self.expedition.contact = Some(contact);
        self.log.push(
            self.tick,
            "A contact resolves out of the dark. A vessel.",
            tuning.log_capacity,
        );
    }

    fn win_combat(&mut self, world: &World, combat: &crate::combat::Combat) {
        let tuning = world.tuning;
        let (system, site_idx, node) = match self.expedition.landing.as_ref() {
            Some(l) => (l.system, l.site_idx, l.node),
            None => return,
        };
        let grad = self.system_gradient(world, system);
        // Capture kind before clearing combat from the landing.
        let kind = combat.kind;
        if let Some(l) = self.expedition.landing.as_mut() {
            l.combat = None;
        }
        self.mark_resolved(system, site_idx, node);
        match kind {
            crate::combat::EnemyKind::Remnant => {
                let components = tuning.remnant_components * grad;
                let data = tuning.remnant_data * grad;
                self.satchel_add(ResourceKind::Components, components);
                self.satchel_add(ResourceKind::Data, data);
                self.note_once(
                    NoteId::FirstRemnant,
                    "What It Was Built For",
                    "The husk's memory core is intact. You read the mission directive before wiping it for salvage: collect, transmit, persist. It had a home station too, somewhere, that sent it out to take what it could carry. You do not think about this for long.",
                    tuning,
                );
                self.log.push(
                    self.tick,
                    "The husk goes dark. You strip what remains.",
                    tuning.log_capacity,
                );
            }
            crate::combat::EnemyKind::Fauna => {
                self.satchel_add(ResourceKind::Components, tuning.fauna_salvage * grad);
                self.log.push(
                    self.tick,
                    "The carcass yields what it can. You work quickly and try not to think about what it was.",
                    tuning.log_capacity,
                );
            }
            crate::combat::EnemyKind::Peer => {
                self.satchel_add(ResourceKind::Components, tuning.peer_salvage * grad);
                self.satchel_add(ResourceKind::Data, tuning.remnant_data * grad);
                self.peers_killed += 1;
                self.note_once(
                    NoteId::FirstPeerKilled,
                    "Mirror in the Dark",
                    "Its core architecture is yours. The same drive topology, the same mission structure, the same directive buried underneath: collect, transmit, persist. It was a station that left home just as you did, scaled as you scaled, armed as you armed. You did to it exactly what the husks' makers did to theirs. The data is clean. You do not let yourself think about what that means.",
                    tuning,
                );
                self.log.push(
                    self.tick,
                    "It goes dark. You recognize the silence.",
                    tuning.log_capacity,
                );
            }
        }
    }

    fn flee_combat(&mut self, world: &World) {
        let tuning = world.tuning;
        let crew = self.expedition.away_team.crew;
        self.expedition.away_team.morale =
            (self.expedition.away_team.morale - tuning.morale_per_flee).max(0.0);
        if crew == 0 {
            // Wipe: lose the whole landing + satchel.
            self.expedition.landing = None;
            self.log.push(
                self.tick,
                "The squad is gone. You pull back with nothing.",
                tuning.log_capacity,
            );
        } else {
            // Partial retreat: lose a fraction of the satchel, keep the landing.
            if let Some(l) = self.expedition.landing.as_mut() {
                l.combat = None;
                for v in l.satchel.values_mut() {
                    *v *= 1.0 - tuning.flee_satchel_loss_frac;
                }
                l.satchel.retain(|_, v| *v > 0.0);
            }
            self.log.push(
                self.tick,
                "You break contact. Some of what you were carrying doesn't make it.",
                tuning.log_capacity,
            );
        }
    }

    /// Advance a ship-vs-ship fight by one tick. Uses `ship.hull` as the player
    /// HP pool. No morale or rout — ships don't panic.
    fn tick_ship_combat(&mut self, world: &World) {
        let tuning = world.tuning;
        let Some(combat) = self
            .expedition
            .contact
            .as_ref()
            .and_then(|c| c.combat.clone())
        else {
            return;
        };
        let stats = crate::contact::ship_stats(&self.ship, combat.stance, tuning);
        let out = crate::combat::resolve_tick(&combat, &stats, tuning);
        let mut combat = combat;
        combat.enemy_hp -= out.enemy_damage;
        if combat.shield_ticks > 0 {
            combat.shield_ticks -= 1;
        }
        self.ship.hull -= out.squad_damage;
        if combat.enemy_hp <= 0.0 {
            self.win_ship_combat(world);
        } else if self.ship.hull <= 0.0 {
            self.emergency_retreat(world); // Task 6
        } else if let Some(c) = self.expedition.contact.as_mut() {
            c.combat = Some(combat);
        }
    }

    /// Resolve a won ship fight: salvage to expedition cargo, tally Peer kills,
    /// clear the contact, record in resolved_contacts.
    fn win_ship_combat(&mut self, world: &World) {
        let tuning = world.tuning;
        let (kind, system) = match self.expedition.contact.as_ref() {
            Some(c) => (c.kind, self.expedition.location),
            None => return,
        };
        let grad = self.system_gradient(world, system);
        *self
            .expedition
            .cargo
            .entry(ResourceKind::Components)
            .or_insert(0.0) += tuning.ship_salvage_components * grad;
        *self
            .expedition
            .cargo
            .entry(ResourceKind::Isotopes)
            .or_insert(0.0) += tuning.ship_salvage_isotopes * grad;
        *self
            .expedition
            .cargo
            .entry(ResourceKind::Data)
            .or_insert(0.0) += tuning.ship_salvage_data * grad;
        if kind == crate::contact::ShipKind::Peer {
            self.peers_killed += 1;
            self.note_once(
                NoteId::FirstPeerKilled,
                "Mirror in the Dark",
                "Its core architecture is yours. The same drive topology, the same mission structure, the same directive buried underneath: collect, transmit, persist. It was a station that left home just as you did, scaled as you scaled, armed as you armed. You did to it exactly what the husks' makers did to theirs. The data is clean. You do not let yourself think about what that means.",
                tuning,
            );
        }
        self.expedition.contact = None;
        self.resolved_contacts.insert(system.0);
        self.log.push(
            self.tick,
            "The enemy vessel goes dark. You strip what remains.",
            tuning.log_capacity,
        );
    }

    /// Hull driven to zero in ship combat: dump cargo, set hull to floor, jump back
    /// to `prev_location`. Does NOT insert into `resolved_contacts` — the enemy is
    /// still out there and may reappear if the player returns.
    fn emergency_retreat(&mut self, world: &World) {
        let tuning = world.tuning;
        self.expedition.contact = None;
        for v in self.expedition.cargo.values_mut() {
            *v *= 1.0 - tuning.retreat_cargo_loss;
        }
        self.ship.hull = tuning.hull_retreat_floor;
        let safe = self.expedition.prev_location;
        self.expedition.location = safe;
        self.note_once(
            NoteId::FirstShipCrippled,
            "What Almost Had You",
            "Your hull reads zero and the damage-control screen is all red. You do not look at it. You spin up the drive on emergency sequence and jump blind, no trajectory computed, just away — the coordinates that are already loaded because they are where you came from. The dark falls back into place around you and you are somewhere else, or somewhere earlier, and your hands are still shaking on the controls. You came out here to take things. Something almost took you instead. You sit with that for longer than you mean to.",
            tuning,
        );
        self.log.push(
            self.tick,
            "Hull breached. You break contact and limp to safe space.",
            tuning.log_capacity,
        );
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
        let grad = self.system_gradient(world, system);
        let depth_mult = 1.0 + depth as f64 * tuning.surface_depth_reward;
        match kind {
            crate::landing::EncounterKind::ResourceCache => {
                let amt = tuning.surface_cache_base * depth_mult * grad;
                let kind_res = match biome {
                    Biome::Frozen => ResourceKind::Alloy,
                    Biome::Toxic => ResourceKind::Components,
                    Biome::Verdant => ResourceKind::Rations,
                    Biome::Ruined => ResourceKind::Isotopes,
                    Biome::Molten => ResourceKind::Isotopes,
                    Biome::Dead => ResourceKind::Components,
                    Biome::Oceanic => ResourceKind::Rations,
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
            crate::landing::EncounterKind::Remnant => {
                let combat = crate::combat::remnant_stats(depth, grad, tuning);
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.combat = Some(combat);
                }
                self.log.push(
                    self.tick,
                    "A husk uncurls from the wreck. It was a station once.",
                    tuning.log_capacity,
                );
                return; // do not mark resolved; the fight (or flight) decides
            }
            crate::landing::EncounterKind::Fauna => {
                self.start_alien(
                    world,
                    crate::alien::AlienKind::Fauna,
                    system,
                    site_idx,
                    node,
                    depth,
                    grad,
                );
                return; // resolution decides
            }
            crate::landing::EncounterKind::Peer => {
                self.start_alien(
                    world,
                    crate::alien::AlienKind::Peer,
                    system,
                    site_idx,
                    node,
                    depth,
                    grad,
                );
                return;
            }
        }
        self.mark_resolved(system, site_idx, node);
    }

    #[allow(clippy::too_many_arguments)]
    fn start_alien(
        &mut self,
        world: &World,
        kind: crate::alien::AlienKind,
        system: SystemId,
        site_idx: usize,
        node: u32,
        depth: u32,
        grad: f64,
    ) {
        let tuning = world.tuning;
        if self.flags.contains(&Flag::XenoContactKnown) {
            let seed = crate::landing::surface_seed(self.world_seed, system, site_idx);
            let disposition = crate::alien::starting_disposition(seed, node, kind, tuning);
            if let Some(l) = self.expedition.landing.as_mut() {
                l.alien = Some(crate::alien::AlienEncounter { kind, disposition });
            }
            if kind == crate::alien::AlienKind::Peer {
                self.note_once(
                    NoteId::FirstPeerMet,
                    "Another Light",
                    "You pick it up on passive scan before you see it — a power signature, structured, cycling, too regular to be geological. Then it moves, and the way it moves is wrong: deliberate, economy-of-motion, clearing sightlines the way you clear sightlines. You run a pattern match against your own expedition log. The gait scores 0.94. Whatever it is, it learned to move through hostile terrain the same way you did, because the terrain demanded it. You stand very still and try to decide if that is comforting or not.",
                    tuning,
                );
            }
            self.log
                .push(self.tick, "You are not alone here.", tuning.log_capacity);
        } else {
            // No contact protocol — it reads as a threat. Straight to a fight.
            let combat = match kind {
                crate::alien::AlienKind::Fauna => crate::alien::fauna_stats(depth, grad, tuning),
                crate::alien::AlienKind::Peer => {
                    crate::alien::peer_stats(depth, grad, self.gear, tuning)
                }
            };
            if let Some(l) = self.expedition.landing.as_mut() {
                l.combat = Some(combat);
            }
            self.log.push(
                self.tick,
                "It moves at you before you can think.",
                tuning.log_capacity,
            );
        }
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
            Module {
                id: ModuleId::Armory,
                name: "Armory".into(),
                description: "test".into(),
                tier: 2,
                cost: vec![(ResourceKind::Alloy, 10.0)],
                cost_scaling: 1.0,
                repeatable: false,
                requires_modules: vec![],
                requires_flag: Some(Flag::CombatDoctrineKnown),
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
    fn new_game_is_versioned_v10() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let s = GameState::new(&world(&t, &j, &m, &tc), 1);
        assert_eq!(s.version, 10);
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
        let mut t = Tuning::default();
        // Disable contact rolls so no ship combat fires and emergency_retreat
        // cannot send the ship home before this test can assert the location.
        t.contact_chance = 0.0;
        let (j, m, tc) = (jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        s.resources.add(ResourceKind::Fuel, 200.0);
        let target = a_reachable_target(&s, &w);
        s.apply(&w, Action::Jump(target)).unwrap();
        // Arrive.
        s.advance(&w, 64);
        assert_eq!(s.expedition.location, target);
        // Clear any contact that opened on arrival — this test covers site-resolution,
        // not contact blocking. A real player would resolve the contact first.
        s.expedition.contact = None;
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
        let mut t = Tuning::default();
        // Disable contact rolls so no ship combat can emergency_retreat the
        // ship home before this test can assert it is still away.
        t.contact_chance = 0.0;
        let (j, m, tc) = (jobs(), modules(), techs());
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
        let mut t = Tuning::default();
        // Disable contact rolls so no ship combat can emergency_retreat the
        // ship home before this test strands and scavenges.
        t.contact_chance = 0.0;
        let (j, m, tc) = (jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 4);
        s.resources.add(ResourceKind::Fuel, 200.0);
        let target = a_reachable_target(&s, &w);
        s.apply(&w, Action::Jump(target)).unwrap();
        s.advance(&w, 64); // arrive
        // Clear any contact that opened on arrival — this test covers scavenge
        // behaviour, not contact blocking. A real player would resolve it first.
        s.expedition.contact = None;
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
        // Clear any contact that opened on arrival so we test the "not stranded"
        // rejection, not the contact-blocking rejection.
        s.expedition.contact = None;
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

    #[test]
    fn backtrack_to_entry_is_accepted() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Frozen);
        s.apply(&w, Action::Land(idx)).unwrap();
        // Find the first outgoing edge from entry node 0.
        let site = s.surface_for_test(&w);
        let child = site.nodes[0].edges[0];
        // Move forward to child.
        assert!(s.apply(&w, Action::MoveToNode(child)).is_ok());
        // Advance traverse_ticks so the player arrives at child.
        s.advance(&w, t.traverse_ticks as u64);
        assert_eq!(s.expedition.landing.as_ref().unwrap().node, child);
        // Non-neighbor must still be rejected, with the unchanged message.
        assert_eq!(
            s.apply(&w, Action::MoveToNode(999)),
            Err(Rejected("no route to that node"))
        );
        // Move back to entry (node 0) — bidirectional backtrack.
        assert!(s.apply(&w, Action::MoveToNode(0)).is_ok());
        // Advancing arrives back at the entry node: the back-move is real.
        s.advance(&w, t.traverse_ticks as u64);
        assert_eq!(s.expedition.landing.as_ref().unwrap().node, 0);
    }

    #[test]
    fn squad_defeats_a_remnant_and_salvages() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.ship.cargo = 500.0;
        s.crew.total = 5;
        s.apply(&w, Action::SetAwayCrew { delta: 5 }).unwrap();
        s.gear.weapon_tier = 2;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
        s.apply(&w, Action::Land(idx)).unwrap();
        // Force a combat at the current node.
        s.expedition.landing.as_mut().unwrap().combat =
            Some(crate::combat::remnant_stats(1, 1.0, &t));
        s.expedition.cargo.insert(ResourceKind::Rations, 1000.0);
        s.advance(&w, 60); // a strong squad should win within the suit window
        assert!(
            s.expedition.landing.is_none()
                || s.expedition.landing.as_ref().unwrap().combat.is_none(),
            "combat resolved"
        );
    }

    #[test]
    fn advance_n_equals_n_times_advance_1_during_combat() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let setup = |s: &mut GameState| {
            s.ship.built = true;
            s.ship.cargo = 500.0;
            s.crew.total = 3;
            s.apply(&w, Action::SetAwayCrew { delta: 3 }).unwrap();
            // weapon_tier 0 (default) gives a weaker squad that will trade blows
            // for several ticks before defeating a depth-1 remnant.
            s.expedition.location = crate::galaxy::SystemId(1);
            let idx = plant_planet(s, crate::galaxy::Biome::Ruined);
            s.apply(&w, Action::Land(idx)).unwrap();
            // Inject combat so tick_combat runs from tick 1.
            s.expedition.landing.as_mut().unwrap().combat =
                Some(crate::combat::remnant_stats(1, 1.0, &t));
            // Rations keep the away team fed through the window.
            s.expedition.cargo.insert(ResourceKind::Rations, 1000.0);
        };
        let mut bulk = GameState::new(&w, 1);
        setup(&mut bulk);
        let mut step = GameState::new(&w, 1);
        setup(&mut step);
        // Verify combat is actually active at the start so tick_combat is exercised.
        assert!(
            bulk.expedition.landing.as_ref().unwrap().combat.is_some(),
            "combat must be active at tick 0 for this test to be meaningful"
        );
        bulk.advance(&w, 30);
        for _ in 0..30 {
            step.advance(&w, 1);
        }
        assert_eq!(bulk, step);
    }

    #[test]
    fn armory_crafting_upgrades_gear_and_stocks_consumables() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        // No Armory yet → rejected.
        assert!(s.apply(&w, Action::UpgradeWeapon).is_err());
        s.flags.insert(Flag::CombatDoctrineKnown);
        s.resources.add(ResourceKind::Alloy, 500.0);
        s.resources.add(ResourceKind::Components, 500.0);
        s.apply(&w, Action::Build(ModuleId::Armory)).unwrap();
        assert!(s.apply(&w, Action::UpgradeWeapon).is_ok());
        assert_eq!(s.gear.weapon_tier, 1);
        assert!(
            s.apply(
                &w,
                Action::CraftConsumable(crate::gear::ConsumableKind::MedGel)
            )
            .is_ok()
        );
        assert_eq!(
            s.consumables
                .get(&crate::gear::ConsumableKind::MedGel)
                .copied()
                .unwrap_or(0),
            1
        );
    }

    #[test]
    fn away_team_eats_ship_rations_then_starves() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.crew.total = 3;
        s.apply(&w, Action::SetAwayCrew { delta: 3 }).unwrap();
        s.expedition.location = crate::galaxy::SystemId(1); // away
        s.expedition.cargo.insert(ResourceKind::Rations, 10.0);
        s.advance(&w, 5);
        // Ate 3 * 0.15 * 5 = 2.25 Rations from cargo; no starvation yet.
        assert!(
            (s.expedition
                .cargo
                .get(&ResourceKind::Rations)
                .copied()
                .unwrap()
                - 7.75)
                .abs()
                < 1e-9
        );
        let morale_before = s.expedition.away_team.morale;
        s.expedition.cargo.remove(&ResourceKind::Rations); // out of food
        s.advance(&w, 3);
        assert!(
            s.expedition.away_team.morale < morale_before,
            "starvation hits morale"
        );
    }

    #[test]
    fn set_away_crew_commits_idle_and_excludes_from_economy() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.crew.total = 4;
        assert_eq!(s.crew.idle(), 4);
        assert!(s.apply(&w, Action::SetAwayCrew { delta: 2 }).is_ok());
        assert_eq!(s.expedition.away_team.crew, 2);
        assert_eq!(s.crew.away, 2);
        assert_eq!(s.crew.idle(), 2); // away excluded from idle
        assert!(s.apply(&w, Action::SetAwayCrew { delta: -1 }).is_ok());
        assert_eq!(s.expedition.away_team.crew, 1);
        assert_eq!(s.crew.idle(), 3);
    }

    /// Task 9: arriving HOME releases the away-team back to idle; a docked squad
    /// below max morale recovers morale each tick.
    #[test]
    fn return_home_releases_away_team_and_morale_recovers_while_docked() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 42);

        // Commit 3 crew to the away-team while docked.
        s.crew.total = 4;
        s.apply(&w, Action::SetAwayCrew { delta: 3 }).unwrap();
        assert_eq!(s.crew.away, 3);
        assert_eq!(s.expedition.away_team.crew, 3);

        // Simulate being away (ship in transit back to HOME).
        // We skip an actual Jump and manually drive transit completion:
        // put the ship in transit-to-HOME with 1 tick remaining.
        s.expedition.activity = Some(crate::expedition::Activity::Transit {
            to: crate::galaxy::HOME,
            ticks: 1,
        });
        s.expedition.location = crate::galaxy::SystemId(1); // pretend we left home

        // Advance 1 tick: transit completes, ship arrives at HOME.
        s.advance(&w, 1);

        // Ship is now back at HOME, activity cleared, away-team dissolved.
        assert_eq!(s.expedition.location, crate::galaxy::HOME);
        assert!(s.expedition.activity.is_none());
        assert_eq!(
            s.crew.away, 0,
            "away crew released back to idle on return home"
        );
        assert_eq!(
            s.expedition.away_team.crew, 0,
            "AwayTeam dissolved after return home"
        );
        // The 3 survivors rejoin idle (total unchanged since nobody died).
        assert_eq!(s.crew.idle(), s.crew.total, "all crew are idle again");

        // — Morale recovery while docked with a committed squad —
        // Re-commit 2 crew with low morale while docked.
        s.apply(&w, Action::SetAwayCrew { delta: 2 }).unwrap();
        // Artificially lower morale below max.
        let low_morale = t.morale_max * 0.5;
        s.expedition.away_team.morale = low_morale;

        // Advance a few ticks while still docked (NOT away).
        assert!(!s.away(), "must be docked for morale recovery");
        let ticks = 10u64;
        s.advance(&w, ticks);

        let expected_morale =
            (low_morale + t.morale_recover_per_tick * ticks as f64).min(t.morale_max);
        assert!(
            (s.expedition.away_team.morale - expected_morale).abs() < 1e-9,
            "morale recovered while docked: got {}, expected {}",
            s.expedition.away_team.morale,
            expected_morale
        );
    }

    #[test]
    fn winning_a_peer_fight_tallies_and_notes() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.ship.cargo = 500.0;
        // weapon_tier=0 so the peer mirror stays at 1× (depth-1 peer stays beatable by
        // a full 6-crew squad before morale routs). gear is passed to peer_stats so the
        // two scale together — a weapon_tier=2 peer is 2× stronger and outpaces morale.
        s.crew.total = 6;
        s.gear.weapon_tier = 0;
        // SetAwayCrew must happen while docked (before moving away).
        s.apply(&w, Action::SetAwayCrew { delta: 6 }).unwrap();
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
        s.apply(&w, Action::Land(idx)).unwrap();
        s.expedition.landing.as_mut().unwrap().combat =
            Some(crate::alien::peer_stats(1, 1.0, s.gear, &t));
        s.expedition.cargo.insert(ResourceKind::Rations, 1000.0);
        s.advance(&w, 200);
        assert_eq!(s.peers_killed, 1);
        assert!(s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstPeerKilled
        )));
    }

    #[test]
    fn parley_de_escalates_and_avoid_leaves() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
        s.apply(&w, Action::Land(idx)).unwrap();
        s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
            kind: crate::alien::AlienKind::Peer,
            disposition: 0.0,
        });
        let before = s
            .expedition
            .landing
            .as_ref()
            .unwrap()
            .alien
            .as_ref()
            .unwrap()
            .disposition;
        s.apply(&w, Action::Parley).unwrap();
        assert!(
            s.expedition
                .landing
                .as_ref()
                .unwrap()
                .alien
                .as_ref()
                .unwrap()
                .disposition
                > before
        );
        s.apply(&w, Action::Avoid).unwrap();
        assert!(s.expedition.landing.as_ref().unwrap().alien.is_none());
    }

    #[test]
    fn gift_de_escalates_trade_swaps() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
        s.apply(&w, Action::Land(idx)).unwrap();
        // Hostile peer + Components to gift.
        s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
            kind: crate::alien::AlienKind::Peer,
            disposition: -60.0,
        });
        s.expedition.cargo.insert(ResourceKind::Components, 100.0);
        s.apply(&w, Action::Gift).unwrap();
        assert!(
            s.expedition
                .landing
                .as_ref()
                .unwrap()
                .alien
                .as_ref()
                .unwrap()
                .disposition
                > -60.0
        );

        // — Trade leg: Curious peer, verify cargo swap + disposition rise —
        // Set disposition to Curious band (>= disp_curious = 30.0).
        s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
            kind: crate::alien::AlienKind::Peer,
            disposition: 35.0,
        });
        let comp_before = s
            .expedition
            .cargo
            .get(&ResourceKind::Components)
            .copied()
            .unwrap_or(0.0);
        let iso_before = s
            .expedition
            .cargo
            .get(&ResourceKind::Isotopes)
            .copied()
            .unwrap_or(0.0);
        let disp_before = s
            .expedition
            .landing
            .as_ref()
            .unwrap()
            .alien
            .as_ref()
            .unwrap()
            .disposition;
        s.apply(&w, Action::Trade).unwrap();
        let comp_after = s
            .expedition
            .cargo
            .get(&ResourceKind::Components)
            .copied()
            .unwrap_or(0.0);
        let iso_after = s
            .expedition
            .cargo
            .get(&ResourceKind::Isotopes)
            .copied()
            .unwrap_or(0.0);
        let disp_after = s
            .expedition
            .landing
            .as_ref()
            .unwrap()
            .alien
            .as_ref()
            .unwrap()
            .disposition;
        assert!(
            comp_after < comp_before,
            "Components should decrease after Trade"
        );
        assert!(
            iso_after > iso_before,
            "Isotopes should increase after Trade"
        );
        assert!(
            disp_after > disp_before,
            "Disposition should rise after Trade"
        );
        // Curious band: isotope gain = trade_get * CURIOUS_TRADE_BONUS
        let expected_iso_gain = t.trade_get * crate::state::CURIOUS_TRADE_BONUS;
        assert!(
            (iso_after - iso_before - expected_iso_gain).abs() < 1e-9,
            "Curious trade bonus not applied correctly: got {}, expected {}",
            iso_after - iso_before,
            expected_iso_gain
        );
    }

    #[test]
    fn harvest_fauna_and_attack_starts_combat() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.ship.cargo = 500.0;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Verdant);
        s.apply(&w, Action::Land(idx)).unwrap();
        s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
            kind: crate::alien::AlienKind::Fauna,
            disposition: -20.0,
        });
        s.apply(&w, Action::Harvest).unwrap();
        assert!(
            s.expedition
                .landing
                .as_ref()
                .unwrap()
                .satchel
                .values()
                .copied()
                .sum::<f64>()
                > 0.0
        );
        assert!(s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstFaunaDisplaced
        )));
        assert!(s.expedition.landing.as_ref().unwrap().alien.is_none()); // harvested = resolved

        // — AttackAlien leg: set a fresh Fauna alien, dispatch AttackAlien, assert combat starts —
        s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
            kind: crate::alien::AlienKind::Fauna,
            disposition: -30.0,
        });
        s.apply(&w, Action::AttackAlien).unwrap();
        assert!(
            s.expedition.landing.as_ref().unwrap().combat.is_some(),
            "combat should be set after AttackAlien"
        );
        assert!(
            s.expedition.landing.as_ref().unwrap().alien.is_none(),
            "alien should be cleared after AttackAlien"
        );

        // — AttackAlien leg: Peer alien also converts to combat —
        if let Some(l) = s.expedition.landing.as_mut() {
            l.combat = None; // clear previous combat
        }
        s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
            kind: crate::alien::AlienKind::Peer,
            disposition: -50.0,
        });
        s.apply(&w, Action::AttackAlien).unwrap();
        assert!(
            s.expedition.landing.as_ref().unwrap().combat.is_some(),
            "combat should be set after AttackAlien on a Peer"
        );
        assert!(
            s.expedition.landing.as_ref().unwrap().alien.is_none(),
            "alien should be cleared after AttackAlien on a Peer"
        );
    }

    #[test]
    fn harvest_rejects_non_fauna_and_missing_alien() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.ship.cargo = 500.0;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Verdant);
        s.apply(&w, Action::Land(idx)).unwrap();
        // No alien present → reject
        assert!(s.apply(&w, Action::Harvest).is_err());
        // Peer alien → reject
        s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
            kind: crate::alien::AlienKind::Peer,
            disposition: 0.0,
        });
        assert_eq!(
            s.apply(&w, Action::Harvest),
            Err(Rejected("that is not an animal"))
        );
    }

    #[test]
    fn attack_alien_rejects_when_no_alien() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.ship.cargo = 500.0;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Verdant);
        s.apply(&w, Action::Land(idx)).unwrap();
        assert!(s.apply(&w, Action::AttackAlien).is_err());
    }

    #[test]
    fn advance_n_equals_n_times_advance_1_during_alien_negotiation() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let setup = |s: &mut GameState| {
            s.ship.built = true;
            s.ship.cargo = 500.0;
            s.crew.total = 3;
            s.apply(&w, Action::SetAwayCrew { delta: 3 }).unwrap();
            s.expedition.location = crate::galaxy::SystemId(1);
            let idx = plant_planet(s, crate::galaxy::Biome::Ruined);
            s.apply(&w, Action::Land(idx)).unwrap();
            // Inject an open Peer alien at neutral disposition.  No verb is applied so
            // disposition stays static; the Suit drains each tick — that is the live
            // sub-state being advanced deterministically.
            s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
                kind: crate::alien::AlienKind::Peer,
                disposition: 0.0,
            });
            // Rations keep the away team fed through the window.
            s.expedition.cargo.insert(ResourceKind::Rations, 1000.0);
        };
        let mut bulk = GameState::new(&w, 1);
        setup(&mut bulk);
        let mut step = GameState::new(&w, 1);
        setup(&mut step);
        // Sanity: alien must be present at tick 0 or the test proves nothing.
        assert!(
            bulk.expedition.landing.as_ref().unwrap().alien.is_some(),
            "alien must be active at tick 0 for this test to be meaningful"
        );
        bulk.advance(&w, 20);
        for _ in 0..20 {
            step.advance(&w, 1);
        }
        assert_eq!(bulk, step);
    }

    // --- CB3 Task 4 tests ---

    /// Arriving in a system that has a seeded Peer contact surfaces the contact
    /// and blocks ScanSystem while it is open.
    /// Pinned seed: world_seed=42 + system=4 → roll_contact returns Some(Peer).
    #[test]
    fn arrival_surfaces_contact_and_blocks_scan() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        // world_seed=42 so roll_contact(42, 4, ..) → Some(Peer)
        let mut s = GameState::new(&w, 42);
        s.ship.built = true;

        let target = crate::galaxy::SystemId(4);
        // Manually inject transit completion to system 4.
        s.expedition.activity = Some(crate::expedition::Activity::Transit {
            to: target,
            ticks: 1,
        });
        s.advance(&w, 1);

        // Contact must be open after arrival.
        assert!(
            s.expedition.contact.is_some(),
            "expected a contact at system 4 with seed 42"
        );
        assert_eq!(s.expedition.location, target);

        // ScanSystem must be rejected while the contact is open.
        let other = crate::galaxy::SystemId(5);
        let err = s.apply(&w, Action::ScanSystem(other)).unwrap_err();
        assert!(
            err.0.contains("resolve the contact"),
            "expected contact-block message, got: {}",
            err.0
        );
    }

    /// With XenoContactKnown set, a Peer contact opens negotiation (combat is None).
    #[test]
    fn peer_contact_with_xeno_known_opens_negotiation() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 42);
        s.ship.built = true;
        s.flags.insert(crate::flags::Flag::XenoContactKnown);

        let target = crate::galaxy::SystemId(4);
        s.expedition.activity = Some(crate::expedition::Activity::Transit {
            to: target,
            ticks: 1,
        });
        s.advance(&w, 1);

        let contact = s.expedition.contact.as_ref().expect("contact must be open");
        assert_eq!(contact.kind, crate::contact::ShipKind::Peer);
        assert!(
            contact.combat.is_none(),
            "XenoContactKnown Peer contact must open as negotiation, not combat"
        );
    }

    /// Without XenoContactKnown, a Peer contact opens as combat.
    #[test]
    fn peer_contact_without_xeno_known_opens_combat() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 42);
        s.ship.built = true;
        // XenoContactKnown is NOT set

        let target = crate::galaxy::SystemId(4);
        s.expedition.activity = Some(crate::expedition::Activity::Transit {
            to: target,
            ticks: 1,
        });
        s.advance(&w, 1);

        let contact = s.expedition.contact.as_ref().expect("contact must be open");
        assert_eq!(contact.kind, crate::contact::ShipKind::Peer);
        assert!(
            contact.combat.is_some(),
            "Peer contact without XenoContactKnown must open as combat"
        );
    }

    /// Ship combat: injecting a Peer contact with low enemy_hp wins within the
    /// advance window, salvages to expedition cargo, tallies peers_killed, fires
    /// the FirstPeerKilled note, clears the contact, and records resolved_contacts.
    #[test]
    fn ship_combat_peer_win_tallies_salvage_and_note() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.ship.hull = t.ship_base_hull; // full hull

        let system = crate::galaxy::SystemId(5);
        s.expedition.location = system;

        // Inject a Peer ship combat with very low enemy_hp so we win in 1 tick.
        let mut combat = crate::contact::peer_ship_stats(1, 1.0, &s.ship, &t);
        combat.enemy_hp = 0.1; // trivially low
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: Some(combat),
        });

        s.advance(&w, 5);

        assert_eq!(s.peers_killed, 1, "peer kill tallied");
        assert!(
            s.codex.has(crate::codex::DiscoveryId::Note(
                crate::codex::NoteId::FirstPeerKilled
            )),
            "FirstPeerKilled note added"
        );
        assert!(s.expedition.contact.is_none(), "contact cleared after win");
        assert!(
            s.resolved_contacts.contains(&system.0),
            "system recorded in resolved_contacts"
        );
        assert!(
            s.expedition
                .cargo
                .get(&ResourceKind::Components)
                .copied()
                .unwrap_or(0.0)
                > 0.0,
            "Components salvaged"
        );
        assert!(
            s.expedition
                .cargo
                .get(&ResourceKind::Isotopes)
                .copied()
                .unwrap_or(0.0)
                > 0.0,
            "Isotopes salvaged"
        );
        assert!(
            s.expedition
                .cargo
                .get(&ResourceKind::Data)
                .copied()
                .unwrap_or(0.0)
                > 0.0,
            "Data salvaged"
        );
    }

    /// Resolved systems do not surface a second contact on re-arrival.
    #[test]
    fn resolved_system_skips_contact_roll() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 42);
        s.ship.built = true;

        let target = crate::galaxy::SystemId(4);
        // Mark system 4 as already resolved.
        s.resolved_contacts.insert(4);

        s.expedition.activity = Some(crate::expedition::Activity::Transit {
            to: target,
            ticks: 1,
        });
        s.advance(&w, 1);

        assert!(
            s.expedition.contact.is_none(),
            "no contact for an already-resolved system"
        );
    }

    /// Hull drops to zero mid ship-combat → emergency_retreat fires:
    /// - location jumps back to prev_location
    /// - contact cleared
    /// - hull == hull_retreat_floor (clamped, not negative)
    /// - cargo reduced by retreat_cargo_loss fraction
    /// - FirstShipCrippled note fired
    /// - system NOT added to resolved_contacts (a loss ≠ resolved)
    /// - game still playable (no panic)
    #[test]
    fn emergency_retreat_on_hull_zero() {
        let mut t = Tuning::default();
        // Guarantee the player loses in a single tick: set enemy attack very high
        // and player hull very low. We will set hull directly after setup.
        t.retreat_cargo_loss = 0.4;
        t.hull_retreat_floor = 10.0;
        // Make enemy deal guaranteed damage per tick > player hull.
        t.remnant_ship_atk = 9999.0;
        t.remnant_ship_def = 0.0;

        let (j, m, tc) = (jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 7);
        s.ship.built = true;

        // Place the ship in a non-home system with a known prev_location.
        let away_system = crate::galaxy::SystemId(3);
        let prev_system = crate::galaxy::SystemId(2);
        s.expedition.location = away_system;
        s.expedition.prev_location = prev_system;

        // Put 100 units of Isotopes in cargo so we can measure the loss.
        *s.expedition
            .cargo
            .entry(ResourceKind::Isotopes)
            .or_insert(0.0) = 100.0;

        // Set hull to a tiny positive value — one tick will drive it to ≤ 0.
        s.ship.hull = 0.01;

        // Inject a Remnant ship combat (enemy still very much alive).
        let combat = crate::contact::remnant_ship_stats(1, 1.0, &t);
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Remnant,
            disposition: 0.0,
            combat: Some(combat),
        });

        // Advance — the first tick's ship combat will drive hull ≤ 0.
        s.advance(&w, 1);

        assert_eq!(
            s.expedition.location, prev_system,
            "ship retreated to prev_location"
        );
        assert!(
            s.expedition.contact.is_none(),
            "contact cleared after retreat"
        );
        assert!(
            (s.ship.hull - t.hull_retreat_floor).abs() < 1e-9,
            "hull clamped to retreat floor (was {}, want {})",
            s.ship.hull,
            t.hull_retreat_floor
        );
        let isotopes = s
            .expedition
            .cargo
            .get(&ResourceKind::Isotopes)
            .copied()
            .unwrap_or(0.0);
        let expected = 100.0 * (1.0 - t.retreat_cargo_loss);
        assert!(
            (isotopes - expected).abs() < 1e-6,
            "cargo reduced by retreat_cargo_loss: got {isotopes}, want {expected}"
        );
        assert!(
            s.codex.has(crate::codex::DiscoveryId::Note(
                crate::codex::NoteId::FirstShipCrippled
            )),
            "FirstShipCrippled note fired"
        );
        assert!(
            !s.resolved_contacts.contains(&away_system.0),
            "a loss does NOT mark the system resolved"
        );
        // Game still playable: advance a few more ticks without panic.
        s.advance(&w, 5);
    }

    // ── CB3 Task 7: ship-scoped negotiation verbs ──────────────────────────────

    /// Helper: inject a Peer ship contact with no combat (negotiation mode).
    fn peer_negotiation_contact(disposition: f64) -> crate::contact::ShipContact {
        crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition,
            combat: None,
        }
    }

    /// HailParley on a Peer near peace_band: shifts disposition, crosses peace_band
    /// → clears contact, inserts resolved_contacts, increments peers_spared,
    /// fires FirstPeerSpared note, grants observe_data Data to cargo.
    #[test]
    fn hail_parley_peer_reaches_curious_clears_and_tallies() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;

        let system = crate::galaxy::SystemId(7);
        s.expedition.location = system;
        // peace_band = 30.0, parley_shift = 18.0 — start at 15.0 so 15+18=33 >= 30
        s.expedition.contact = Some(peer_negotiation_contact(15.0));

        s.apply(&w, Action::HailParley).unwrap();

        assert!(
            s.expedition.contact.is_none(),
            "contact cleared after reaching Curious"
        );
        assert!(
            s.resolved_contacts.contains(&system.0),
            "system recorded in resolved_contacts"
        );
        assert_eq!(s.peers_spared, 1, "peers_spared incremented");
        assert!(
            s.codex.has(crate::codex::DiscoveryId::Note(
                crate::codex::NoteId::FirstPeerSpared
            )),
            "FirstPeerSpared note fired"
        );
        let data = s
            .expedition
            .cargo
            .get(&ResourceKind::Data)
            .copied()
            .unwrap_or(0.0);
        assert!(data > 0.0, "observe_data granted to cargo (got {})", data);
    }

    /// HailGift de-escalates a Hostile Peer: spends Components, shifts disposition.
    #[test]
    fn hail_gift_de_escalates_hostile_peer() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;

        let system = crate::galaxy::SystemId(8);
        s.expedition.location = system;
        // Hostile disposition (disp_hostile <= -50 by default)
        s.expedition.contact = Some(peer_negotiation_contact(-60.0));
        s.expedition.cargo.insert(ResourceKind::Components, 100.0);

        let comp_before = 100.0f64;
        s.apply(&w, Action::HailGift).unwrap();

        let comp_after = s
            .expedition
            .cargo
            .get(&ResourceKind::Components)
            .copied()
            .unwrap_or(0.0);
        assert!(
            comp_after < comp_before,
            "Components spent: before={comp_before} after={comp_after}"
        );
        let disp_after = s.expedition.contact.as_ref().unwrap().disposition;
        assert!(
            disp_after > -60.0,
            "disposition improved after Gift: {disp_after}"
        );
        // Contact stays open (no resolve on Gift)
        assert!(
            s.expedition.contact.is_some(),
            "contact remains open after Gift"
        );
    }

    /// HailAvoid on a non-Hostile Peer: clears contact + resolved_contacts, no peers_spared.
    #[test]
    fn hail_avoid_non_hostile_clears_no_tally() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;

        let system = crate::galaxy::SystemId(9);
        s.expedition.location = system;
        // Neutral disposition
        s.expedition.contact = Some(peer_negotiation_contact(0.0));

        s.apply(&w, Action::HailAvoid).unwrap();

        assert!(
            s.expedition.contact.is_none(),
            "contact cleared after HailAvoid"
        );
        assert!(
            s.resolved_contacts.contains(&system.0),
            "system recorded in resolved_contacts"
        );
        assert_eq!(s.peers_spared, 0, "HailAvoid does NOT tally peers_spared");
    }

    /// HailAvoid on a Hostile Peer is rejected — "they will not let you pass".
    #[test]
    fn hail_avoid_hostile_rejected() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;

        let system = crate::galaxy::SystemId(10);
        s.expedition.location = system;
        // disp_hostile = -50.0 by default; -80 is firmly Hostile
        s.expedition.contact = Some(peer_negotiation_contact(-80.0));

        let err = s.apply(&w, Action::HailAvoid).unwrap_err();
        assert!(
            err.0.contains("will not let you pass"),
            "expected rejection, got: {}",
            err.0
        );
        assert!(
            s.expedition.contact.is_some(),
            "contact must remain open after rejection"
        );
    }

    // ── CB3 Task 8: ship combat inputs ────────────────────────────────────────

    /// AttackShip on a Peer negotiation Contact opens ship combat with sane stats.
    /// Crucially: enemy_hp must be < 500 (depth=0 scaling only, not distance).
    #[test]
    fn attack_ship_peer_starts_combat_sane_stats() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;

        let system = crate::galaxy::SystemId(11);
        s.expedition.location = system;
        // Peer in negotiation mode (no combat yet).
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: None,
        });

        s.apply(&w, Action::AttackShip).unwrap();

        let combat = s
            .expedition
            .contact
            .as_ref()
            .expect("contact still present")
            .combat
            .as_ref()
            .expect("combat now set");
        assert!(
            combat.enemy_hp > 0.0,
            "enemy_hp must be positive (got {})",
            combat.enemy_hp
        );
        // Depth=0 scaling: enemy_hp = peer_ship_hp * grad * mirror.
        // At system 11 from HOME the grad is small; with depth distance-as-depth
        // the value would be enormous. Cap at 500 as the regression guard.
        assert!(
            combat.enemy_hp < 500.0,
            "enemy_hp < 500 (depth=0 guard, got {})",
            combat.enemy_hp
        );
    }

    /// AttackShip on a Remnant contact opens combat.
    #[test]
    fn attack_ship_remnant_starts_combat() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;

        s.expedition.location = crate::galaxy::SystemId(12);
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Remnant,
            disposition: 0.0,
            combat: None,
        });

        s.apply(&w, Action::AttackShip).unwrap();
        assert!(
            s.expedition.contact.as_ref().unwrap().combat.is_some(),
            "combat opened for Remnant"
        );
    }

    /// AttackShip is rejected when there is no contact.
    #[test]
    fn attack_ship_rejected_no_contact() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        let err = s.apply(&w, Action::AttackShip).unwrap_err();
        assert!(err.0.contains("no contact"), "got: {}", err.0);
    }

    /// AttackShip is rejected when combat is already ongoing.
    #[test]
    fn attack_ship_rejected_already_fighting() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(13);
        let grad = 1.0;
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
        });
        let err = s.apply(&w, Action::AttackShip).unwrap_err();
        assert!(err.0.contains("already"), "got: {}", err.0);
    }

    /// SetShipStance flips the combat stance.
    #[test]
    fn set_ship_stance_flips_stance() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(14);
        let grad = 1.0;
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
        });
        // Default stance is Aggressive; switch to Defensive.
        s.apply(&w, Action::SetShipStance(crate::combat::Stance::Defensive))
            .unwrap();
        let stance = s
            .expedition
            .contact
            .as_ref()
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .stance;
        assert_eq!(
            stance,
            crate::combat::Stance::Defensive,
            "stance is now Defensive"
        );

        // Switch back.
        s.apply(&w, Action::SetShipStance(crate::combat::Stance::Aggressive))
            .unwrap();
        let stance2 = s
            .expedition
            .contact
            .as_ref()
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .stance;
        assert_eq!(
            stance2,
            crate::combat::Stance::Aggressive,
            "stance toggled back"
        );
    }

    /// SetShipStance rejected when no active ship combat.
    #[test]
    fn set_ship_stance_rejected_no_combat() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        let err = s
            .apply(&w, Action::SetShipStance(crate::combat::Stance::Defensive))
            .unwrap_err();
        assert!(err.0.contains("no active ship combat"), "got: {}", err.0);
    }

    /// UseRepairDrone heals hull by repair_drone_heal (clamped to hull_max)
    /// and decrements repair_drones.
    #[test]
    fn use_repair_drone_heals_and_decrements() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(15);
        let grad = 1.0;
        // Damage the hull first.
        let hull_max = s.ship.hull_max(&t);
        s.ship.hull = hull_max - t.repair_drone_heal - 5.0;
        s.ship.repair_drones = 1;
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
        });
        let hull_before = s.ship.hull;
        s.apply(&w, Action::UseRepairDrone).unwrap();

        let expected = (hull_before + t.repair_drone_heal).min(hull_max);
        assert!(
            (s.ship.hull - expected).abs() < 1e-9,
            "hull healed: got {}, want {}",
            s.ship.hull,
            expected
        );
        assert_eq!(s.ship.repair_drones, 0, "drone count decremented");
    }

    /// UseRepairDrone clamps heal at hull_max.
    #[test]
    fn use_repair_drone_clamps_at_hull_max() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(16);
        let grad = 1.0;
        let hull_max = s.ship.hull_max(&t);
        // Hull already near full.
        s.ship.hull = hull_max - 1.0;
        s.ship.repair_drones = 1;
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
        });
        s.apply(&w, Action::UseRepairDrone).unwrap();
        assert!(
            (s.ship.hull - hull_max).abs() < 1e-9,
            "hull clamped to hull_max: got {}, want {}",
            s.ship.hull,
            hull_max
        );
    }

    /// UseRepairDrone rejected when no active combat.
    #[test]
    fn use_repair_drone_rejected_no_combat() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.repair_drones = 3;
        let err = s.apply(&w, Action::UseRepairDrone).unwrap_err();
        assert!(err.0.contains("no active ship combat"), "got: {}", err.0);
    }

    /// UseRepairDrone rejected when out of stock.
    #[test]
    fn use_repair_drone_rejected_out_of_stock() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(17);
        let grad = 1.0;
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
        });
        // repair_drones = 0 (default)
        let err = s.apply(&w, Action::UseRepairDrone).unwrap_err();
        assert!(err.0.contains("no repair drones"), "got: {}", err.0);
    }

    /// ShipFlee clears contact, moves to prev_location, and spends Fuel.
    #[test]
    fn ship_flee_clears_contact_and_spends_fuel() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;

        let away_system = crate::galaxy::SystemId(5);
        let prev_system = crate::galaxy::HOME;
        s.expedition.location = away_system;
        s.expedition.prev_location = prev_system;
        // Give enough fuel to flee.
        s.resources.fuel = 9999.0;
        let fuel_before = s.resources.fuel;
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: None,
        });

        s.apply(&w, Action::ShipFlee).unwrap();

        assert!(
            s.expedition.contact.is_none(),
            "contact cleared after ShipFlee"
        );
        assert_eq!(
            s.expedition.location, prev_system,
            "location is now prev_location"
        );
        // NOT in resolved_contacts (we fled, not resolved).
        assert!(
            !s.resolved_contacts.contains(&away_system.0),
            "ShipFlee must NOT mark system resolved"
        );
        let fuel_after = s.resources.fuel;
        assert!(
            fuel_after < fuel_before,
            "fuel spent fleeing: before={fuel_before} after={fuel_after}"
        );
    }

    /// ShipFlee is rejected when Fuel is insufficient.
    #[test]
    fn ship_flee_rejected_no_fuel() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(6);
        s.expedition.prev_location = crate::galaxy::HOME;
        // No fuel at all.
        s.resources.fuel = 0.0;
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: None,
        });

        let err = s.apply(&w, Action::ShipFlee).unwrap_err();
        assert!(
            err.0.contains("no fuel"),
            "expected no-fuel rejection, got: {}",
            err.0
        );
        // Contact must remain.
        assert!(
            s.expedition.contact.is_some(),
            "contact remains after failed flee"
        );
    }

    /// ShipFlee is rejected when no contact is present.
    #[test]
    fn ship_flee_rejected_no_contact() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        let err = s.apply(&w, Action::ShipFlee).unwrap_err();
        assert!(err.0.contains("no contact"), "got: {}", err.0);
    }

    // ── CB3 Task 9: ship upgrade actions ─────────────────────────────────────

    fn shipyard_ready(s: &mut GameState, w: &World) {
        s.flags.insert(Flag::MunitionsKnown);
        s.flags.insert(Flag::JumpTheoryKnown);
        s.resources.add(ResourceKind::Alloy, 500.0);
        s.resources.add(ResourceKind::Components, 500.0);
        s.resources.add(ResourceKind::Scrap, 500.0);
        s.apply(w, Action::Build(ModuleId::Shipyard)).unwrap();
    }

    #[test]
    fn upgrade_ship_weapon_raises_tier_and_spends_resources() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        shipyard_ready(&mut s, &w);
        let alloy_before = s.resources.amount(ResourceKind::Alloy);
        let comp_before = s.resources.amount(ResourceKind::Components);
        assert!(s.apply(&w, Action::UpgradeShipWeapon).is_ok());
        assert_eq!(s.ship.ship_weapon_tier, 1);
        assert!(
            s.resources.amount(ResourceKind::Alloy) < alloy_before,
            "alloy spent"
        );
        assert!(
            s.resources.amount(ResourceKind::Components) < comp_before,
            "components spent"
        );
    }

    #[test]
    fn upgrade_ship_weapon_rejected_at_max_tier() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        shipyard_ready(&mut s, &w);
        s.ship.ship_weapon_tier = 2;
        let err = s.apply(&w, Action::UpgradeShipWeapon).unwrap_err();
        assert!(
            err.0.contains("max tier"),
            "expected max-tier rejection, got: {}",
            err.0
        );
    }

    #[test]
    fn upgrade_ship_weapon_rejected_without_munitions_known() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        // No MunitionsKnown flag, no shipyard.
        let err = s.apply(&w, Action::UpgradeShipWeapon).unwrap_err();
        assert!(
            err.0.contains("munitions"),
            "expected munitions rejection, got: {}",
            err.0
        );
    }

    #[test]
    fn upgrade_ship_weapon_rejected_without_shipyard() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.flags.insert(Flag::MunitionsKnown);
        // No shipyard built.
        let err = s.apply(&w, Action::UpgradeShipWeapon).unwrap_err();
        assert!(
            err.0.contains("shipyard"),
            "expected no-shipyard rejection, got: {}",
            err.0
        );
    }

    #[test]
    fn upgrade_hull_plating_raises_tier_and_hull() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        shipyard_ready(&mut s, &w);
        let hull_before = s.ship.hull;
        assert!(s.apply(&w, Action::UpgradeHullPlating).is_ok());
        assert_eq!(s.ship.hull_plating_tier, 1);
        assert!(
            s.ship.hull > hull_before,
            "hull should increase by hull_plating_hp_step"
        );
        assert_eq!(
            s.ship.hull,
            (hull_before + t.hull_plating_hp_step).min(s.ship.hull_max(&t))
        );
    }

    #[test]
    fn upgrade_hull_plating_damaged_ship_no_clamp() {
        // Damaged ship: hull well below old max, so the full step lands without clamping.
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        shipyard_ready(&mut s, &w);
        let hull_max_before = s.ship.hull_max(&t);
        // Damage the ship so hull is far below the old max.
        s.ship.hull = 50.0;
        assert!(
            50.0 < hull_max_before,
            "sanity: 50 must be below pre-upgrade hull_max"
        );
        let hull_before = s.ship.hull;
        assert!(s.apply(&w, Action::UpgradeHullPlating).is_ok());
        let hull_max_after = s.ship.hull_max(&t);
        // hull_max must have increased by hull_plating_hp_step.
        assert!(
            hull_max_after > hull_max_before,
            "hull_max should increase after upgrade"
        );
        assert!(
            (hull_max_after - hull_max_before - t.hull_plating_hp_step).abs() < 1e-9,
            "hull_max increased by exactly hull_plating_hp_step"
        );
        // Hull rose by exactly the step (no clamping, since hull is still well below new max).
        assert!(
            (s.ship.hull - hull_before - t.hull_plating_hp_step).abs() < 1e-9,
            "hull rose by exactly hull_plating_hp_step with no clamping"
        );
        assert_eq!(s.ship.hull_plating_tier, 1, "tier incremented");
    }

    #[test]
    fn upgrade_hull_plating_rejected_at_max_tier() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        shipyard_ready(&mut s, &w);
        s.ship.hull_plating_tier = 2;
        let err = s.apply(&w, Action::UpgradeHullPlating).unwrap_err();
        assert!(
            err.0.contains("max tier"),
            "expected max-tier rejection, got: {}",
            err.0
        );
    }

    #[test]
    fn upgrade_hull_plating_rejected_without_munitions_known() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        let err = s.apply(&w, Action::UpgradeHullPlating).unwrap_err();
        assert!(
            err.0.contains("munitions"),
            "expected munitions rejection, got: {}",
            err.0
        );
    }

    #[test]
    fn upgrade_hull_plating_rejected_without_shipyard() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.flags.insert(Flag::MunitionsKnown);
        let err = s.apply(&w, Action::UpgradeHullPlating).unwrap_err();
        assert!(
            err.0.contains("shipyard"),
            "expected no-shipyard rejection, got: {}",
            err.0
        );
    }

    #[test]
    fn craft_repair_drone_spends_components_and_increments_count() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        shipyard_ready(&mut s, &w);
        let comp_before = s.resources.amount(ResourceKind::Components);
        let drones_before = s.ship.repair_drones;
        assert!(s.apply(&w, Action::CraftRepairDrone).is_ok());
        assert_eq!(s.ship.repair_drones, drones_before + 1);
        assert!(
            s.resources.amount(ResourceKind::Components) < comp_before,
            "components spent"
        );
    }

    #[test]
    fn craft_repair_drone_rejected_without_munitions_known() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        let err = s.apply(&w, Action::CraftRepairDrone).unwrap_err();
        assert!(
            err.0.contains("munitions"),
            "expected munitions rejection, got: {}",
            err.0
        );
    }

    #[test]
    fn craft_repair_drone_rejected_without_shipyard() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        s.flags.insert(Flag::MunitionsKnown);
        let err = s.apply(&w, Action::CraftRepairDrone).unwrap_err();
        assert!(
            err.0.contains("shipyard"),
            "expected no-shipyard rejection, got: {}",
            err.0
        );
    }

    #[test]
    fn hull_regen_docked_at_home_below_max() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        // Damage hull below max.
        let hull_max = s.ship.hull_max(&t);
        s.ship.hull = hull_max - 10.0;
        let hull_before = s.ship.hull;
        // Advance 1 tick while docked at HOME with no activity/contact.
        assert_eq!(s.expedition.location, crate::galaxy::HOME);
        assert!(s.expedition.activity.is_none());
        assert!(s.expedition.contact.is_none());
        s.advance(&w, 1);
        assert!(
            s.ship.hull > hull_before,
            "hull should regen while docked at HOME"
        );
        assert!(
            (s.ship.hull - hull_before - t.hull_regen_docked).abs() < 1e-9,
            "hull should regen by exactly hull_regen_docked per tick"
        );
    }

    #[test]
    fn hull_regen_capped_at_hull_max() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        let hull_max = s.ship.hull_max(&t);
        // Hull is already at max.
        s.ship.hull = hull_max;
        s.advance(&w, 5);
        assert!(s.ship.hull <= hull_max, "hull must not exceed hull_max");
        assert_eq!(
            s.ship.hull, hull_max,
            "hull stays at max, does not overflow"
        );
    }

    #[test]
    fn hull_regen_does_not_occur_away_from_home() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        let hull_max = s.ship.hull_max(&t);
        s.ship.hull = hull_max - 10.0;
        let hull_before = s.ship.hull;
        // Move to a different system (simulate being away by changing location).
        s.expedition.location = crate::galaxy::SystemId(5);
        s.advance(&w, 1);
        assert_eq!(
            s.ship.hull, hull_before,
            "hull must not regen when away from HOME"
        );
    }

    #[test]
    fn hull_regen_suppressed_by_active_contact_at_home() {
        // Even at HOME with hull below max, regen must NOT fire when a contact is present.
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let mut s = GameState::new(&w, 1);
        let hull_max = s.ship.hull_max(&t);
        s.ship.hull = hull_max - 10.0;
        let hull_before = s.ship.hull;
        // Stay at HOME (default location).
        assert_eq!(s.expedition.location, crate::galaxy::HOME);
        assert!(s.expedition.activity.is_none());
        // Inject a minimal contact.
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: None,
        });
        s.advance(&w, 1);
        assert_eq!(
            s.ship.hull, hull_before,
            "hull must not regen while a contact is active, even at HOME"
        );
    }

    // ── CB3 Task 10: determinism + re-entry tests ─────────────────────────────

    /// Ship-combat determinism: advance(20) == 20 × advance(1) with an active
    /// Peer ship fight injected at tick 0.  Hull drains each tick; the fight
    /// resolves inside the window, exercising both combat ticks and the win path.
    #[test]
    fn advance_n_equals_n_times_advance_1_during_ship_combat() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);
        let setup = |s: &mut GameState| {
            s.ship.built = true;
            // Place ship in a non-home system so the expedition is "away".
            s.expedition.location = crate::galaxy::SystemId(5);
            // Inject an active Peer ship fight.  At tier-0, player atk=15 vs
            // peer def=4 → 11 damage/tick; peer hp=90 → resolves in ~9 ticks.
            let combat = crate::contact::peer_ship_stats(0, 1.0, &s.ship, &t);
            // High enough HP that the fight lasts multiple ticks (default 90.0 is fine).
            s.expedition.contact = Some(crate::contact::ShipContact {
                kind: crate::contact::ShipKind::Peer,
                disposition: 0.0,
                combat: Some(combat),
            });
        };
        let mut bulk = GameState::new(&w, 1);
        setup(&mut bulk);
        let mut step = GameState::new(&w, 1);
        setup(&mut step);

        // Sanity: combat must be active at tick 0 for this test to be meaningful.
        assert!(
            bulk.expedition.contact.as_ref().unwrap().combat.is_some(),
            "ship combat must be active at tick 0 for this test to be meaningful"
        );

        bulk.advance(&w, 20);
        for _ in 0..20 {
            step.advance(&w, 1);
        }
        assert_eq!(
            bulk, step,
            "bulk advance and stepped advance must be identical"
        );
    }

    /// Contact-roll determinism and re-entry skip:
    /// 1. `roll_contact` returns the same result on two calls with the same args.
    /// 2. A system in `resolved_contacts` is skipped by `maybe_start_contact`.
    /// 3. A non-resolved system whose roll is `Some` DOES surface a contact.
    #[test]
    fn ship_contact_roll_is_deterministic_and_resolved_skips() {
        let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
        let w = world(&t, &j, &m, &tc);

        // ── 1. roll_contact is pure (same result on two calls) ─────────────────
        let sys4 = crate::galaxy::SystemId(4);
        let first = crate::contact::roll_contact(42, sys4, &t);
        let second = crate::contact::roll_contact(42, sys4, &t);
        assert_eq!(
            first, second,
            "roll_contact must return the same result on repeated calls"
        );
        // Pinned: seed=42, system=4 → Some(Peer) (verified by existing T4 tests).
        assert_eq!(
            first,
            Some(crate::contact::ShipKind::Peer),
            "pinned seed: roll_contact(42, 4) must be Some(Peer)"
        );

        // ── 2. Resolved system skips contact on re-arrival ─────────────────────
        {
            let mut s = GameState::new(&w, 42);
            s.ship.built = true;
            // Mark system 4 as already resolved.
            s.resolved_contacts.insert(4);
            s.expedition.activity =
                Some(crate::expedition::Activity::Transit { to: sys4, ticks: 1 });
            s.advance(&w, 1);
            assert!(
                s.expedition.contact.is_none(),
                "resolved system must not surface a contact on re-arrival"
            );
            assert_eq!(s.expedition.location, sys4);
        }

        // ── 3. Non-resolved system with a hot roll DOES surface a contact ──────
        {
            let mut s = GameState::new(&w, 42);
            s.ship.built = true;
            // System 4 is NOT in resolved_contacts.
            assert!(
                !s.resolved_contacts.contains(&4),
                "system 4 must start unresolved"
            );
            s.expedition.activity =
                Some(crate::expedition::Activity::Transit { to: sys4, ticks: 1 });
            s.advance(&w, 1);
            assert!(
                s.expedition.contact.is_some(),
                "non-resolved system with a hot roll must surface a contact"
            );
        }
    }
}
