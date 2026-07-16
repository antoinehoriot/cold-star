use dioxus::prelude::*;

#[component]
pub(crate) fn JobPanel(
    idle: u32,
    starving: bool,
    starving_label: String,
    rows: Vec<(sim::job::JobId, String, u32, bool, &'static str)>,
    on_assign: EventHandler<(sim::job::JobId, i32)>,
) -> Element {
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Crew" }
            div { class: "res-row",
                span { "Idle" }
                span { class: "num", "{idle}" }
            }
            if starving { div { class: "starving", "{starving_label}" } }
            for (id, name, count, has_crew, dark) in rows {
                div { class: "job-row",
                    span {
                        "{name}"
                        span { class: "mut", " \u{00d7}{count}" }
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
