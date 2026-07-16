use dioxus::prelude::*;

#[component]
pub(crate) fn PriorityPanel(
    rows: Vec<(sim::priority::Consumer, String, bool, bool)>,
    on_move: EventHandler<(sim::priority::Consumer, bool)>,
) -> Element {
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Power Priority" }
            p { class: "muted", "higher = browns out last" }
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
