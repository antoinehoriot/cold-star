use dioxus::prelude::*;

use crate::vm::ContactVM;

#[component]
pub(crate) fn ContactPanel(
    vm: ContactVM,
    on_parley: EventHandler<()>,
    on_trade: EventHandler<()>,
    on_gift: EventHandler<()>,
    on_avoid: EventHandler<()>,
    on_attack: EventHandler<()>,
    on_flee: EventHandler<()>,
    on_stance: EventHandler<()>,
    on_repair: EventHandler<()>,
) -> Element {
    let disp_pct = ((vm.disposition + 100.0) / 2.0).clamp(0.0, 100.0);
    let hull_pct = if vm.hull_max > 0.0 {
        (vm.hull / vm.hull_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    rsx! {
        div { class: "panel contact",
            p { class: "eyebrow", "Contact \u{00b7} {vm.kind}" }
            // Hull readout + bar
            div { class: "contact-stat-row",
                span { class: "mut", "Hull" }
                span { class: "num", "{vm.hull:.0} / {vm.hull_max:.0}" }
            }
            div { class: "bar bar-hull", span { style: "width: {hull_pct}%" } }
            if vm.in_combat {
                // Combat mode: enemy hull + actions
                div { class: "contact-stat-row",
                    span { class: "mut", "Enemy hull" }
                    span { class: "num down", "{vm.enemy_hp:.0}" }
                }
                div { class: "bar bar-enemy", span { style: "width: {(vm.enemy_hp).clamp(0.0,100.0)}%" } }
                div { class: "contact-actions",
                    button { class: "mini", onclick: move |_| on_stance.call(()), "Stance: {vm.stance}" }
                    button { class: "mini", disabled: vm.repair_drones == 0, onclick: move |_| on_repair.call(()), "Repair drone ({vm.repair_drones})" }
                    button { class: "act", disabled: !vm.can_flee, onclick: move |_| on_flee.call(()), "Flee" }
                }
            } else {
                // Hail mode: disposition band + verbs (peer) or husk note
                if vm.is_peer {
                    div { class: "neg-band band-{vm.band}", "Disposition: {vm.band} ({vm.disposition:.0})" }
                    div { class: "bar bar-disp", span { style: "width: {disp_pct}%" } }
                    div { class: "muted", style: "margin: .35rem 0 .1rem;",
                        "It moves the way you move. It is deciding about you, too."
                    }
                } else {
                    div { class: "muted", "A husk-ship. It does not answer hails." }
                }
                div { class: "contact-actions",
                    if vm.is_peer {
                        button { class: "mini", disabled: !vm.can_parley, onclick: move |_| on_parley.call(()), "Parley" }
                        button { class: "mini", disabled: !vm.can_trade, onclick: move |_| on_trade.call(()), "Trade" }
                        button { class: "mini", disabled: !vm.can_gift, onclick: move |_| on_gift.call(()), "Gift" }
                        button { class: "mini", disabled: !vm.can_avoid, onclick: move |_| on_avoid.call(()), "Avoid" }
                    }
                    button { class: "act", onclick: move |_| on_attack.call(()), "Attack" }
                    button { class: "act", disabled: !vm.can_flee, onclick: move |_| on_flee.call(()), "Flee" }
                }
            }
        }
    }
}
