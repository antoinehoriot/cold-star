use dioxus::prelude::*;
use sim::Tuning;
use sim::flags::Flag;
use sim::job::JobRegistry;
use sim::module::ModuleRegistry;
use sim::origin::OriginDef;
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
fn ResourcesPanel(rows: Vec<(String, f64, f64, f64)>) -> Element {
    // rows: (name, amount, cap, net)
    rsx! {
        div { class: "panel",
            div { class: "panel-title", "RESOURCES" }
            for (name, amount, cap, net) in rows {
                div { class: "res-row",
                    span {
                        class: if amount >= cap { "res-full" } else { "" },
                        "{name}"
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
pub fn App() -> Element {
    let tuning = use_signal(|| content::load().unwrap_or_else(|_| Tuning::default()));
    let jobs = use_signal(|| content::load_jobs().map(|r| r.0).unwrap_or_default());
    let modules = use_signal(|| content::load_modules().map(|r| r.0).unwrap_or_default());
    let origins = use_signal(|| content::load_origins().unwrap_or_default());
    let state =
        use_signal(|| crate::persist::load_initial(&tuning.read(), &jobs.read(), &modules.read()));

    crate::tick::use_tick_driver(state, tuning, jobs, modules, 1000.0, 8 * 60 * 60);

    let has_game = state.read().is_some();

    rsx! {
        style { dangerous_inner_html: CSS }
        main { class: "console",
            h1 { class: "title", "COLD STAR" }
            if has_game {
                Console { state, tuning, jobs, modules }
            } else {
                OriginSelect { state, tuning, jobs, modules, origins }
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
                        let world = World { tuning: &t, jobs: &j, modules: &m };
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
    let fuel = gs.resources.fuel;
    let generation = gs.telemetry.generation;
    let active_draw = gs.telemetry.active_draw;
    let res_rows: Vec<(String, f64, f64, f64)> = ResourceKind::ALL
        .iter()
        .filter(|k| gs.resources.amount(**k) > 0.0 || gs.telemetry.net.contains_key(*k))
        .map(|k| {
            let net = gs.telemetry.net.get(k).copied().unwrap_or(0.0);
            (
                format!("{k:?}"),
                gs.resources.amount(*k),
                gs.resources.cap,
                net,
            )
        })
        .collect();
    drop(snapshot);

    let mut act = move |action: Action| {
        let t = tuning.read().clone();
        let j = JobRegistry(jobs.read().clone());
        let m = ModuleRegistry(modules.read().clone());
        let world = World {
            tuning: &t,
            jobs: &j,
            modules: &m,
        };
        if let Some(gs) = state.write().as_mut() {
            let _ = gs.apply(&world, action);
        }
    };

    rsx! {
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

        if economy_unlocked {
            // Two-column dashboard (left column this task; right column Task 5).
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
                    ResourcesPanel { rows: res_rows.clone() }
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
                    // Right column (JobPanel / BuildMenu / PriorityPanel) — Task 5.
                }
            }
        } else {
            // Pre-economy M1 readout.
            div { class: "readout", "REACTOR CORE" }
            div { class: "charge", "{charge:.0} / {capacity:.0}" }
            div { class: "status status-{status}", "{status}" }
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
