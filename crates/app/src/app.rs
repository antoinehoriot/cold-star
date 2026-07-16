use dioxus::prelude::*;
use sim::Tuning;
use sim::flags::Flag;
use sim::job::JobRegistry;
use sim::module::ModuleRegistry;
use sim::origin::OriginDef;
use sim::research::{Tech, TechRegistry};
use sim::resources::ResourceKind;
use sim::state::{Action, GameState, World};

use crate::components::build::BuildMenu;
use crate::components::codex::CodexPanel;
use crate::components::jobs::JobPanel;
use crate::components::loadout::LoadoutPanel;
use crate::components::munitions::ShipMunitionsPanel;
use crate::components::power::PowerMeter;
use crate::components::priority::PriorityPanel;
use crate::components::research::ResearchPanel;
use crate::components::resources::ResourcesPanel;
use crate::screens::contact::ContactPanel;
use crate::screens::endgame::EndgamePanel;
use crate::screens::navigation::NavigationPanel;
use crate::screens::surface::SurfaceView;
use crate::style::CSS;
use crate::theme::{DriftSlot, crew_word, drift_word, emberfall_stops};
use crate::vm::{
    CombatVM, ContactVM, EndgameVM, LandingVM, LoadoutVM, NavRow, NegotiationVM, ShipMunitionsVM,
    SurfaceNodeVM,
};

const REPO_URL: &str = "https://github.com/antoinehoriot/cold-star";

#[component]
pub fn App() -> Element {
    let tuning = use_signal(|| content::load().unwrap_or_else(|_| Tuning::default()));
    let jobs = use_signal(|| content::load_jobs().map(|r| r.0).unwrap_or_default());
    let modules = use_signal(|| content::load_modules().map(|r| r.0).unwrap_or_default());
    let techs = use_signal(|| content::load_techs().map(|r| r.0).unwrap_or_default());
    let origins = use_signal(|| content::load_origins().unwrap_or_default());
    let state = use_signal(|| {
        crate::persist::load_initial(&tuning.read(), &jobs.read(), &modules.read(), &techs.read())
    });

    crate::tick::use_tick_driver(state, tuning, jobs, modules, techs, 1000.0, 8 * 60 * 60);

    let has_game = state.read().is_some();
    let mut legal = use_signal(|| Option::<&'static str>::None);

    rsx! {
        style { dangerous_inner_html: CSS }
        main { class: "console",
            h1 { class: "title", "COLD STAR" }
            if has_game {
                Console { state, tuning, jobs, modules, techs }
            } else {
                OriginSelect { state, tuning, jobs, modules, techs, origins }
            }
            footer { class: "site-footer",
                a {
                    class: "foot-link",
                    href: REPO_URL,
                    target: "_blank",
                    rel: "noopener noreferrer",
                    "Source on GitHub"
                }
                span { class: "foot-sep", "\u{00b7}" }
                button { class: "foot-link", onclick: move |_| legal.set(Some("privacy")), "Privacy" }
                span { class: "foot-sep", "\u{00b7}" }
                button { class: "foot-link", onclick: move |_| legal.set(Some("terms")), "Terms" }
            }
        }
        if let Some(page) = legal() {
            div { class: "legal-overlay", onclick: move |_| legal.set(None),
                div {
                    class: "legal-card",
                    onclick: move |e| e.stop_propagation(),
                    button {
                        class: "legal-close",
                        aria_label: "Close",
                        onclick: move |_| legal.set(None),
                        "\u{2715}"
                    }
                    if page == "privacy" {
                        h2 { class: "legal-title", "Privacy" }
                        p { "Cold Star runs entirely in your browser. There are no accounts and no servers \u{2014} nobody, including the author, receives or stores your game data." }
                        p { "Your progress is saved in your browser\u{2019}s local storage, on this device only. \u{201c}Copy save link\u{201d} packs the whole game into a URL: nothing is uploaded, the save lives inside the link itself, and it travels only if you choose to share that link." }
                        p { "There is no analytics, tracking, advertising, or cookies." }
                        p { class: "legal-mut", "The one external request the game makes is to Google Fonts (fonts.googleapis.com), to load its two typefaces. Google may receive your IP address when serving those fonts, under Google\u{2019}s own privacy policy." }
                    } else {
                        h2 { class: "legal-title", "Terms & legal" }
                        p { "Cold Star is free, open-source software, provided under the MIT License \u{201c}as is\u{201d}, without warranty of any kind. You may use, modify, and share it under that license." }
                        p { "It is a non-commercial project made for the love of the genre, inspired by \u{2014} but not affiliated with or endorsed by \u{2014} the creators of \u{2018}A Dark Room\u{2019}." }
                        p {
                            "Source code, license, and how to contribute: "
                            a {
                                class: "foot-link",
                                href: REPO_URL,
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "github.com/antoinehoriot/cold-star"
                            }
                        }
                        p { class: "legal-mut", "This is a plain-language summary for players, not legal advice. The MIT License in the repository is the authoritative text." }
                    }
                }
            }
        }
    }
}

#[component]
fn OriginSelect(
    mut state: Signal<Option<GameState>>,
    tuning: Signal<Tuning>,
    jobs: Signal<Vec<sim::job::Job>>,
    modules: Signal<Vec<sim::module::Module>>,
    techs: Signal<Vec<Tech>>,
    origins: Signal<Vec<OriginDef>>,
) -> Element {
    rsx! {
        div { class: "console-root",
            p { class: "origins-header", "Choose an origin" }
            div { class: "origins",
                for o in origins.read().iter().cloned() {
                    div {
                        class: "origin-card",
                        onclick: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            let seed = crate::persist::seed_from_clock();
                            let gs = GameState::new_with_origin(&o, &world, seed);
                            state.set(Some(gs));
                        },
                        div { class: "origin-name", "{o.name}" }
                        div { class: "origin-intro", "{o.intro}" }
                    }
                }
            }
        }
    }
}

#[component]
fn Console(
    mut state: Signal<Option<GameState>>,
    tuning: Signal<Tuning>,
    jobs: Signal<Vec<sim::job::Job>>,
    modules: Signal<Vec<sim::module::Module>>,
    techs: Signal<Vec<Tech>>,
) -> Element {
    let snapshot = state.read();
    let Some(gs) = snapshot.as_ref() else {
        return rsx! {
            div { class: "loading-placeholder",
                "COLD STAR"
                span { style: "display:block;margin-top:.5rem;width:40px;height:1px;background:var(--line);" }
            }
        };
    };
    let charge = gs.reactor.charge;
    let capacity = gs.reactor.capacity;
    let status = format!("{:?}", gs.reactor.status);
    let is_cold = gs.reactor.status == sim::reactor::ReactorStatus::Cold;
    let salvage_unlocked = gs.flags.contains(&Flag::SalvageUnlocked);
    let source_available = gs.flags.contains(&Flag::CrewSourceAvailable);
    let source_active = gs.flags.contains(&Flag::CrewSourceActive);
    let crew_arrived = gs.flags.contains(&Flag::CrewArrived);
    let scrap = gs.resources.scrap;
    let crew = gs.crew.total;
    let salvage_cooldown = gs.salvage_cooldown;
    let source_name = gs.origin.source_name.clone();
    let crew_noun = gs.origin.crew_noun.clone();
    let log_lines: Vec<(u64, String)> = gs
        .log
        .entries()
        .iter()
        .rev()
        .map(|e| (e.tick, e.text.clone()))
        .collect();
    let economy_unlocked = gs.flags.contains(&Flag::EconomyUnlocked);
    let power_visible = gs.flags.contains(&Flag::PowerBudgetVisible);
    let build_menu_unlocked = gs.flags.contains(&Flag::BuildMenuUnlocked);
    let fuel = gs.resources.fuel;
    let generation = gs.telemetry.generation;
    let active_draw = gs.telemetry.active_draw;
    let res_rows: Vec<(String, f64, f64, f64, bool)> = ResourceKind::ALL
        .iter()
        .filter(|k| gs.resources.amount(**k) > 0.0 || gs.telemetry.net.contains_key(*k))
        .map(|k| {
            let net = gs.telemetry.net.get(k).copied().unwrap_or(0.0);
            let amount = gs.resources.amount(*k);
            let cap = gs.resources.cap;
            let at_cap = cap.is_finite() && amount >= cap;
            (format!("{k:?}"), amount, cap, net, at_cap)
        })
        .collect();
    let idle = gs.crew.idle();
    let starving = gs.telemetry.starving;
    let job_rows: Vec<(sim::job::JobId, String, u32, bool, &'static str)> = jobs
        .read()
        .iter()
        .filter(|j| j.gated_by.map(|g| gs.built_count(g) > 0).unwrap_or(true))
        .map(|j| {
            let count = gs.crew.assigned(j.id);
            let dark = if count > 0
                && gs
                    .telemetry
                    .paused
                    .contains(&sim::priority::Consumer::Job(j.id))
            {
                "power"
            } else if gs.telemetry.starved_input.contains(&j.id) {
                "input"
            } else {
                ""
            };
            (j.id, j.name.clone(), count, count > 0, dark)
        })
        .collect();
    let build_rows: Vec<(sim::module::ModuleId, String, String, String, u32, bool)> = modules
        .read()
        .iter()
        .filter(|m| {
            let prereqs_ok = m.requires_modules.iter().all(|r| gs.built_count(*r) > 0);
            let buildable_again = m.repeatable || gs.built_count(m.id) == 0;
            let flag_ok = m
                .requires_flag
                .map(|f| gs.flags.contains(&f))
                .unwrap_or(true);
            prereqs_ok && buildable_again && flag_ok
        })
        .map(|m| {
            let n = gs.built_count(m.id);
            let scaled_cost: Vec<(ResourceKind, f64)> = m
                .cost
                .iter()
                .map(|(k, a)| (*k, a * m.cost_scaling.powi(n as i32)))
                .collect();
            let cost = scaled_cost
                .iter()
                .map(|(k, a)| format!("{:.0} {k:?}", a))
                .collect::<Vec<_>>()
                .join(", ");
            let affordable = scaled_cost
                .iter()
                .all(|(k, a)| gs.resources.amount(*k) >= *a);
            (
                m.id,
                m.name.clone(),
                m.description.clone(),
                cost,
                n,
                affordable,
            )
        })
        .collect();
    let prio_rows: Vec<(sim::priority::Consumer, String, bool, bool)> = {
        let list = &gs.priority.0;
        let last = list.len().saturating_sub(1);
        list.iter()
            .enumerate()
            .map(|(i, c)| {
                let name = match c {
                    sim::priority::Consumer::Atmosphere => "Atmosphere".to_string(),
                    sim::priority::Consumer::Thermal => "Thermal".to_string(),
                    sim::priority::Consumer::Job(id) => jobs
                        .read()
                        .iter()
                        .find(|j| j.id == *id)
                        .map(|j| j.name.clone())
                        .unwrap_or_else(|| format!("{id:?}")),
                };
                (*c, name, i == 0, i == last)
            })
            .collect()
    };
    let research_unlocked = gs.flags.contains(&Flag::ResearchUnlocked);
    let data_amt = gs.resources.data;
    let research_active: Option<(String, u32, u32)> = gs.research.active.as_ref().map(|a| {
        let total = techs
            .read()
            .iter()
            .find(|t| t.id == a.tech)
            .map(|t| t.research_ticks)
            .unwrap_or(a.ticks_remaining);
        let name = techs
            .read()
            .iter()
            .find(|t| t.id == a.tech)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| format!("{:?}", a.tech));
        (name, a.ticks_remaining, total)
    });
    let codex_unlocked = gs.flags.contains(&Flag::CodexUnlocked);
    let codex_rows: Vec<(String, String)> = gs
        .codex
        .entries
        .iter()
        .rev()
        .map(|d| (d.title.clone(), d.text.clone()))
        .collect();
    // ---- M4 navigation view-model ----
    let shipyard_complete = gs.flags.contains(&Flag::ShipyardComplete);
    let navigation_known = gs.flags.contains(&Flag::NavigationKnown);
    let has_sensor = gs.built_count(sim::module::ModuleId::SensorArray) > 0;
    let away = gs.away();
    let ship_drive = gs.ship.drive_range;
    let ship_cargo_max = gs.ship.cargo;
    let ship_cargo_used = gs.expedition.cargo_total();
    let isotopes = gs.resources.amount(ResourceKind::Isotopes);
    // A textual label of the current activity (or "Docked"/"Holding at <id>").
    let activity_label: String = match &gs.expedition.activity {
        Some(sim::expedition::Activity::Transit { to, ticks }) => {
            format!("In transit to system {} \u{2014} {ticks}s", to.0)
        }
        Some(sim::expedition::Activity::Scan { target, ticks }) => {
            format!("Scanning system {} \u{2014} {ticks}s", target.0)
        }
        Some(sim::expedition::Activity::ResolveSite { system, ticks, .. }) => {
            format!("Working a site in system {} \u{2014} {ticks}s", system.0)
        }
        Some(sim::expedition::Activity::Scavenge { ticks }) => {
            format!("Scavenging fuel \u{2014} {ticks}s")
        }
        None if gs.expedition.location == sim::galaxy::HOME => "Docked at the station".to_string(),
        None => format!("Holding at system {}", gs.expedition.location.0),
    };
    let busy = gs.expedition.activity.is_some();
    let loc = gs.expedition.location;
    // Build a World to regenerate the galaxy for rendering.
    let nav_rows: Vec<NavRow> = if shipyard_complete {
        let t = tuning.read().clone();
        let j = JobRegistry(jobs.read().clone());
        let m = ModuleRegistry(modules.read().clone());
        let tc = TechRegistry(techs.read().clone());
        let world = World {
            tuning: &t,
            jobs: &j,
            modules: &m,
            techs: &tc,
        };
        let galaxy = gs.galaxy(&world);
        let here = galaxy.get(loc).cloned();
        let mut rows: Vec<NavRow> = Vec::new();
        for &sid in gs.galaxy_state.revealed.iter() {
            let id = sim::galaxy::SystemId(sid);
            let Some(sys) = galaxy.get(id) else { continue };
            let dist = here
                .as_ref()
                .map(|h| sim::galaxy::distance(h, sys))
                .unwrap_or(0.0);
            let reachable = id != loc && dist <= ship_drive;
            let cost = sim::galaxy::jump_cost(dist, &t);
            let affordable = gs.resources.fuel >= cost;
            let scanned = gs.galaxy_state.scanned.contains(&sid);
            let sites: Vec<(usize, String, bool, bool)> = gs
                .galaxy_state
                .sites
                .get(&sid)
                .map(|v| {
                    v.iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let landable = matches!(s.kind, sim::galaxy::SiteKind::Planet(_));
                            (i, format!("{:?}", s.kind), s.depleted, landable)
                        })
                        .collect()
                })
                .unwrap_or_default();
            rows.push(NavRow {
                id,
                star: format!("{:?}", sys.star_type),
                pos: sys.pos,
                dist,
                is_current: id == loc,
                reachable,
                affordable,
                scanned,
                cost,
                sites,
            });
        }
        rows.sort_by(|a, b| {
            a.dist
                .partial_cmp(&b.dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rows
    } else {
        Vec::new()
    };
    // Stranded? (mirror engine availability so Scavenge only shows when useful)
    let stranded = away && {
        let cheapest = nav_rows
            .iter()
            .filter(|r| r.reachable)
            .map(|r| r.cost)
            .fold(f64::INFINITY, f64::min);
        cheapest.is_infinite() || gs.resources.fuel < cheapest
    };
    let can_upgrade_drive =
        navigation_known && !away && isotopes >= tuning.read().drive_upgrade_isotopes;
    // Atmosphere warning: the survival clock is running (grace below max) while the
    // Atmosphere module is built and crew exist. Crit when grace is nearly gone.
    let atmo_built = gs
        .built
        .get(&sim::module::ModuleId::Atmosphere)
        .copied()
        .unwrap_or(0)
        > 0;
    let grace = gs.atmosphere_grace;
    let grace_max = tuning.read().atmosphere_grace_max;
    let atmo_warning: Option<(u32, bool)> = if atmo_built && gs.crew.total > 0 && grace < grace_max
    {
        Some((grace, grace * 4 <= grace_max)) // crit in the last quarter of the clock
    } else {
        None
    };
    let research_busy = gs.research.active.is_some();
    let research_rows: Vec<(
        sim::research::TechId,
        String,
        String,
        f64,
        u32,
        &'static str,
        bool,
    )> = techs
        .read()
        .iter()
        .map(|t| {
            let done = gs.research.completed.contains(&t.id);
            let prereqs_met = t.requires.iter().all(|r| gs.research.completed.contains(r));
            let st: &'static str = if done {
                "done"
            } else if prereqs_met && !research_busy {
                "available"
            } else {
                "locked"
            };
            let affordable = data_amt >= t.data_cost;
            (
                t.id,
                t.name.clone(),
                t.blurb.clone(),
                t.data_cost,
                t.research_ticks,
                st,
                affordable,
            )
        })
        .collect();
    let landing_vm: Option<LandingVM> = gs.expedition.landing.as_ref().map(|l| {
        let tn = tuning.read();
        let surface = sim::landing::SurfaceSite::generate(
            sim::landing::surface_seed(gs.world_seed, l.system, l.site_idx),
            l.biome,
            &tn,
        );
        let resolved = gs
            .galaxy_state
            .sites
            .get(&l.system.0)
            .and_then(|v| v.get(l.site_idx))
            .map(|s| s.resolved_nodes.clone())
            .unwrap_or_default();
        let cur = surface.get(l.node).cloned();
        let nodes = surface
            .nodes
            .iter()
            .map(|n| SurfaceNodeVM {
                id: n.id,
                depth: n.depth,
                kind: format!("{:?}", n.kind),
                resolved: resolved.contains(&n.id),
                is_current: n.id == l.node,
                edges: n.edges.clone(),
            })
            .collect();
        LandingVM {
            biome: format!("{:?}", l.biome),
            suit: l.suit,
            suit_max: tn.suit_max,
            satchel: l
                .satchel
                .iter()
                .map(|(k, v)| (format!("{k:?}"), *v))
                .collect(),
            depth: cur.as_ref().map(|n| n.depth).unwrap_or(0),
            encounter: cur
                .as_ref()
                .map(|n| format!("{:?}", n.kind))
                .unwrap_or_default(),
            pending_choice: l.pending_choice,
            traverse: l.traverse,
            nodes,
            away_crew: gs.expedition.away_team.crew,
            squad_hp: gs.expedition.away_team.hp,
            squad_hp_max: gs.expedition.away_team.crew as f64
                * tn.marine_hp
                * (1.0 + gs.gear.armor_tier as f64 * tn.armor_hp_step),
            morale: gs.expedition.away_team.morale,
            morale_max: tn.morale_max,
            combat: l.combat.as_ref().map(|c| CombatVM {
                enemy_hp: c.enemy_hp,
                stance: format!("{:?}", c.stance),
                shield_ticks: c.shield_ticks,
            }),
            negotiation: l.alien.as_ref().map(|a| {
                use sim::alien::{AlienKind, Band};
                let band = sim::alien::band(a.disposition, &tn);
                let is_peer = a.kind == AlienKind::Peer;
                let not_hostile = band != Band::Hostile;
                let tradeable_band = matches!(band, Band::Neutral | Band::Curious);
                let components = gs
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                NegotiationVM {
                    kind: format!("{:?}", a.kind),
                    is_peer,
                    band: format!("{band:?}"),
                    disposition: a.disposition,
                    can_parley: not_hostile,
                    can_trade: is_peer && tradeable_band && components >= tn.trade_give,
                    can_gift: components >= tn.gift_cost,
                    can_observe: !is_peer && not_hostile,
                    can_harvest: !is_peer,
                }
            }),
            consumables: {
                use sim::gear::ConsumableKind::*;
                [
                    (MedGel, "Med-gel"),
                    (Grenade, "Grenade"),
                    (Shield, "Shield"),
                ]
                .iter()
                .map(|(k, label)| {
                    (
                        *k,
                        label.to_string(),
                        gs.consumables.get(k).copied().unwrap_or(0),
                    )
                })
                .collect()
            },
        }
    });
    let armory_built = gs.built_count(sim::module::ModuleId::Armory) > 0;
    let loadout_vm = if armory_built {
        use sim::gear::ConsumableKind::*;
        Some(LoadoutVM {
            away_crew: gs.expedition.away_team.crew,
            idle: gs.crew.idle(),
            weapon_tier: gs.gear.weapon_tier,
            armor_tier: gs.gear.armor_tier,
            consumables: [
                (MedGel, "Med-gel"),
                (Grenade, "Grenade"),
                (Shield, "Shield"),
            ]
            .iter()
            .map(|(k, label)| {
                (
                    *k,
                    label.to_string(),
                    gs.consumables.get(k).copied().unwrap_or(0),
                )
            })
            .collect(),
        })
    } else {
        None
    };
    let contact_vm = gs.expedition.contact.as_ref().map(|c| {
        use sim::contact::ShipKind;
        let tn = tuning.read();
        let in_combat = c.combat.is_some();
        let is_peer = c.kind == ShipKind::Peer;
        let band = sim::alien::band(c.disposition, &tn);
        let not_hostile = band != sim::alien::Band::Hostile;
        let tradeable = matches!(band, sim::alien::Band::Neutral | sim::alien::Band::Curious);
        let components = gs
            .expedition
            .cargo
            .get(&ResourceKind::Components)
            .copied()
            .unwrap_or(0.0);
        // flee cost = jump_cost(distance(here, prev_location)). Galaxy needs only seed + tuning
        // (no full World): `Galaxy::generate(gs.world_seed, tn)`. Reuse an in-scope galaxy binding
        // if Console already built one for the nav rows.
        let flee_cost = {
            let g = sim::galaxy::Galaxy::generate(gs.world_seed, &tn);
            match (
                g.get(gs.expedition.location),
                g.get(gs.expedition.prev_location),
            ) {
                (Some(a), Some(b)) => sim::galaxy::jump_cost(sim::galaxy::distance(a, b), &tn),
                _ => sim::galaxy::jump_cost(1.0, &tn),
            }
        };
        let fuel = gs.resources.amount(ResourceKind::Fuel);
        ContactVM {
            kind: format!("{:?}", c.kind),
            is_peer,
            in_combat,
            band: format!("{band:?}"),
            disposition: c.disposition,
            hull: gs.ship.hull,
            hull_max: gs.ship.hull_max(&tn),
            enemy_hp: c.combat.as_ref().map(|cb| cb.enemy_hp).unwrap_or(0.0),
            stance: c
                .combat
                .as_ref()
                .map(|cb| format!("{:?}", cb.stance))
                .unwrap_or_default(),
            repair_drones: gs.ship.repair_drones,
            can_parley: is_peer && !in_combat && not_hostile,
            can_trade: is_peer && !in_combat && tradeable && components >= tn.trade_give,
            can_gift: is_peer && !in_combat && components >= tn.gift_cost,
            can_avoid: is_peer && !in_combat && not_hostile,
            can_flee: fuel >= flee_cost,
        }
    });
    let munitions_vm = if gs.flags.contains(&sim::flags::Flag::MunitionsKnown)
        && gs.built_count(sim::module::ModuleId::Shipyard) > 0
    {
        let tn = tuning.read();
        // VERIFIED: the three upgrade actions spend via `self.resources.try_spend_all(...)` —
        // BOTH Alloy and Components come from station `gs.resources` (NOT expedition.cargo).
        let alloy = gs.resources.amount(ResourceKind::Alloy);
        let components = gs.resources.amount(ResourceKind::Components);
        Some(ShipMunitionsVM {
            weapon_tier: gs.ship.ship_weapon_tier,
            plating_tier: gs.ship.hull_plating_tier,
            hull: gs.ship.hull,
            hull_max: gs.ship.hull_max(&tn),
            repair_drones: gs.ship.repair_drones,
            can_upgrade_weapon: gs.ship.ship_weapon_tier < 2
                && alloy >= tn.ship_weapon_upgrade_alloy
                && components >= tn.ship_weapon_upgrade_components,
            can_upgrade_plating: gs.ship.hull_plating_tier < 2
                && alloy >= tn.hull_plating_upgrade_alloy
                && components >= tn.hull_plating_upgrade_components,
            can_craft_drone: components >= tn.repair_drone_cost,
        })
    } else {
        None
    };
    let endgame_vm = gs.endgame.as_ref().map(|eg| {
        use sim::Ending::*;
        let copy = |e: sim::Ending| -> (&'static str, &'static str) {
            match e {
                Ascendant => (
                    "Press onward",
                    "consume what remains; become the next thing the dark survives",
                ),
                Communion => (
                    "Stand down",
                    "refuse to consume; the harder, quieter future",
                ),
                PatternBroken => (
                    "Stop here",
                    "leave a living world intact; survival without conquest",
                ),
                Cold => (
                    "Let the fire die",
                    "let the reactor go dark; the cycle pauses here",
                ),
            }
        };
        match eg {
            sim::Endgame::WatchedArrived => {
                let tn = tuning.read();
                EndgameVM {
                    ended: false,
                    choices: gs
                        .available_endings(&tn)
                        .into_iter()
                        .map(|e| {
                            let (l, d) = copy(e);
                            (e, l.to_string(), d.to_string())
                        })
                        .collect(),
                    ending_title: String::new(),
                    ending_body: String::new(),
                }
            }
            sim::Endgame::Ended(e) => {
                use sim::codex::{DiscoveryId, NoteId};
                let nid = match e {
                    Ascendant => NoteId::EndingAscendant,
                    Communion => NoteId::EndingCommunion,
                    PatternBroken => NoteId::EndingPatternBroken,
                    Cold => NoteId::EndingCold,
                };
                let entry = gs
                    .codex
                    .entries
                    .iter()
                    .find(|d| d.id == DiscoveryId::Note(nid));
                EndgameVM {
                    ended: true,
                    choices: Vec::new(),
                    ending_title: entry.map(|d| d.title.clone()).unwrap_or_default(),
                    ending_body: entry.map(|d| d.text.clone()).unwrap_or_default(),
                }
            }
        }
    });
    // Whether away-crew can be changed now (docked only).
    let can_set_crew = !gs.away() && !gs.landed();
    // Word-drift: compute expansion tier from a freshly built World (reads only tuning).
    let tier = {
        let t = tuning.read().clone();
        let j = JobRegistry(jobs.read().clone());
        let m = ModuleRegistry(modules.read().clone());
        let tc = TechRegistry(techs.read().clone());
        let world = World {
            tuning: &t,
            jobs: &j,
            modules: &m,
            techs: &tc,
        };
        gs.expansion_tier(&world)
    };
    let charge_ratio = if capacity > 0.0 {
        (charge / capacity).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (accent, text_col, warm) = emberfall_stops(tier);
    let starving_label = format!(
        "{} starving \u{2014} rations short",
        crew_word(tier, &crew_noun)
    );
    drop(snapshot);

    let mut act = move |action: Action| {
        let t = tuning.read().clone();
        let j = JobRegistry(jobs.read().clone());
        let m = ModuleRegistry(modules.read().clone());
        let tc = TechRegistry(techs.read().clone());
        let world = World {
            tuning: &t,
            jobs: &j,
            modules: &m,
            techs: &tc,
        };
        if let Some(gs) = state.write().as_mut() {
            let _ = gs.apply(&world, action);
        }
    };

    rsx! {
        div {
            class: "console-root",
            style: "--charge: {charge_ratio}; --tier: {tier}; --accent: {accent}; --text: {text_col}; --warm: {warm};",
        if let Some(egvm) = endgame_vm.clone() {
            EndgamePanel {
                vm: egvm,
                on_choose: move |ending| {
                    let t = tuning.read().clone();
                    let j = JobRegistry(jobs.read().clone());
                    let m = ModuleRegistry(modules.read().clone());
                    let tc = TechRegistry(techs.read().clone());
                    let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                    if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::ChooseEnding(ending)); }
                },
                on_restart: move |_| { state.set(None); },
            }
        } else {
        // Core rail — pinned heartbeat bar. ALWAYS visible (charge + status),
        // independent of disclosure. NOT inside a filter: keeps position:sticky working.
        div { class: "rail",
            div { class: "rail-in",
                div {
                    div { class: "core-num",
                        b { "{charge:.0}" }
                        span { class: "core-cap", " / {capacity:.0}" }
                    }
                    div { class: "core-meta",
                        span { class: "dot" }
                        span { "Reactor core" }
                        if shipyard_complete {
                            span { "\u{00b7} ship \u{00b7} drive {ship_drive:.0} \u{00b7} cargo {ship_cargo_used:.0}/{ship_cargo_max:.0} \u{00b7} iso {isotopes:.0}" }
                        }
                    }
                }
                div { class: "status status-{status}", "{status}" }
                div { class: "core-bar", i {} }
            }
        }

        // Life veil — siblings of .rail, filtered together. .rail stays outside.
        div { class: "life-veil",

        if let Some((secs, crit)) = atmo_warning {
            div {
                class: if crit { "ls-warn crit" } else { "ls-warn" },
                if secs > 0 { "ATMOSPHERE FAILING \u{2014} {secs}s of air" } else { "ATMOSPHERE GONE \u{2014} losing crew" }
            }
        }

        // Core action buttons — always rendered, but dimmed/disabled while away
        // (they manage the station, which the engine refuses while exploring).
        div { class: if away { "dimmed" } else { "" },
            if is_cold {
                button { class: "act", onclick: move |_| act(Action::Ignite), "Ignite the core" }
            } else {
                if salvage_unlocked {
                    button {
                        class: "act",
                        disabled: salvage_cooldown > 0,
                        onclick: move |_| act(Action::Salvage),
                        if salvage_cooldown > 0 { "{drift_word(DriftSlot::Salvage, tier)} ({salvage_cooldown}s)" } else { "{drift_word(DriftSlot::Salvage, tier)}" }
                    }
                    button { class: "act", onclick: move |_| act(Action::FeedCore), "{drift_word(DriftSlot::FeedCore, tier)}" }
                }
                if source_available && !source_active {
                    button {
                        class: "act",
                        onclick: move |_| act(Action::ActivateCrewSource),
                        "Activate {source_name}"
                    }
                }
            }
        }
        button {
            class: "act ghost",
            onclick: move |_| {
                if let Some(gs) = state.read().as_ref() { crate::persist::copy_link(gs); }
            },
            "Copy save link"
        }

        if away {
            div { class: "away-banner", "AWAY \u{2014} STATION UNATTENDED \u{00b7} the core runs on without you" }
        }
        if shipyard_complete {
            div { class: "nav-wrap",
                if let Some(vm) = landing_vm.clone() {
                    SurfaceView {
                        vm,
                        tier,
                        on_move: move |node| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::MoveToNode(node)); }
                        },
                        on_harvest: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HarvestLife); }
                        },
                        on_leave: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::LeaveLife); }
                        },
                        on_extract: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Extract); }
                        },
                        on_use_consumable: move |kind| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UseConsumable(kind)); }
                        },
                        on_stance: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() {
                                let next = match gs.expedition.landing.as_ref().and_then(|l| l.combat.as_ref()).map(|c| c.stance) {
                                    Some(sim::combat::Stance::Aggressive) => sim::combat::Stance::Defensive,
                                    _ => sim::combat::Stance::Aggressive,
                                };
                                let _ = gs.apply(&world, Action::SetStance(next));
                            }
                        },
                        on_flee: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Flee); }
                        },
                        on_parley: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Parley); }
                        },
                        on_trade: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Trade); }
                        },
                        on_gift: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Gift); }
                        },
                        on_avoid: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Avoid); }
                        },
                        on_observe: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Observe); }
                        },
                        on_harvest_fauna: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Harvest); }
                        },
                        on_attack_alien: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::AttackAlien); }
                        },
                    }
                } else if let Some(cvm) = contact_vm.clone() {
                    ContactPanel {
                        vm: cvm,
                        on_parley: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HailParley); }
                        },
                        on_trade: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HailTrade); }
                        },
                        on_gift: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HailGift); }
                        },
                        on_avoid: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::HailAvoid); }
                        },
                        on_attack: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::AttackShip); }
                        },
                        on_flee: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::ShipFlee); }
                        },
                        on_stance: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() {
                                let next = match gs.expedition.contact.as_ref().and_then(|c| c.combat.as_ref()).map(|c| c.stance) {
                                    Some(sim::combat::Stance::Aggressive) => sim::combat::Stance::Defensive,
                                    _ => sim::combat::Stance::Aggressive,
                                };
                                let _ = gs.apply(&world, Action::SetShipStance(next));
                            }
                        },
                        on_repair: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UseRepairDrone); }
                        },
                    }
                } else {
                    NavigationPanel {
                        drive: ship_drive,
                        cargo_used: ship_cargo_used,
                        cargo_max: ship_cargo_max,
                        fuel,
                        isotopes,
                        activity_label: activity_label.clone(),
                        busy,
                        away,
                        has_sensor,
                        stranded,
                        can_upgrade_drive,
                        current: loc,
                        rows: nav_rows.clone(),
                        on_scan: move |id| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::ScanSystem(id)); }
                        },
                        on_jump: move |id| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Jump(id)); }
                        },
                        on_resolve: move |(id, idx)| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::ResolveSite(id, idx)); }
                        },
                        on_scavenge: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Scavenge); }
                        },
                        on_upgrade: move |_| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeDrive); }
                        },
                        on_land: move |idx| {
                            let t = tuning.read().clone();
                            let j = JobRegistry(jobs.read().clone());
                            let m = ModuleRegistry(modules.read().clone());
                            let tc = TechRegistry(techs.read().clone());
                            let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                            if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::Land(idx)); }
                        },
                    }
                }
            }
        }
        div { class: "content-grid",
        div { class: "content-main",
        if build_menu_unlocked || economy_unlocked || power_visible || salvage_unlocked {
            // Two-column dashboard — panels appear via per-panel disclosure flags (§5).
            div { class: "dash",
                div { class: if away { "col dimmed" } else { "col" },
                    if power_visible {
                        PowerMeter {
                            status: status.clone(),
                            charge,
                            capacity,
                            fuel,
                            generation,
                            active_draw,
                        }
                    }
                    if salvage_unlocked {
                        ResourcesPanel { rows: res_rows.clone() }
                    }
                    if codex_unlocked {
                        CodexPanel { entries: codex_rows.clone() }
                    }
                }
                div { class: if away { "col dimmed" } else { "col" },
                    if economy_unlocked {
                        JobPanel {
                            idle,
                            starving,
                            starving_label: starving_label.clone(),
                            rows: job_rows.clone(),
                            on_assign: move |(job, delta)| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() {
                                    let _ = gs.apply(&world, Action::AssignCrew { job, delta });
                                }
                            },
                        }
                    }
                    if build_menu_unlocked {
                        BuildMenu {
                            rows: build_rows.clone(),
                            on_build: move |id| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() {
                                    let _ = gs.apply(&world, Action::Build(id));
                                }
                            },
                        }
                    }
                    if research_unlocked {
                        ResearchPanel {
                            active: research_active.clone(),
                            rows: research_rows.clone(),
                            on_start: move |id| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() {
                                    let _ = gs.apply(&world, Action::StartResearch(id));
                                }
                            },
                        }
                    }
                    if power_visible {
                        PriorityPanel {
                            rows: prio_rows.clone(),
                            on_move: move |(consumer, up)| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() {
                                    let _ = gs.apply(&world, Action::SetPriority { consumer, up });
                                }
                            },
                        }
                    }
                    if let Some(vm) = loadout_vm.clone() {
                        LoadoutPanel {
                            vm,
                            tier,
                            can_set_crew,
                            on_set_crew: move |delta| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::SetAwayCrew { delta }); }
                            },
                            on_upgrade_weapon: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeWeapon); }
                            },
                            on_upgrade_armor: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeArmor); }
                            },
                            on_craft: move |kind| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::CraftConsumable(kind)); }
                            },
                        }
                    }
                    if let Some(vm) = munitions_vm.clone() {
                        ShipMunitionsPanel {
                            vm,
                            on_upgrade_weapon: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeShipWeapon); }
                            },
                            on_upgrade_plating: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::UpgradeHullPlating); }
                            },
                            on_craft_drone: move |_| {
                                let t = tuning.read().clone();
                                let j = JobRegistry(jobs.read().clone());
                                let m = ModuleRegistry(modules.read().clone());
                                let tc = TechRegistry(techs.read().clone());
                                let world = World { tuning: &t, jobs: &j, modules: &m, techs: &tc };
                                if let Some(gs) = state.write().as_mut() { let _ = gs.apply(&world, Action::CraftRepairDrone); }
                            },
                        }
                    }
                }
            }
        } else {
            // Reactor readout + buttons are rendered above; this branch is the
            // bare opening (nothing disclosed yet) and adds the intro res line.
            if salvage_unlocked {
                div { class: "res",
                    "Scrap: {scrap:.0}"
                    if crew_arrived { "   \u{00b7}   Crew: {crew}" }
                }
            }
        }
        } // content-main
        // Log rail — persistent right-hand column on wide screens, below on mobile
        div { class: "log-rail",
            div { class: "panel",
                div { class: "eyebrow", "LOG" }
                div { class: "log",
                    for (tick, text) in log_lines {
                        div { class: "log-line", "[{tick}] {text}" }
                    }
                }
            }
        }
        } // content-grid
        } // life-veil
        } // else (no endgame)
        } // console-root
    }
}
