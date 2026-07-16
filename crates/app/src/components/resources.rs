use dioxus::prelude::*;

#[component]
pub(crate) fn ResourcesPanel(rows: Vec<(String, f64, f64, f64, bool)>) -> Element {
    // rows: (name, amount, cap, net, at_cap)
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Resources \u{00b7} net / tick" }
            for (name, amount, cap, net, at_cap) in rows {
                div { class: "res-row",
                    span {
                        class: if at_cap { "res-full" } else { "" },
                        "{name}"
                        if at_cap { span { class: "res-full", " (full)" } }
                    }
                    span { class: "num",
                        "{amount:.0}"
                        if cap.is_finite() && cap < 1e9 { span { class: "mut", " / {cap:.0}" } }
                        " \u{00a0}"
                        if net > 0.0 {
                            span { class: "mut up", "+{net:.1}" }
                        } else if net < 0.0 {
                            span { class: "mut down", "\u{2212}{(-net):.1}" }
                        }
                    }
                }
            }
        }
    }
}
