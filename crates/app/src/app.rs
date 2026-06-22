use dioxus::prelude::*;
use sim::Tuning;
use sim::flags::Flag;
use sim::job::JobRegistry;
use sim::module::ModuleRegistry;
use sim::origin::OriginDef;
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
";

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
        div { class: "readout", "REACTOR CORE" }
        div { class: "charge", "{charge:.0} / {capacity:.0}" }
        div { class: "status status-{status}", "{status}" }
        if salvage_unlocked {
            div { class: "res",
                "Scrap: {scrap:.0}"
                if crew_arrived { "   ·   Crew: {crew}" }
            }
        }

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

        div { class: "log",
            for (tick, text) in log_lines {
                div { class: "log-line", "[{tick}] {text}" }
            }
        }
    }
}
