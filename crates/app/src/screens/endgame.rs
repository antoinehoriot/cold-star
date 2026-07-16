use dioxus::prelude::*;

use crate::vm::EndgameVM;

#[component]
pub(crate) fn EndgamePanel(
    vm: EndgameVM,
    on_choose: EventHandler<sim::Ending>,
    on_restart: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "watched",
            if vm.ended {
                // Ended: per-origin closing note + restart
                p { class: "eyebrow", "The record is closed" }
                h2 { class: "watched-title", "{vm.ending_title}" }
                div { class: "endgame-body", "{vm.ending_body}" }
                div { class: "endgame-complete", "The run is complete." }
                button { class: "act", onclick: move |_| on_restart.call(()), "Begin again" }
            } else {
                // WatchedArrived: takeover panel — cold terminal climax
                p { class: "eyebrow", "Contact \u{00b7} resolved out of the dark" }
                h2 { class: "watched-title", "The Watched" }
                p { class: "watched-body",
                    "It does not attack. It holds position and broadcasts on a frequency your station once used too. It moves the way you move. It has watched you become this."
                    span { class: "mut", " The ruins you stripped were stations like yours. The peers were you, at a different hour." }
                }
                p { class: "eyebrow", style: "margin-top: 1.1rem;", "The record is open. Choose how this ends." }
                div { class: "endgame-choices",
                    for (e, label, desc) in vm.choices.clone() {
                        button {
                            class: "act choice",
                            onclick: move |_| on_choose.call(e),
                            span { class: "c-l", "{label}" }
                            br {}
                            span { class: "c-d", "{desc}" }
                        }
                    }
                }
            }
        }
    }
}
