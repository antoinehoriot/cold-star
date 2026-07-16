use dioxus::prelude::*;

use crate::vm::ShipMunitionsVM;

#[component]
pub(crate) fn ShipMunitionsPanel(
    vm: ShipMunitionsVM,
    on_upgrade_weapon: EventHandler<()>,
    on_upgrade_plating: EventHandler<()>,
    on_craft_drone: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "panel munitions",
            p { class: "eyebrow", "Ship munitions" }
            div { class: "job-row",
                span { "Hull" }
                span { class: "num", "{vm.hull:.0} / {vm.hull_max:.0}" }
            }
            div { class: "job-row",
                span { "Guns T{vm.weapon_tier}" }
                button { class: "mini", disabled: !vm.can_upgrade_weapon, onclick: move |_| on_upgrade_weapon.call(()), "upgrade" }
            }
            div { class: "job-row",
                span { "Plating T{vm.plating_tier}" }
                button { class: "mini", disabled: !vm.can_upgrade_plating, onclick: move |_| on_upgrade_plating.call(()), "upgrade" }
            }
            div { class: "job-row",
                span { "Repair drones \u{00d7}{vm.repair_drones}" }
                button { class: "mini", disabled: !vm.can_craft_drone, onclick: move |_| on_craft_drone.call(()), "craft" }
            }
        }
    }
}
