use dioxus::prelude::*;

#[component]
pub(crate) fn PowerMeter(
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
            p { class: "eyebrow", "Reactor" }
            div { class: "status status-{status}", "{status}" }
            div { class: "res-row",
                span { "Output" }
                span { class: "num", "{generation:.1}" }
            }
            div { class: "bar bar-gen", span { style: "width: {gen_pct}%" } }
            div { class: "res-row",
                span { "Draw" }
                span { class: "num", "{active_draw:.1}" }
            }
            div { class: "bar bar-draw", span { style: "width: {draw_pct}%" } }
            div { class: "res-row",
                span { "Charge" }
                span { class: "num", "{charge:.0} / {capacity:.0}" }
            }
            div { class: "bar bar-charge", span { style: "width: {charge_pct}%" } }
            div { class: "res-row",
                span { "Fuel" }
                span { class: "num", "{fuel:.0}" }
            }
        }
    }
}
