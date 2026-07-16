use dioxus::prelude::*;

#[component]
pub(crate) fn BuildMenu(
    rows: Vec<(sim::module::ModuleId, String, String, String, u32, bool)>,
    on_build: EventHandler<sim::module::ModuleId>,
) -> Element {
    // Which module's narrative info line is currently expanded (toggled by its
    // (i) button). None = all collapsed.
    let mut info_open = use_signal(|| None::<sim::module::ModuleId>);
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Modules" }
            div { class: "mods",
                for (id, name, description, cost, n, affordable) in rows {
                    div { class: if affordable { "mod" } else { "mod dim" },
                        b { "{name}" if n > 0 { span { class: "mut", " ({n})" } } }
                        span { class: "tag", "{cost}" }
                        if info_open() == Some(id) {
                            div { class: "build-desc", "{description}" }
                        }
                        div { style: "margin-top: 8px; display: flex; gap: 4px; align-items: center;",
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
                        }
                    }
                }
            }
        }
    }
}
