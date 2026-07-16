use dioxus::prelude::*;

use crate::vm::NavRow;

#[component]
pub(crate) fn NavigationPanel(
    drive: f64,
    cargo_used: f64,
    cargo_max: f64,
    fuel: f64,
    isotopes: f64,
    activity_label: String,
    busy: bool,
    away: bool,
    has_sensor: bool,
    stranded: bool,
    can_upgrade_drive: bool,
    current: sim::galaxy::SystemId,
    rows: Vec<NavRow>,
    on_scan: EventHandler<sim::galaxy::SystemId>,
    on_jump: EventHandler<sim::galaxy::SystemId>,
    on_resolve: EventHandler<(sim::galaxy::SystemId, usize)>,
    on_scavenge: EventHandler<()>,
    on_upgrade: EventHandler<()>,
    on_land: EventHandler<usize>,
) -> Element {
    let mut selected = use_signal(|| current);
    // Center on the current system's position.
    let center = rows
        .iter()
        .find(|r| r.is_current)
        .map(|r| r.pos)
        .unwrap_or((0.0, 0.0));
    // viewBox half-extent: fit the drive ring + a margin (clamped so a huge drive still renders).
    let half = (drive * 1.25).clamp(30.0, 140.0);
    let view = format!("{} {} {} {}", -half, -half, 2.0 * half, 2.0 * half);
    let sel = selected();
    let sel_row = rows.iter().find(|r| r.id == sel).cloned();
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Navigation" }
            div { class: "muted",
                "Drive {drive:.0} \u{00b7} Cargo {cargo_used:.0}/{cargo_max:.0} \u{00b7} Fuel {fuel:.0} \u{00b7} Isotopes {isotopes:.0}"
            }
            if !activity_label.is_empty() {
                div { class: "nav-activity", "{activity_label}" }
            }
            if stranded {
                button { class: "act", onclick: move |_| on_scavenge.call(()), "Scavenge fuel" }
            }
            if can_upgrade_drive {
                button { class: "act", onclick: move |_| on_upgrade.call(()), "Recalibrate drive (+range)" }
            }
            // Map + detail side-by-side on wide screens; stacked on narrow.
            div { class: "nav-split",
            // SVG star map — live-tinted: range ring + current dot use var(--accent) via CSS.
            svg {
                class: "navmap",
                view_box: "{view}",
                preserve_aspect_ratio: "xMidYMid meet",
                // range ring: styled via .range-ring (stroke: var(--accent))
                circle { cx: 0, cy: 0, r: "{drive}", class: "range-ring" }
                // edges from current to reachable systems
                for row in rows.iter().filter(|r| r.reachable) {
                    line {
                        x1: 0, y1: 0,
                        x2: "{row.pos.0 - center.0}", y2: "{row.pos.1 - center.1}",
                        class: "nav-edge",
                    }
                }
                // system dots
                for row in rows.clone() {
                    {
                        let id = row.id;
                        let is_home = row.id.0 == 0;
                        let dot_class = {
                            let mut c = format!("sys star-{}", row.star);
                            if row.is_current { c.push_str(" sys-current"); }
                            else if row.reachable { c.push_str(" sys-reachable"); }
                            else { c.push_str(" sys-far"); }
                            if is_home { c.push_str(" sys-home"); }
                            if sel == row.id { c.push_str(" sys-selected"); }
                            c
                        };
                        let r_val = if is_home { "5.5" } else if row.is_current { "4.5" } else { "3.2" };
                        let cx_val = format!("{}", row.pos.0 - center.0);
                        let cy_val = format!("{}", row.pos.1 - center.1);
                        let label_y = format!("{}", row.pos.1 - center.1 + 14.0);
                        rsx! {
                            // The home station gets a halo + label so it always reads as "home".
                            if is_home {
                                circle { cx: "{cx_val}", cy: "{cy_val}", r: "10", class: "sys-home-ring" }
                            }
                            circle {
                                cx: "{cx_val}",
                                cy: "{cy_val}",
                                r: "{r_val}",
                                class: "{dot_class}",
                                onclick: move |_| selected.set(id),
                            }
                            if is_home {
                                text {
                                    x: "{cx_val}",
                                    y: "{label_y}",
                                    text_anchor: "middle",
                                    class: "sys-home-label",
                                    "HOME"
                                }
                            }
                        }
                    }
                }
            }
            // Detail + actions for the selected system.
            if let Some(row) = sel_row {
                div { class: "nav-detail",
                    div { class: "nav-row",
                        span {
                            if row.is_current { span { class: "nav-here", "\u{25c9} " } }
                            span { class: "nav-sys-meta",
                                "Sys {row.id.0} \u{00b7} {row.star} \u{00b7} {row.dist:.0} ly"
                            }
                            if row.scanned { span { class: "muted", " \u{00b7} scanned" } } else { span { class: "muted", " \u{00b7} unscanned" } }
                        }
                        span { class: "nav-sys-actions",
                            if !row.is_current && has_sensor && !row.scanned {
                                button { class: "mini", disabled: busy, onclick: move |_| on_scan.call(row.id), "Scan" }
                            }
                            if row.reachable {
                                button {
                                    class: "act",
                                    disabled: busy || !row.affordable,
                                    onclick: move |_| on_jump.call(row.id),
                                    if row.affordable { "Jump \u{00b7} {row.cost:.0} fuel" } else { "Jump (low fuel)" }
                                }
                            }
                        }
                    }
                    for (idx, kind, depleted, landable) in row.sites.clone() {
                        div { class: "nav-site",
                            span { "{kind}" if depleted { span { class: "muted", " \u{00b7} stripped" } } }
                            if row.is_current && !depleted {
                                if landable {
                                    button { class: "act", disabled: busy, onclick: move |_| on_land.call(idx), "Land" }
                                } else {
                                    button { class: "mini", disabled: busy, onclick: move |_| on_resolve.call((row.id, idx)), "work" }
                                }
                            }
                        }
                    }
                    if !row.scanned && row.sites.is_empty() {
                        div { class: "muted", "Scan to reveal what\u{2019}s here." }
                    }
                }
            }
            } // end nav-split
            if away {
                div { class: "muted", "Return to the station to deposit cargo and resume control." }
            }
        }
    }
}
