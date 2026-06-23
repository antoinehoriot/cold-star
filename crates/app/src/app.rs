use dioxus::prelude::*;
use sim::Tuning;
use sim::flags::Flag;
use sim::job::JobRegistry;
use sim::module::ModuleRegistry;
use sim::origin::OriginDef;
use sim::research::{Tech, TechRegistry};
use sim::resources::ResourceKind;
use sim::state::{Action, GameState, World};

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
";

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
    rows: Vec<(sim::job::JobId, String, u32, bool, &'static str)>,
    on_assign: EventHandler<(sim::job::JobId, i32)>,
) -> Element {
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "CREW" }
            div { class: "idle", "Idle: {idle}" }
            if starving { div { class: "starving", "crew starving \u{2014} rations short" } }
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
            prereqs_ok && buildable_again
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
        // Reactor readout — the heartbeat. ALWAYS visible (charge + status),
        // independent of disclosure, so the core gauge never disappears.
        div { class: "readout", "REACTOR CORE" }
        div { class: "charge", "{charge:.0} / {capacity:.0}" }
        div { class: "status status-{status}", "{status}" }

        // Core action buttons — always rendered regardless of economy state.
        if is_cold {
            button { class: "act", onclick: move |_| act(Action::Ignite), "Ignite the core" }
        } else {
            if salvage_unlocked {
                button {
                    class: "act",
                    disabled: salvage_cooldown > 0,
                    onclick: move |_| act(Action::Salvage),
                    if salvage_cooldown > 0 { "Salvage ({salvage_cooldown}s)" } else { "Salvage" }
                }
                button { class: "act", onclick: move |_| act(Action::FeedCore), "Feed the core" }
            }
            if source_available && !source_active {
                button {
                    class: "act",
                    onclick: move |_| act(Action::ActivateCrewSource),
                    "Activate {source_name}"
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

        if build_menu_unlocked || economy_unlocked || power_visible || salvage_unlocked {
            // Two-column dashboard — panels appear via per-panel disclosure flags (§5).
            div { class: "dash",
                div { class: "col",
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
                }
                div { class: "col",
                    if economy_unlocked {
                        JobPanel {
                            idle,
                            starving,
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
    }
}
