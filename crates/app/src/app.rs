use dioxus::prelude::*;
use sim::Tuning;

const CSS: &str = "
:root { color-scheme: dark; }
body { margin: 0; background: #05070a; color: #c7d3dd;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
.console { max-width: 560px; margin: 8vh auto; padding: 2rem;
  border: 1px solid #16313f; border-radius: 6px; background: #070b10;
  box-shadow: 0 0 40px rgba(40,120,160,.08); }
.title { letter-spacing: .4em; font-weight: 600; color: #6fb7d6; margin: 0 0 1.5rem; }
.readout { font-size: .72rem; letter-spacing: .25em; color: #5b7686; }
.charge { font-size: 2.4rem; margin: .2rem 0; color: #d8e6ef; }
.status { font-size: .8rem; letter-spacing: .2em; }
.status-Stable { color: #6fd6a0; }
.status-Igniting { color: #d6c46f; }
.status-Cold { color: #d66f6f; }
.act { margin-top: 1.5rem; margin-right: .6rem; padding: .6rem 1.1rem;
  background: #0c1822; color: #c7d3dd; border: 1px solid #1d4456;
  border-radius: 4px; font-family: inherit; cursor: pointer; }
.act:hover { background: #112433; }
.act.ghost { opacity: .7; }
";

#[component]
pub fn App() -> Element {
    let tuning = use_signal(|| content::load().unwrap_or_else(|_| Tuning::default()));
    let mut state = use_signal(|| crate::persist::load_initial(&tuning.read()));

    crate::tick::use_tick_driver(state, tuning, 1000.0, 8 * 60 * 60);

    let r = state.read();
    let charge = r.reactor.charge;
    let capacity = r.reactor.capacity;
    let status = format!("{:?}", r.reactor.status);
    drop(r);

    rsx! {
        style { dangerous_inner_html: CSS }
        main { class: "console",
            h1 { class: "title", "COLD STAR" }
            div { class: "readout", "REACTOR CORE" }
            div { class: "charge", "{charge:.0} / {capacity:.0}" }
            div { class: "status status-{status}", "{status}" }
            button {
                class: "act",
                onclick: move |_| {
                    let t = tuning.read().clone();
                    let _ = state.write().apply(&t, sim::state::Action::Salvage);
                },
                "Salvage scrap"
            }
            button {
                class: "act ghost",
                onclick: move |_| crate::persist::copy_link(&state.read()),
                "Copy save link"
            }
        }
    }
}
