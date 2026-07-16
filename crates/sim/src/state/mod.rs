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

mod apply;
mod encounter;
mod narrative;
mod tick;

#[cfg(test)]
mod tests;

pub(crate) use narrative::{
    ANOTHER_VESSEL_BODY, COMMUNION_THREAD_BODY, MIRROR_IN_THE_DARK_BODY, ending_note_id,
    ending_text,
};

/// Cumulative scrap salvaged that triggers the heavy-consumption uneasy note.
const HEAVY_CONSUMPTION_SCRAP: f64 = 500.0;

/// Multiplier applied to `trade_get` when the alien is in the Curious band.
pub const CURIOUS_TRADE_BONUS: f64 = 1.5;

pub const SAVE_VERSION: u32 = 11;

/// Content registries supplied to the engine each call (like `Tuning`, these are
/// not part of the saved state).
pub struct World<'a> {
    pub tuning: &'a Tuning,
    pub jobs: &'a JobRegistry,
    pub modules: &'a ModuleRegistry,
    pub techs: &'a crate::research::TechRegistry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ending {
    Ascendant,
    Communion,
    PatternBroken,
    Cold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Endgame {
    WatchedArrived,
    Ended(Ending),
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
    ChooseEnding(Ending),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected(pub &'static str);

/// Memoized galaxy, lazily derived from `world_seed` + tuning. Never serialized
/// (saves regenerate it) and deliberately transparent to `PartialEq`: a filled
/// cache must not make otherwise-equal states unequal (the determinism tests
/// compare states across paths that may or may not have touched the galaxy).
/// Invariant: tuning is constant for the lifetime of a run; anything that
/// changes galaxy-shaping tuning mid-game must construct a fresh `GameState`.
#[derive(Debug, Clone, Default)]
pub struct GalaxyCache(std::cell::OnceCell<Galaxy>);

impl PartialEq for GalaxyCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

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
    pub endgame: Option<Endgame>,
    #[serde(skip)]
    galaxy_cache: GalaxyCache,
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
            endgame: None,
            galaxy_cache: GalaxyCache::default(),
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
        let _ = state.galaxy_cache.0.set(galaxy);
        state
    }

    pub fn origin_id(&self) -> Origin {
        self.origin.id
    }

    /// The (unsaved) galaxy for this run: generated from the world seed on
    /// first use, memoized afterwards. See [`GalaxyCache`] for the invariants.
    pub fn galaxy(&self, tuning: &Tuning) -> &Galaxy {
        self.galaxy_cache
            .0
            .get_or_init(|| Galaxy::generate(self.world_seed, tuning))
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

    /// Cumulative expansion, derived from seeded state. Monotonic non-decreasing.
    pub fn expansion_score(&self, world: &World) -> f64 {
        let t = world.tuning;
        let peers = (self.peers_spared + self.peers_killed) as f64;
        let worlds_stripped: usize = self
            .galaxy_state
            .sites
            .values()
            .flat_map(|v| v.iter())
            .map(|s| s.resolved_nodes.len())
            .sum();
        let systems_visited = self.galaxy_state.sites.len() as f64;
        t.expansion_w_peers * peers
            + t.expansion_w_strip * worlds_stripped as f64
            + t.expansion_w_consume * self.total_scrap_gained
            + t.expansion_w_systems * systems_visited
    }

    /// Vocabulary-cooling tier (0–3), derived from expansion. Monotonic.
    pub fn expansion_tier(&self, world: &World) -> u8 {
        let s = self.expansion_score(world);
        let t = world.tuning;
        if s >= t.drift_t3 {
            3
        } else if s >= t.drift_t2 {
            2
        } else if s >= t.drift_t1 {
            1
        } else {
            0
        }
    }

    /// The endings the player has earned (the watched offers only these).
    pub fn available_endings(&self, t: &Tuning) -> Vec<Ending> {
        let mut out = vec![Ending::Ascendant, Ending::Cold]; // always available
        let spared_life = self.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::LifeSpared,
        ));
        if self.peers_spared as f64 >= t.communion_min_spared
            && self.peers_killed as f64 <= t.communion_max_kills
        {
            out.push(Ending::Communion);
        }
        if spared_life && self.peers_killed as f64 <= t.pattern_max_kills {
            out.push(Ending::PatternBroken);
        }
        out
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
        let galaxy = self.galaxy(world.tuning);
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

    /// Current amount of `k` in the expedition's ship cargo (0.0 if absent).
    fn cargo_amount(&self, k: ResourceKind) -> f64 {
        self.expedition.cargo.get(&k).copied().unwrap_or(0.0)
    }

    /// Add `v` (may be negative) to `k` in the expedition's ship cargo. Unclamped.
    fn cargo_add(&mut self, k: ResourceKind, v: f64) {
        *self.expedition.cargo.entry(k).or_insert(0.0) += v;
    }

    /// Add to the expedition's ship cargo, clamped to remaining cargo space.
    /// (Distinct from `satchel_add`, which clamps the surface satchel instead.)
    fn cargo_add_clamped(&mut self, k: ResourceKind, amt: f64) {
        let space = (self.ship.cargo - self.expedition.cargo_total()).max(0.0);
        let take = amt.min(space);
        if take > 0.0 {
            self.cargo_add(k, take);
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
}
