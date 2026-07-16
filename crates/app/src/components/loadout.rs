use dioxus::prelude::*;

use crate::theme::{DriftSlot, drift_word};
use crate::vm::LoadoutVM;

#[component]
pub(crate) fn LoadoutPanel(
    vm: LoadoutVM,
    tier: u8,
    can_set_crew: bool,
    on_set_crew: EventHandler<i32>,
    on_upgrade_weapon: EventHandler<()>,
    on_upgrade_armor: EventHandler<()>,
    on_craft: EventHandler<sim::gear::ConsumableKind>,
) -> Element {
    rsx! {
        div { class: "panel",
            // Away-team section
            p { class: "eyebrow", "Away-team" }
            div { class: "job-row",
                span {
                    span { class: "drift", "{drift_word(DriftSlot::AwayTeam, tier)}" }
                    " \u{00d7}{vm.away_crew}"
                    span { class: "mut", "  ({vm.idle} idle)" }
                }
                span {
                    button { class: "mini", disabled: !can_set_crew || vm.away_crew == 0, onclick: move |_| on_set_crew.call(-1), "\u{2212}" }
                    button { class: "mini", disabled: !can_set_crew || vm.idle == 0, onclick: move |_| on_set_crew.call(1), "+" }
                }
            }
            if !can_set_crew { div { class: "muted", "Dock to change the away-team." } }
            // Armory section
            p { class: "eyebrow", style: "margin-top: .7rem;", "Armory" }
            div { class: "job-row",
                span { "Weapon T{vm.weapon_tier}" }
                button { class: "mini", disabled: vm.weapon_tier >= 2, onclick: move |_| on_upgrade_weapon.call(()), "upgrade" }
            }
            div { class: "job-row",
                span { "Armor T{vm.armor_tier}" }
                button { class: "mini", disabled: vm.armor_tier >= 2, onclick: move |_| on_upgrade_armor.call(()), "upgrade" }
            }
            for (kind, label, count) in vm.consumables.clone() {
                div { class: "job-row",
                    span { "{label} \u{00d7}{count}" }
                    button { class: "mini", onclick: move |_| on_craft.call(kind), "craft" }
                }
            }
        }
    }
}
