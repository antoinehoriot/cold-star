use dioxus::prelude::*;

#[component]
pub(crate) fn ResearchPanel(
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
            p { class: "eyebrow", "Research" }
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
