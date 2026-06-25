use dioxus::prelude::*;
use sim::Tuning;
use sim::flags::Flag;
use sim::job::JobRegistry;
use sim::module::ModuleRegistry;
use sim::origin::OriginDef;
use sim::research::{Tech, TechRegistry};
use sim::resources::ResourceKind;
use sim::state::{Action, GameState, World};

#[derive(Clone, PartialEq)]
struct NavRow {
    id: sim::galaxy::SystemId,
    star: String,
    pos: (f64, f64),
    dist: f64,
    is_current: bool,
    reachable: bool,
    affordable: bool,
    scanned: bool,
    cost: f64,
    // (site_index, kind label, depleted, landable)
    sites: Vec<(usize, String, bool, bool)>,
}

#[derive(Clone, PartialEq)]
struct SurfaceNodeVM {
    id: u32,
    depth: u32,
    kind: String,
    resolved: bool,
    is_current: bool,
    edges: Vec<u32>,
}

#[derive(Clone, PartialEq)]
struct CombatVM {
    enemy_hp: f64,
    stance: String,
    shield_ticks: u32,
}

#[derive(Clone, PartialEq)]
struct NegotiationVM {
    kind: String, // "Fauna" | "Peer"
    is_peer: bool,
    band: String, // "Hostile" | "Fearful" | "Neutral" | "Curious"
    disposition: f64,
    // disable predicates — exact duals of the engine's apply() rejections
    can_parley: bool,
    can_trade: bool,
    can_gift: bool,
    can_observe: bool,
    can_harvest: bool,
}

#[derive(Clone, PartialEq)]
struct LandingVM {
    biome: String,
    suit: f64,
    suit_max: f64,
    satchel: Vec<(String, f64)>,
    node: u32,
    depth: u32,
    encounter: String,
    pending_choice: bool,
    traverse: u32,
    neighbors: Vec<(u32, String, bool)>,
    nodes: Vec<SurfaceNodeVM>,
    away_crew: u32,
    squad_hp: f64,
    squad_hp_max: f64,
    morale: f64,
    morale_max: f64,
    combat: Option<CombatVM>,
    negotiation: Option<NegotiationVM>,
    consumables: Vec<(sim::gear::ConsumableKind, String, u32)>,
}

#[derive(Clone, PartialEq)]
struct LoadoutVM {
    away_crew: u32,
    idle: u32,
    weapon_tier: u8,
    armor_tier: u8,
    // (kind, label, count) for each consumable kind
    consumables: Vec<(sim::gear::ConsumableKind, String, u32)>,
}

#[derive(Clone, PartialEq)]
struct ContactVM {
    kind: String, // "Peer" | "Remnant"
    is_peer: bool,
    in_combat: bool,
    band: String, // "Hostile" | "Fearful" | "Neutral" | "Curious"
    disposition: f64,
    hull: f64,
    hull_max: f64,
    enemy_hp: f64,
    stance: String,
    repair_drones: u32,
    // disable predicates — exact duals of the engine rejections
    can_parley: bool,
    can_trade: bool,
    can_gift: bool,
    can_avoid: bool,
    can_flee: bool,
}

#[derive(Clone, PartialEq)]
struct ShipMunitionsVM {
    weapon_tier: u8,
    plating_tier: u8,
    hull: f64,
    hull_max: f64,
    repair_drones: u32,
    can_upgrade_weapon: bool,
    can_upgrade_plating: bool,
    can_craft_drone: bool,
}

const CSS: &str = "
:root { color-scheme: dark; }
body { margin: 0; background: #05070a; color: #c7d3dd;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
.console { max-width: 620px; margin: 7vh auto; padding: 2rem;
  border: 1px solid #16313f; border-radius: 6px; background: #070b10;
  box-shadow: 0 0 40px rgba(40,120,160,.08); }
.title { letter-spacing: .4em; font-weight: 600; color: #6fb7d6; margin: 0 0 1.5rem; }
.readout { font-size: .72rem; letter-spacing: .25em; color: #5b7686; }
.charge { font-size: 2.4rem; margin: .2rem 0; color: #d8e6ef; }
.status { font-size: .8rem; letter-spacing: .2em; }
.status-Stable { color: #6fd6a0; }
.status-Igniting { color: #d6c46f; }
.status-Failing { color: #e0a24a; }
.status-Cold { color: #d66f6f; }
.res { margin-top: .8rem; font-size: .9rem; color: #9fb3c2; }
.act { margin-top: 1.2rem; margin-right: .6rem; padding: .6rem 1.1rem;
  background: #0c1822; color: #c7d3dd; border: 1px solid #1d4456;
  border-radius: 4px; font-family: inherit; cursor: pointer; }
.act:hover:not(:disabled) { background: #112433; }
.act:disabled { opacity: .35; cursor: not-allowed; }
.act.ghost { opacity: .7; }
.origins { display: flex; gap: 1rem; flex-wrap: wrap; }
.origin-card { flex: 1 1 160px; padding: 1rem; border: 1px solid #1d4456;
  border-radius: 5px; background: #0a141c; cursor: pointer; }
.origin-card:hover { background: #112433; border-color: #2f6a80; }
.origin-name { color: #6fb7d6; letter-spacing: .15em; margin-bottom: .5rem; }
.origin-intro { font-size: .82rem; color: #9fb3c2; line-height: 1.4; }
.log { margin-top: 1.5rem; border-top: 1px solid #16313f; padding-top: .8rem;
  max-height: 220px; overflow-y: auto; font-size: .82rem; line-height: 1.5; }
.log-line { color: #8aa0b0; }
.dash { display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem; }
@media (max-width: 640px) { .dash { grid-template-columns: 1fr; } }
.col { display: flex; flex-direction: column; gap: 1.2rem; }
.panel { border: 1px solid #16313f; border-radius: 5px; padding: .9rem 1rem; background: #0a141c; }
.panel-title { font-size: .68rem; letter-spacing: .25em; color: #5b7686; margin-bottom: .6rem; }
.bar { height: 6px; background: #0c1822; border-radius: 3px; overflow: hidden; margin: .3rem 0; }
.bar > span { display: block; height: 100%; }
.bar-gen > span { background: #6fb7d6; }
.bar-draw > span { background: #e0a24a; }
.bar-charge > span { background: #6fd6a0; }
.res-row { display: flex; justify-content: space-between; font-size: .85rem; padding: .15rem 0; }
.res-net-up { color: #6fd6a0; }
.res-net-down { color: #d66f6f; }
.res-full { color: #d6c46f; }
.status-Straining { color: #e08a4a; }
.status-Critical { color: #d66f6f; font-weight: 600; }
.muted { color: #5b7686; font-size: .8rem; }
.job-row, .build-row, .prio-row { display: flex; justify-content: space-between; align-items: center; font-size: .85rem; padding: .25rem 0; gap: .5rem; }
.mini { padding: .15rem .5rem; min-width: 1.6rem; background: #0c1822; color: #c7d3dd; border: 1px solid #1d4456; border-radius: 3px; font-family: inherit; cursor: pointer; }
.mini:hover:not(:disabled) { background: #112433; }
.mini:disabled { opacity: .3; cursor: not-allowed; }
.badge { font-size: .7rem; padding: .05rem .4rem; border-radius: 3px; margin-left: .4rem; }
.badge-power { color: #e0a24a; border: 1px solid #5a4326; }
.badge-input { color: #6f9fd6; border: 1px solid #264a5a; }
.idle { color: #6fd6a0; font-size: .8rem; margin-bottom: .5rem; }
.starving { color: #d66f6f; font-size: .8rem; margin-bottom: .5rem; }
.cost { color: #9fb3c2; font-size: .78rem; }
.info { font-style: italic; min-width: 1.6rem; color: #6fb7d6; }
.build-desc { font-size: .78rem; color: #9fb3c2; line-height: 1.45;
  border-left: 2px solid #1d4456; padding: .1rem 0 .4rem .6rem; margin: 0 0 .4rem; }
.research-row { display: flex; justify-content: space-between; align-items: center; font-size: .85rem; padding: .25rem 0; gap: .5rem; }
.research-name { color: #c7d3dd; }
.research-done { color: #6fd6a0; }
.research-locked { color: #5b7686; }
.research-blurb { color: #9fb3c2; font-size: .76rem; }
.research-cost { color: #9fb3c2; font-size: .76rem; }
.research-active { border-left: 2px solid #2f6a80; padding-left: .6rem; margin: .3rem 0; }
.rbar { height: 6px; background: #0c1822; border-radius: 3px; overflow: hidden; margin: .3rem 0; }
.rbar > span { display: block; height: 100%; background: #6fb7d6; }
.codex-row { font-size: .84rem; padding: .2rem 0; color: #8aa0b0; cursor: pointer; }
.codex-row:hover { color: #c7d3dd; }
.codex-text { font-size: .8rem; color: #9fb3c2; line-height: 1.5; border-left: 2px solid #1d4456; padding: .2rem 0 .4rem .6rem; margin: 0 0 .4rem; }
.ls-warn { margin: .8rem 0; padding: .5rem .8rem; border: 1px solid #5a2626; border-radius: 4px;
  color: #e0a24a; font-size: .82rem; letter-spacing: .1em; }
.ls-warn.crit { color: #d66f6f; border-color: #7a2a2a; }
.nav-activity { font-size: .82rem; color: #6fb7d6; letter-spacing: .08em; margin: .3rem 0 .5rem; }
.nav-row { display: flex; justify-content: space-between; align-items: center; font-size: .85rem; padding: .2rem 0; gap: .5rem; }
.nav-site { display: flex; justify-content: space-between; align-items: center; font-size: .8rem; color: #9fb3c2; padding: .1rem 0 .1rem 1rem; }
.nav-here { color: #6fd6a0; }
.away-banner { margin: .8rem 0; padding: .5rem .8rem; border: 1px solid #5a4326; border-radius: 4px;
  color: #e0a24a; font-size: .82rem; letter-spacing: .12em; background: #140d05; }
.nav-wrap { margin-bottom: 1.2rem; }
.dimmed { opacity: .4; pointer-events: none; filter: grayscale(.3); transition: opacity .3s; }
.navmap { width: 100%; height: 240px; display: block; margin: .4rem 0 .6rem;
  background: radial-gradient(circle at 50% 50%, #0a141c 0%, #070b10 70%); border: 1px solid #16313f; border-radius: 5px; }
.range-ring { fill: rgba(111,183,214,.05); stroke: #2f6a80; stroke-width: .5; stroke-dasharray: 2 2; }
.nav-edge { stroke: #16313f; stroke-width: .4; }
.sys { cursor: pointer; stroke: #05070a; stroke-width: .6; }
.sys-current { fill: #6fd6a0; }
.sys-reachable { fill: #6fb7d6; }
.sys-far { fill: #3a4a55; }
.sys-selected { stroke: #d8e6ef; stroke-width: 1.2; }
.sys-home { fill: #e8d28a; stroke: #d6c46f; stroke-width: 1; }
.sys-home-ring { fill: none; stroke: #d6c46f; stroke-width: .6; opacity: .55; }
.sys-home-label { fill: #d6c46f; font-size: 6px; letter-spacing: .12em; }
.star-Neutron { }
.star-Dead { }
.navmap circle.sys:hover { stroke: #9fd0e6; stroke-width: 1.2; }
.nav-detail { border-top: 1px solid #16313f; padding-top: .4rem; }
.surface { border-color: #2f5a40; }
.bar-suit > span { background: #6fd6a0; }
.surf-here { font-size: .85rem; color: #c7d3dd; margin: .3rem 0; }
.surf-choice { border-left: 2px solid #5a4a26; padding-left: .6rem; margin: .4rem 0; }
.surfmap-scroll { width: 100%; max-height: 300px; overflow: auto; margin: .4rem 0;
  background: radial-gradient(circle at 50% 50%, #0a1810 0%, #070b08 70%); border: 1px solid #1d4a30; border-radius: 5px; }
.surfmap { display: block; }
.surf-edge { stroke: #234; stroke-width: .6; }
.surf-node { cursor: pointer; stroke: #05070a; stroke-width: .6; }
.surf-current { fill: #6fd6a0; }
.surf-reach { fill: #6fb7d6; }
.surf-done { fill: #2a3a30; }
.surf-far { fill: #3a4a44; }
.kind-Hazard { stroke: #e0a24a; stroke-width: 1; }
.combat { border: 1px solid #5a2626; border-radius: 5px; padding: .6rem .8rem; margin: .4rem 0; background: #140808; }
.bar-enemy > span { background: #d66f6f; }
.bar-hp > span { background: #6fd6a0; }
.bar-morale > span { background: #6f9fd6; }
.combat-actions { display: flex; flex-wrap: wrap; gap: .4rem; margin-top: .5rem; align-items: center; }
.neg { margin-top: .6rem; border-top: 1px solid #16313f; padding-top: .6rem; }
.neg-band { font-size: .8rem; letter-spacing: .18em; }
.band-Hostile { color: #d66f6f; }
.band-Fearful { color: #e0a24a; }
.band-Neutral { color: #c7d3dd; }
.band-Curious { color: #6fd6a0; }
.bar-disp > span { background: #9b7fd6; }
.neg-actions { margin-top: .6rem; display: flex; flex-wrap: wrap; gap: .4rem; }
.contact { border: 1px solid #3a2630; }
.contact-actions { margin-top: .6rem; display: flex; flex-wrap: wrap; gap: .4rem; }
.bar-hull > span { background: #6fb7d6; }
.bar-enemy > span { background: #d66f6f; }
.munitions { margin-top: 1rem; }
.endgame { border-color: #5b3a4a; max-width: 560px; margin: 1rem auto; }
.endgame-body { white-space: pre-wrap; line-height: 1.6; color: #c7d3dd; margin: .6rem 0 1rem; }
.endgame-complete { letter-spacing: .25em; margin-bottom: 1rem; }
.endgame-choices { display: flex; flex-direction: column; gap: .6rem; margin-top: .8rem; }
.endgame-choice { text-align: left; }
.endgame-choice-label { color: #d8e6ef; letter-spacing: .1em; }
";

/// Word-drift slots whose label cools as expansion rises (cold vocab shared across origins).
#[derive(Clone, Copy)]
enum DriftSlot {
    AwayTeam,
    Take,
    Salvage,
    FeedCore,
}

/// The cold-vocabulary table: [T0, T1, T2, T3] per slot.
fn drift_word(slot: DriftSlot, tier: u8) -> &'static str {
    let row: [&str; 4] = match slot {
        DriftSlot::AwayTeam => ["Marines", "the team", "the detail", "the asset"],
        DriftSlot::Take => ["Harvest", "Extract", "Process", "Strip"],
        DriftSlot::Salvage => ["Salvage", "Recover", "Reclaim", "Render"],
        DriftSlot::FeedCore => [
            "Feed the core",
            "Fuel the core",
            "Charge the core",
            "Burn it",
        ],
    };
    row[(tier as usize).min(3)]
}

/// The crew noun: the origin's warm word at T0, then shared cold words.
fn crew_word(tier: u8, origin_noun: &str) -> String {
    match tier {
        0 => origin_noun.to_string(),
        1 => "crew".into(),
        2 => "units".into(),
        _ => "yield".into(),
    }
}

#[component]
fn PowerMeter(
    status: String,
    charge: f64,
    capacity: f64,
    fuel: f64,
    generation: f64,
    active_draw: f64,
) -> Element {
    let cap = if capacity > 0.0 { capacity } else { 1.0 };
    let charge_pct = (charge / cap * 100.0).clamp(0.0, 100.0);
    let scale = generation.max(active_draw).max(1.0);
    let gen_pct = (generation / scale * 100.0).clamp(0.0, 100.0);
    let draw_pct = (active_draw / scale * 100.0).clamp(0.0, 100.0);
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "REACTOR" }
            div { class: "status status-{status}", "{status}" }
            div { class: "muted", "Output {generation:.1}  \u{00b7}  Draw {active_draw:.1}" }
            div { class: "bar bar-gen", span { style: "width: {gen_pct}%" } }
            div { class: "bar bar-draw", span { style: "width: {draw_pct}%" } }
            div { class: "muted", "Charge {charge:.0} / {capacity:.0}  \u{00b7}  Fuel {fuel:.0}" }
            div { class: "bar bar-charge", span { style: "width: {charge_pct}%" } }
        }
    }
}

#[component]
fn ResourcesPanel(rows: Vec<(String, f64, f64, f64, bool)>) -> Element {
    // rows: (name, amount, cap, net, at_cap)
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "RESOURCES" }
            for (name, amount, cap, net, at_cap) in rows {
                div { class: "res-row",
                    span {
                        class: if at_cap { "res-full" } else { "" },
                        "{name}"
                        if at_cap { span { class: "res-full", " (full)" } }
                    }
                    span {
                        "{amount:.0} / {cap:.0}  "
                        if net > 0.0 {
                            span { class: "res-net-up", "\u{25b2}{net:.1}" }
                        } else if net < 0.0 {
                            span { class: "res-net-down", "\u{25bc}{(-net):.1}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn JobPanel(
    idle: u32,
    starving: bool,
    starving_label: String,
    rows: Vec<(sim::job::JobId, String, u32, bool, &'static str)>,
    on_assign: EventHandler<(sim::job::JobId, i32)>,
) -> Element {
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "CREW" }
            div { class: "idle", "Idle: {idle}" }
            if starving { div { class: "starving", "{starving_label}" } }
            for (id, name, count, has_crew, dark) in rows {
                div { class: "job-row",
                    span {
                        "{name} \u{d7}{count}"
                        if has_crew && dark == "power" { span { class: "badge badge-power", "\u{23f8} no power" } }
                        if has_crew && dark == "input" { span { class: "badge badge-input", "\u{25bd} no input" } }
                    }
                    span {
                        button {
                            class: "mini",
                            disabled: count == 0,
                            onclick: move |_| on_assign.call((id, -1)),
                            "\u{2212}"
                        }
                        button {
                            class: "mini",
                            disabled: idle == 0,
                            onclick: move |_| on_assign.call((id, 1)),
                            "+"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn BuildMenu(
    rows: Vec<(sim::module::ModuleId, String, String, String, u32, bool)>,
    on_build: EventHandler<sim::module::ModuleId>,
) -> Element {
    // Which module's narrative info line is currently expanded (toggled by its
    // (i) button). None = all collapsed.
    let mut info_open = use_signal(|| None::<sim::module::ModuleId>);
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "BUILD" }
            for (id, name, description, cost, n, affordable) in rows {
                div { class: "build-row",
                    span {
                        "{name}"
                        if n > 0 { span { class: "muted", " ({n})" } }
                        div { class: "cost", "{cost}" }
                    }
                    span {
                        button {
                            class: "mini info",
                            title: "What is this?",
                            onclick: move |_| {
                                let next = if info_open() == Some(id) { None } else { Some(id) };
                                info_open.set(next);
                            },
                            "i"
                        }
                        button {
                            class: "mini",
                            disabled: !affordable,
                            onclick: move |_| on_build.call(id),
                            "build"
                        }
                        if !affordable { span { class: "muted", " (can't afford)" } }
                    }
                }
                if info_open() == Some(id) {
                    div { class: "build-desc", "{description}" }
                }
            }
        }
    }
}

#[component]
fn PriorityPanel(
    rows: Vec<(sim::priority::Consumer, String, bool, bool)>,
    on_move: EventHandler<(sim::priority::Consumer, bool)>,
) -> Element {
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "POWER PRIORITY" }
            div { class: "muted", "higher = browns out last" }
            for (c, name, is_first, is_last) in rows {
                div { class: "prio-row",
                    span { "{name}" }
                    span {
                        button { class: "mini", disabled: is_first, onclick: move |_| on_move.call((c, true)), "\u{25b2}" }
                        button { class: "mini", disabled: is_last, onclick: move |_| on_move.call((c, false)), "\u{25bc}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ResearchPanel(
    // active: (name, ticks_remaining, ticks_total) when something is researching
    active: Option<(String, u32, u32)>,
    // rows: (id, name, blurb, data_cost, research_ticks, state, affordable) where state is
    // "done" | "available" | "locked"; available rows are startable iff affordable
    rows: Vec<(
        sim::research::TechId,
        String,
        String,
        f64,
        u32,
        &'static str,
        bool,
    )>,
    on_start: EventHandler<sim::research::TechId>,
) -> Element {
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "RESEARCH" }
            if let Some((name, rem, total)) = active {
                {
                    let done = total.saturating_sub(rem);
                    let pct = if total > 0 { (done as f64 / total as f64 * 100.0).clamp(0.0, 100.0) } else { 0.0 };
                    rsx! {
                        div { class: "research-active",
                            div { class: "research-name", "Researching: {name}  ({rem}s left)" }
                            div { class: "rbar", span { style: "width: {pct}%" } }
                        }
                    }
                }
            }
            for (id, name, blurb, cost, ticks, st, affordable) in rows {
                div { class: "research-row",
                    span {
                        span {
                            class: match st { "done" => "research-done", "locked" => "research-locked", _ => "research-name" },
                            "{name}"
                        }
                        div { class: "research-blurb", "{blurb}" }
                        if st == "available" {
                            div { class: "research-cost", "{cost:.0} Data  \u{00b7}  {ticks}s" }
                        }
                    }
                    span {
                        if st == "done" {
                            span { class: "research-done", "\u{2713}" }
                        } else if st == "available" {
                            button {
                                class: "mini",
                                disabled: !affordable,
                                onclick: move |_| on_start.call(id),
                                "research"
                            }
                        } else {
                            span { class: "research-locked", "locked" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CodexPanel(entries: Vec<(String, String)>) -> Element {
    // entries: (title, text). Click a title to expand its text inline.
    let mut open = use_signal(|| None::<usize>);
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "CODEX" }
            for (i, (title, text)) in entries.into_iter().enumerate() {
                div {
                    class: "codex-row",
                    onclick: move |_| {
                        let next = if open() == Some(i) { None } else { Some(i) };
                        open.set(next);
                    },
                    "{title}"
                }
                if open() == Some(i) {
                    div { class: "codex-text", "{text}" }
                }
            }
        }
    }
}

#[component]
fn NavigationPanel(
    drive: f64,
    cargo_used: f64,
    cargo_max: f64,
    fuel: f64,
    isotopes: f64,
    activity_label: String,
    busy: bool,
    away: bool,
    has_sensor: bool,
    stranded: bool,
    can_upgrade_drive: bool,
    current: sim::galaxy::SystemId,
    rows: Vec<NavRow>,
    on_scan: EventHandler<sim::galaxy::SystemId>,
    on_jump: EventHandler<sim::galaxy::SystemId>,
    on_resolve: EventHandler<(sim::galaxy::SystemId, usize)>,
    on_scavenge: EventHandler<()>,
    on_upgrade: EventHandler<()>,
    on_land: EventHandler<usize>,
) -> Element {
    let mut selected = use_signal(|| current);
    // Center on the current system's position.
    let center = rows
        .iter()
        .find(|r| r.is_current)
        .map(|r| r.pos)
        .unwrap_or((0.0, 0.0));
    // viewBox half-extent: fit the drive ring + a margin (clamped so a huge drive still renders).
    let half = (drive * 1.25).clamp(30.0, 140.0);
    let view = format!("{} {} {} {}", -half, -half, 2.0 * half, 2.0 * half);
    let sel = selected();
    let sel_row = rows.iter().find(|r| r.id == sel).cloned();
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "NAVIGATION" }
            div { class: "muted",
                "Drive {drive:.0}  \u{00b7}  Cargo {cargo_used:.0}/{cargo_max:.0}  \u{00b7}  Fuel {fuel:.0}  \u{00b7}  Isotopes {isotopes:.0}"
            }
            div { class: "nav-activity", "{activity_label}" }
            if stranded {
                button { class: "act", onclick: move |_| on_scavenge.call(()), "Scavenge fuel" }
            }
            if can_upgrade_drive {
                button { class: "act", onclick: move |_| on_upgrade.call(()), "Recalibrate drive (+range)" }
            }
            // SVG star map.
            svg {
                class: "navmap",
                view_box: "{view}",
                preserve_aspect_ratio: "xMidYMid meet",
                // range ring
                circle { cx: 0, cy: 0, r: "{drive}", class: "range-ring" }
                // edges from current to reachable systems (subtle)
                for row in rows.iter().filter(|r| r.reachable) {
                    line {
                        x1: 0, y1: 0,
                        x2: "{row.pos.0 - center.0}", y2: "{row.pos.1 - center.1}",
                        class: "nav-edge",
                    }
                }
                // system dots
                for row in rows.clone() {
                    {
                        let id = row.id;
                        let is_home = row.id.0 == 0;
                        let dot_class = {
                            let mut c = format!("sys star-{}", row.star);
                            if row.is_current { c.push_str(" sys-current"); }
                            else if row.reachable { c.push_str(" sys-reachable"); }
                            else { c.push_str(" sys-far"); }
                            if is_home { c.push_str(" sys-home"); }
                            if sel == row.id { c.push_str(" sys-selected"); }
                            c
                        };
                        let r_val = if is_home { "5.5" } else if row.is_current { "4.5" } else { "3.2" };
                        let cx_val = format!("{}", row.pos.0 - center.0);
                        let cy_val = format!("{}", row.pos.1 - center.1);
                        let label_y = format!("{}", row.pos.1 - center.1 + 13.0);
                        rsx! {
                            // The home station gets a halo + label so it always reads as "home".
                            if is_home {
                                circle { cx: "{cx_val}", cy: "{cy_val}", r: "9", class: "sys-home-ring" }
                            }
                            circle {
                                cx: "{cx_val}",
                                cy: "{cy_val}",
                                r: "{r_val}",
                                class: "{dot_class}",
                                onclick: move |_| selected.set(id),
                            }
                            if is_home {
                                text {
                                    x: "{cx_val}",
                                    y: "{label_y}",
                                    text_anchor: "middle",
                                    class: "sys-home-label",
                                    "HOME"
                                }
                            }
                        }
                    }
                }
            }
            // Detail + actions for the selected system.
            if let Some(row) = sel_row {
                div { class: "nav-detail",
                    div { class: "nav-row",
                        span {
                            if row.is_current { span { class: "nav-here", "\u{25c9} " } }
                            "Sys {row.id.0} \u{00b7} {row.star} \u{00b7} {row.dist:.0} ly"
                            if row.scanned { span { class: "muted", " \u{00b7} scanned" } } else { span { class: "muted", " \u{00b7} unscanned" } }
                        }
                        span {
                            if !row.is_current && has_sensor && !row.scanned {
                                button { class: "mini", disabled: busy, onclick: move |_| on_scan.call(row.id), "scan" }
                            }
                            if row.reachable {
                                button {
                                    class: "mini",
                                    disabled: busy || !row.affordable,
                                    onclick: move |_| on_jump.call(row.id),
                                    if row.affordable { "jump ({row.cost:.0} fuel)" } else { "jump (low fuel)" }
                                }
                            }
                        }
                    }
                    for (idx, kind, depleted, landable) in row.sites.clone() {
                        div { class: "nav-site",
                            span { "{kind}" if depleted { span { class: "muted", " \u{00b7} stripped" } } }
                            if row.is_current && !depleted {
                                if landable {
                                    button { class: "mini", disabled: busy, onclick: move |_| on_land.call(idx), "land" }
                                } else {
                                    button { class: "mini", disabled: busy, onclick: move |_| on_resolve.call((row.id, idx)), "work" }
                                }
                            }
                        }
                    }
                    if !row.scanned && row.sites.is_empty() {
                        div { class: "muted", "Scan to reveal what's here." }
                    }
                }
            }
            if away {
                div { class: "muted", "Return to the station to deposit cargo and resume control." }
            }
        }
    }
}

#[component]
fn ContactPanel(
    vm: ContactVM,
    on_parley: EventHandler<()>,
    on_trade: EventHandler<()>,
    on_gift: EventHandler<()>,
    on_avoid: EventHandler<()>,
    on_attack: EventHandler<()>,
    on_flee: EventHandler<()>,
    on_stance: EventHandler<()>,
    on_repair: EventHandler<()>,
) -> Element {
    let disp_pct = ((vm.disposition + 100.0) / 2.0).clamp(0.0, 100.0);
    let hull_pct = if vm.hull_max > 0.0 {
        (vm.hull / vm.hull_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    rsx! {
        div { class: "panel contact",
            div { class: "panel-title", "CONTACT \u{2014} {vm.kind}" }
            div { class: "muted", "Hull {vm.hull:.0} / {vm.hull_max:.0}" }
            div { class: "bar bar-hull", span { style: "width: {hull_pct}%" } }
            if vm.in_combat {
                div { class: "muted", "Enemy hull {vm.enemy_hp:.0}" }
                div { class: "bar bar-enemy", span { style: "width: {(vm.enemy_hp).clamp(0.0,100.0)}%" } }
                div { class: "contact-actions",
                    button { class: "mini", onclick: move |_| on_stance.call(()), "stance: {vm.stance}" }
                    button { class: "mini", disabled: vm.repair_drones == 0, onclick: move |_| on_repair.call(()), "Repair drone ({vm.repair_drones})" }
                    button { class: "act", disabled: !vm.can_flee, onclick: move |_| on_flee.call(()), "Flee" }
                }
            } else {
                if vm.is_peer {
                    div { class: "neg-band band-{vm.band}", "Disposition: {vm.band} ({vm.disposition:.0})" }
                    div { class: "bar bar-disp", span { style: "width: {disp_pct}%" } }
                    div { class: "muted", "It moves the way you move. It is deciding about you, too." }
                } else {
                    div { class: "muted", "A husk-ship. It does not answer hails." }
                }
                div { class: "contact-actions",
                    if vm.is_peer {
                        button { class: "mini", disabled: !vm.can_parley, onclick: move |_| on_parley.call(()), "Parley" }
                        button { class: "mini", disabled: !vm.can_trade, onclick: move |_| on_trade.call(()), "Trade" }
                        button { class: "mini", disabled: !vm.can_gift, onclick: move |_| on_gift.call(()), "Gift" }
                        button { class: "mini", disabled: !vm.can_avoid, onclick: move |_| on_avoid.call(()), "Avoid" }
                    }
                    button { class: "act", onclick: move |_| on_attack.call(()), "Attack" }
                    button { class: "act", disabled: !vm.can_flee, onclick: move |_| on_flee.call(()), "Flee" }
                }
            }
        }
    }
}

#[component]
fn EndgamePanel(
    vm: EndgameVM,
    on_choose: EventHandler<sim::Ending>,
    on_restart: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "panel endgame",
            if vm.ended {
                div { class: "panel-title", "{vm.ending_title}" }
                div { class: "endgame-body", "{vm.ending_body}" }
                div { class: "muted endgame-complete", "The run is complete." }
                button { class: "act", onclick: move |_| on_restart.call(()), "Begin again" }
            } else {
                div { class: "panel-title", "THE WATCHED" }
                div { class: "muted", "It waits. The record is open. Choose how this ends." }
                div { class: "endgame-choices",
                    for (e, label, desc) in vm.choices.clone() {
                        button {
                            class: "act endgame-choice",
                            onclick: move |_| on_choose.call(e),
                            div { class: "endgame-choice-label", "{label}" }
                            div { class: "muted", "{desc}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SurfaceView(
    vm: LandingVM,
    tier: u8,
    on_move: EventHandler<u32>,
    on_harvest: EventHandler<()>,
    on_leave: EventHandler<()>,
    on_extract: EventHandler<()>,
    on_use_consumable: EventHandler<sim::gear::ConsumableKind>,
    on_stance: EventHandler<()>,
    on_flee: EventHandler<()>,
    on_parley: EventHandler<()>,
    on_trade: EventHandler<()>,
    on_gift: EventHandler<()>,
    on_avoid: EventHandler<()>,
    on_observe: EventHandler<()>,
    on_harvest_fauna: EventHandler<()>,
    on_attack_alien: EventHandler<()>,
) -> Element {
    let suit_pct = (vm.suit / vm.suit_max * 100.0).clamp(0.0, 100.0);
    let busy = vm.traverse > 0;

    // Layout nodes by depth (x) and band position (y).
    // Pixel-space layout: 70px per depth step (x), 46px per slot (y), 30px margin.
    const PX_X: f64 = 70.0;
    const PX_Y: f64 = 46.0;
    const MARGIN: f64 = 30.0;
    let max_depth = vm.nodes.iter().map(|n| n.depth).max().unwrap_or(0).max(1);
    let mut by_depth: std::collections::BTreeMap<u32, Vec<&SurfaceNodeVM>> = Default::default();
    for n in &vm.nodes {
        by_depth.entry(n.depth).or_default().push(n);
    }
    let max_band = by_depth.values().map(|v| v.len()).max().unwrap_or(1).max(1);
    let placed: Vec<(SurfaceNodeVM, f64, f64)> = by_depth
        .iter()
        .flat_map(|(d, band)| {
            let n = band.len();
            band.iter()
                .enumerate()
                .map(move |(i, node)| {
                    let x = MARGIN + (*d as f64) * PX_X;
                    let y = MARGIN
                        + (i as f64 + 0.5 - n as f64 / 2.0) * PX_Y
                        + (max_band as f64 / 2.0) * PX_Y;
                    ((*node).clone(), x, y)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    // SVG content size derived from the actual node extent.
    let svg_w = (MARGIN + max_depth as f64 * PX_X + MARGIN).max(120.0);
    let svg_h = (MARGIN + max_band as f64 * PX_Y + MARGIN).max(80.0);
    let view = format!("0 0 {svg_w} {svg_h}");

    // Current node's reachable set — bidirectional: cur→n (forward) OR n→cur (backward).
    let cur = vm.nodes.iter().find(|n| n.is_current);
    let cur_id = cur.map(|n| n.id).unwrap_or(u32::MAX);
    let cur_edges: Vec<u32> = vm
        .nodes
        .iter()
        .filter(|n| !n.is_current)
        .filter(|n| {
            // current→n (forward) OR n→current (backward)
            cur.map_or(false, |c| c.edges.contains(&n.id)) || n.edges.contains(&cur_id)
        })
        .map(|n| n.id)
        .collect();

    // Build a lookup from node id to (x, y) for drawing edges.
    let pos_map: std::collections::HashMap<u32, (f64, f64)> =
        placed.iter().map(|(n, x, y)| (n.id, (*x, *y))).collect();

    let squad_hp_pct = if vm.squad_hp_max > 0.0 {
        (vm.squad_hp / vm.squad_hp_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let morale_pct = if vm.morale_max > 0.0 {
        (vm.morale / vm.morale_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let disp_pct = vm
        .negotiation
        .as_ref()
        .map(|neg| ((neg.disposition + 100.0) / 2.0).clamp(0.0, 100.0))
        .unwrap_or(0.0);

    rsx! {
        div { class: "panel surface",
            div { class: "panel-title", "SURFACE \u{00b7} {vm.biome}" }
            div { class: "muted", "Suit {vm.suit:.0} / {vm.suit_max:.0}" }
            div { class: "bar bar-suit", span { style: "width: {suit_pct}%" } }
            div { class: "surf-here", "You are at: {vm.encounter} (depth {vm.depth})" }
            if busy { div { class: "muted", "Crossing the ground\u{2026} {vm.traverse}s" } }
            div { class: "panel-title", "SATCHEL" }
            if vm.satchel.is_empty() { div { class: "muted", "empty" } }
            for (name, amt) in vm.satchel.clone() {
                div { class: "res-row", span { "{name}" } span { "{amt:.0}" } }
            }
            if let Some(c) = vm.combat.clone() {
                div { class: "combat",
                    div { class: "panel-title", "CONTACT \u{2014} REMNANT" }
                    div { class: "muted", "Enemy hull {c.enemy_hp:.0}" }
                    div { class: "bar bar-enemy", span { style: "width: {(c.enemy_hp).clamp(0.0,100.0)}%" } }
                    div { class: "muted", "{drift_word(DriftSlot::AwayTeam, tier)} \u{00d7}{vm.away_crew}  \u{00b7}  HP {vm.squad_hp:.0}/{vm.squad_hp_max:.0}  \u{00b7}  morale {vm.morale:.0}" }
                    div { class: "bar bar-hp", span { style: "width: {squad_hp_pct}%" } }
                    div { class: "bar bar-morale", span { style: "width: {morale_pct}%" } }
                    if c.shield_ticks > 0 { div { class: "muted", "shield {c.shield_ticks}s" } }
                    div { class: "combat-actions",
                        button { class: "mini", onclick: move |_| on_stance.call(()), "stance: {c.stance}" }
                        for (kind, label, count) in vm.consumables.clone() {
                            {
                                let kind = kind;
                                rsx! {
                                    button { class: "mini", disabled: count == 0, onclick: move |_| on_use_consumable.call(kind), "{label} ({count})" }
                                }
                            }
                        }
                        button { class: "act", onclick: move |_| on_flee.call(()), "Flee" }
                    }
                }
            } else if let Some(neg) = vm.negotiation.clone() {
                div { class: "neg",
                    div { class: "panel-title", "CONTACT \u{2014} {neg.kind}" }
                    div { class: "neg-band band-{neg.band}", "Disposition: {neg.band} ({neg.disposition:.0})" }
                    div { class: "bar bar-disp", span { style: "width: {disp_pct}%" } }
                    div { class: "muted",
                        if neg.is_peer { "It moves the way you move. It is deciding about you, too." }
                        else { "Native life. Wary, watching." }
                    }
                    div { class: "neg-actions",
                        button { class: "mini", disabled: !neg.can_parley, onclick: move |_| on_parley.call(()), "Parley" }
                        if neg.is_peer {
                            button { class: "mini", disabled: !neg.can_trade, onclick: move |_| on_trade.call(()), "Trade" }
                        }
                        button { class: "mini", disabled: !neg.can_gift, onclick: move |_| on_gift.call(()), "Gift" }
                        if !neg.is_peer {
                            button { class: "mini", disabled: !neg.can_observe, onclick: move |_| on_observe.call(()), "Observe" }
                            button { class: "mini", disabled: !neg.can_harvest, onclick: move |_| on_harvest_fauna.call(()), "{drift_word(DriftSlot::Take, tier)}" }
                        }
                        button { class: "act", onclick: move |_| on_avoid.call(()), "Avoid" }
                        button { class: "act", onclick: move |_| on_attack_alien.call(()), "Attack" }
                    }
                }
            } else if vm.pending_choice {
                div { class: "surf-choice",
                    div { class: "muted", "Something living. It has not noticed you yet." }
                    button { class: "act", onclick: move |_| on_harvest.call(()), "{drift_word(DriftSlot::Take, tier)} it" }
                    button { class: "act", onclick: move |_| on_leave.call(()), "Leave it" }
                }
            } else {
                // SVG node-graph map — scrollable, pixel-sized to content.
                div { class: "surfmap-scroll",
                svg {
                    class: "surfmap",
                    view_box: "{view}",
                    width: "{svg_w}",
                    height: "{svg_h}",
                    // Edges — draw between placed nodes that share an edge.
                    for (node, x1v, y1v) in placed.clone() {
                        for eid in node.edges.clone() {
                            if let Some((x2v, y2v)) = pos_map.get(&eid) {
                                line {
                                    x1: "{x1v}",
                                    y1: "{y1v}",
                                    x2: "{x2v}",
                                    y2: "{y2v}",
                                    class: "surf-edge",
                                }
                            }
                        }
                    }
                    // Node circles.
                    for (node, xv, yv) in placed.clone() {
                        {
                            let id = node.id;
                            let reachable = cur_edges.contains(&id);
                            let node_class = {
                                let mut c = "surf-node".to_string();
                                if node.is_current {
                                    c.push_str(" surf-current");
                                } else if reachable {
                                    c.push_str(" surf-reach");
                                } else if node.resolved {
                                    c.push_str(" surf-done");
                                } else {
                                    c.push_str(" surf-far");
                                }
                                if node.kind == "Hazard" {
                                    c.push_str(" kind-Hazard");
                                }
                                c
                            };
                            let r_val = if node.is_current { "4" } else { "3" };
                            rsx! {
                                circle {
                                    cx: "{xv}",
                                    cy: "{yv}",
                                    r: "{r_val}",
                                    class: "{node_class}",
                                    onclick: move |_| {
                                        if reachable && !busy {
                                            on_move.call(id);
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
                } // end surfmap-scroll div
                if cur_edges.is_empty() {
                    div { class: "muted", "No reachable nodes. Extract." }
                }
            }
            button { class: "act ghost", onclick: move |_| on_extract.call(()), "Extract to orbit" }
        }
    }
}

#[component]
fn LoadoutPanel(
    vm: LoadoutVM,
    tier: u8,
    can_set_crew: bool,
    on_set_crew: EventHandler<i32>,
    on_upgrade_weapon: EventHandler<()>,
    on_upgrade_armor: EventHandler<()>,
    on_craft: EventHandler<sim::gear::ConsumableKind>,
) -> Element {
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "AWAY-TEAM" }
            div { class: "job-row",
                span { "{drift_word(DriftSlot::AwayTeam, tier)} \u{00d7}{vm.away_crew}  " span { class: "muted", "({vm.idle} idle)" } }
                span {
                    button { class: "mini", disabled: !can_set_crew || vm.away_crew == 0, onclick: move |_| on_set_crew.call(-1), "\u{2212}" }
                    button { class: "mini", disabled: !can_set_crew || vm.idle == 0, onclick: move |_| on_set_crew.call(1), "+" }
                }
            }
            if !can_set_crew { div { class: "muted", "Dock to change the away-team." } }
            div { class: "panel-title", "ARMORY" }
            div { class: "job-row",
                span { "Weapon T{vm.weapon_tier}" }
                button { class: "mini", disabled: vm.weapon_tier >= 2, onclick: move |_| on_upgrade_weapon.call(()), "upgrade" }
            }
            div { class: "job-row",
                span { "Armor T{vm.armor_tier}" }
                button { class: "mini", disabled: vm.armor_tier >= 2, onclick: move |_| on_upgrade_armor.call(()), "upgrade" }
            }
            for (kind, label, count) in vm.consumables.clone() {
                div { class: "job-row",
                    span { "{label} \u{00d7}{count}" }
                    button { class: "mini", onclick: move |_| on_craft.call(kind), "craft" }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct EndgameVM {
    ended: bool,
    // WatchedArrived: the earned choices — (ending, button label, one-line meaning)
    choices: Vec<(sim::Ending, String, String)>,
    // Ended: the chosen ending's per-origin closing note (read from the codex)
    ending_title: String,
    ending_body: String,
}

#[component]
fn ShipMunitionsPanel(
    vm: ShipMunitionsVM,
    on_upgrade_weapon: EventHandler<()>,
    on_upgrade_plating: EventHandler<()>,
    on_craft_drone: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "panel munitions",
            div { class: "panel-title", "SHIP MUNITIONS" }
            div { class: "muted", "Hull {vm.hull:.0}/{vm.hull_max:.0}  \u{00b7}  guns T{vm.weapon_tier}  \u{00b7}  plating T{vm.plating_tier}  \u{00b7}  drones {vm.repair_drones}" }
            div { class: "contact-actions",
                button { class: "mini", disabled: !vm.can_upgrade_weapon, onclick: move |_| on_upgrade_weapon.call(()), "Upgrade guns" }
                button { class: "mini", disabled: !vm.can_upgrade_plating, onclick: move |_| on_upgrade_plating.call(()), "Upgrade plating" }
                button { class: "mini", disabled: !vm.can_craft_drone, onclick: move |_| on_craft_drone.call(()), "Craft repair drone" }
            }
        }
    }
}

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

    rsx! {
        style { dangerous_inner_html: CSS }
        main { class: "console",
            h1 { class: "title", "COLD STAR" }
            if has_game {
                Console { state, tuning, jobs, modules, techs }
            } else {
                OriginSelect { state, tuning, jobs, modules, techs, origins }
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
        div { class: "readout", "CHOOSE AN ORIGIN" }
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
        return rsx! { div {} };
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
        let neighbors = cur
            .as_ref()
            .map(|n| {
                n.edges
                    .iter()
                    .filter_map(|&e| {
                        surface
                            .get(e)
                            .map(|nn| (e, format!("{:?}", nn.kind), resolved.contains(&e)))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
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
            node: l.node,
            depth: cur.as_ref().map(|n| n.depth).unwrap_or(0),
            encounter: cur
                .as_ref()
                .map(|n| format!("{:?}", n.kind))
                .unwrap_or_default(),
            pending_choice: l.pending_choice,
            traverse: l.traverse,
            neighbors,
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
        // Reactor readout — the heartbeat. ALWAYS visible (charge + status),
        // independent of disclosure, so the core gauge never disappears.
        div { class: "readout", "REACTOR CORE" }
        div { class: "charge", "{charge:.0} / {capacity:.0}" }
        div { class: "status status-{status}", "{status}" }
        if shipyard_complete {
            div { class: "readout", "SHIP \u{00b7} drive {ship_drive:.0} \u{00b7} cargo {ship_cargo_used:.0}/{ship_cargo_max:.0} \u{00b7} isotopes {isotopes:.0}" }
        }

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
                    div { class: "panel",
                        div { class: "panel-title", "LOG" }
                        div { class: "log",
                            for (tick, text) in log_lines.clone() {
                                div { class: "log-line", "[{tick}] {text}" }
                            }
                        }
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
            // bare opening (nothing disclosed yet) and adds the intro log.
            if salvage_unlocked {
                div { class: "res",
                    "Scrap: {scrap:.0}"
                    if crew_arrived { "   \u{00b7}   Crew: {crew}" }
                }
            }
            div { class: "log",
                for (tick, text) in log_lines {
                    div { class: "log-line", "[{tick}] {text}" }
                }
            }
        }
        } // else (no endgame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drift_word_per_slot_and_tier() {
        // Take slot
        assert_eq!(drift_word(DriftSlot::Take, 0), "Harvest");
        assert_eq!(drift_word(DriftSlot::Take, 1), "Extract");
        assert_eq!(drift_word(DriftSlot::Take, 2), "Process");
        assert_eq!(drift_word(DriftSlot::Take, 3), "Strip");
        // Clamp at 3
        assert_eq!(drift_word(DriftSlot::Take, 4), "Strip");
        assert_eq!(drift_word(DriftSlot::Take, 255), "Strip");

        // Salvage slot
        assert_eq!(drift_word(DriftSlot::Salvage, 0), "Salvage");
        assert_eq!(drift_word(DriftSlot::Salvage, 3), "Render");

        // FeedCore slot
        assert_eq!(drift_word(DriftSlot::FeedCore, 0), "Feed the core");
        assert_eq!(drift_word(DriftSlot::FeedCore, 3), "Burn it");

        // AwayTeam slot
        assert_eq!(drift_word(DriftSlot::AwayTeam, 0), "Marines");
        assert_eq!(drift_word(DriftSlot::AwayTeam, 3), "the asset");
    }

    #[test]
    fn crew_word_origin_at_t0_cold_at_higher() {
        assert_eq!(crew_word(0, "survivor"), "survivor");
        assert_eq!(crew_word(1, "survivor"), "crew");
        assert_eq!(crew_word(2, "survivor"), "units");
        assert_eq!(crew_word(3, "survivor"), "yield");
        // Clamp beyond 3 still returns "yield"
        assert_eq!(crew_word(4, "survivor"), "yield");
    }
}
