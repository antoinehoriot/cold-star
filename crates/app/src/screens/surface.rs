use dioxus::prelude::*;

use crate::theme::{DriftSlot, drift_word};
use crate::vm::{LandingVM, SurfaceNodeVM};

#[component]
pub(crate) fn SurfaceView(
    vm: LandingVM,
    tier: u8,
    on_move: EventHandler<u32>,
    on_harvest: EventHandler<()>,
    on_leave: EventHandler<()>,
    on_extract: EventHandler<()>,
    on_use_consumable: EventHandler<sim::gear::ConsumableKind>,
    on_stance: EventHandler<()>,
    on_flee: EventHandler<()>,
    on_parley: EventHandler<()>,
    on_trade: EventHandler<()>,
    on_gift: EventHandler<()>,
    on_avoid: EventHandler<()>,
    on_observe: EventHandler<()>,
    on_harvest_fauna: EventHandler<()>,
    on_attack_alien: EventHandler<()>,
) -> Element {
    let suit_pct = (vm.suit / vm.suit_max * 100.0).clamp(0.0, 100.0);
    let busy = vm.traverse > 0;

    // Layout nodes by depth (x) and band position (y).
    // Pixel-space layout: 70px per depth step (x), 46px per slot (y), 30px margin.
    const PX_X: f64 = 70.0;
    const PX_Y: f64 = 46.0;
    const MARGIN: f64 = 30.0;
    let max_depth = vm.nodes.iter().map(|n| n.depth).max().unwrap_or(0).max(1);
    let mut by_depth: std::collections::BTreeMap<u32, Vec<&SurfaceNodeVM>> = Default::default();
    for n in &vm.nodes {
        by_depth.entry(n.depth).or_default().push(n);
    }
    let max_band = by_depth.values().map(|v| v.len()).max().unwrap_or(1).max(1);
    let placed: Vec<(SurfaceNodeVM, f64, f64)> = by_depth
        .iter()
        .flat_map(|(d, band)| {
            let n = band.len();
            band.iter()
                .enumerate()
                .map(move |(i, node)| {
                    let x = MARGIN + (*d as f64) * PX_X;
                    let y = MARGIN
                        + (i as f64 + 0.5 - n as f64 / 2.0) * PX_Y
                        + (max_band as f64 / 2.0) * PX_Y;
                    ((*node).clone(), x, y)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    // SVG content size derived from the actual node extent.
    let svg_w = (MARGIN + max_depth as f64 * PX_X + MARGIN).max(120.0);
    let svg_h = (MARGIN + max_band as f64 * PX_Y + MARGIN).max(80.0);
    let view = format!("0 0 {svg_w} {svg_h}");

    // Current node's reachable set — bidirectional: cur→n (forward) OR n→cur (backward).
    let cur = vm.nodes.iter().find(|n| n.is_current);
    let cur_id = cur.map(|n| n.id).unwrap_or(u32::MAX);
    let cur_edges: Vec<u32> = vm
        .nodes
        .iter()
        .filter(|n| !n.is_current)
        .filter(|n| {
            // current→n (forward) OR n→current (backward)
            cur.is_some_and(|c| c.edges.contains(&n.id)) || n.edges.contains(&cur_id)
        })
        .map(|n| n.id)
        .collect();

    // Build a lookup from node id to (x, y) for drawing edges.
    let pos_map: std::collections::HashMap<u32, (f64, f64)> =
        placed.iter().map(|(n, x, y)| (n.id, (*x, *y))).collect();

    let squad_hp_pct = if vm.squad_hp_max > 0.0 {
        (vm.squad_hp / vm.squad_hp_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let morale_pct = if vm.morale_max > 0.0 {
        (vm.morale / vm.morale_max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let disp_pct = vm
        .negotiation
        .as_ref()
        .map(|neg| ((neg.disposition + 100.0) / 2.0).clamp(0.0, 100.0))
        .unwrap_or(0.0);

    rsx! {
        div { class: "panel surface",
            p { class: "eyebrow", "Surface \u{00b7} {vm.biome}" }
            // Suit integrity gauge
            p { class: "eyebrow", style: "margin-top: .6rem;", "Suit integrity" }
            div { class: "suit-gauge", i { style: "width: {suit_pct}%" } }
            div { class: "muted", style: "font-size: .75rem; margin: .15rem 0 .4rem;",
                "{vm.suit:.0} / {vm.suit_max:.0} \u{00b7} {vm.encounter} (depth {vm.depth})"
            }
            if busy { div { class: "muted", "Crossing the ground\u{2026} {vm.traverse}s" } }
            // Satchel
            p { class: "eyebrow", style: "margin-top: .7rem;", "Satchel" }
            if vm.satchel.is_empty() { div { class: "muted", "empty" } }
            for (name, amt) in vm.satchel.clone() {
                div { class: "satchel-row", span { "{name}" } span { class: "num", "{amt:.0}" } }
            }
            // Sub-states: combat → negotiation → pending-choice → node-graph map (order unchanged)
            if let Some(c) = vm.combat.clone() {
                div { class: "combat",
                    div { class: "combat-title", "Contact \u{2014} Remnant" }
                    div { class: "muted", "Enemy hull {c.enemy_hp:.0}" }
                    div { class: "bar bar-enemy", span { style: "width: {(c.enemy_hp).clamp(0.0,100.0)}%" } }
                    div { class: "muted",
                        "{drift_word(DriftSlot::AwayTeam, tier)} \u{00d7}{vm.away_crew} \u{00b7} HP {vm.squad_hp:.0}/{vm.squad_hp_max:.0} \u{00b7} morale {vm.morale:.0}"
                    }
                    div { class: "bar bar-hp", span { style: "width: {squad_hp_pct}%" } }
                    div { class: "bar bar-morale", span { style: "width: {morale_pct}%" } }
                    if c.shield_ticks > 0 { div { class: "muted", "Shield {c.shield_ticks}s" } }
                    div { class: "combat-actions",
                        button { class: "mini", onclick: move |_| on_stance.call(()), "Stance: {c.stance}" }
                        for (kind, label, count) in vm.consumables.clone() {
                            button { class: "mini", disabled: count == 0, onclick: move |_| on_use_consumable.call(kind), "{label} ({count})" }
                        }
                        button { class: "act", onclick: move |_| on_flee.call(()), "Flee" }
                    }
                }
            } else if let Some(neg) = vm.negotiation.clone() {
                div { class: "neg",
                    div { class: "neg-title", "Contact \u{2014} {neg.kind}" }
                    div { class: "neg-band band-{neg.band}", "Disposition: {neg.band} ({neg.disposition:.0})" }
                    div { class: "bar bar-disp", span { style: "width: {disp_pct}%" } }
                    div { class: "muted",
                        if neg.is_peer { "It moves the way you move. It is deciding about you, too." }
                        else { "Native life. Wary, watching." }
                    }
                    div { class: "neg-actions",
                        button { class: "mini", disabled: !neg.can_parley, onclick: move |_| on_parley.call(()), "Parley" }
                        if neg.is_peer {
                            button { class: "mini", disabled: !neg.can_trade, onclick: move |_| on_trade.call(()), "Trade" }
                        }
                        button { class: "mini", disabled: !neg.can_gift, onclick: move |_| on_gift.call(()), "Gift" }
                        if !neg.is_peer {
                            button { class: "mini", disabled: !neg.can_observe, onclick: move |_| on_observe.call(()), "Observe" }
                            button { class: "mini", disabled: !neg.can_harvest, onclick: move |_| on_harvest_fauna.call(()), "{drift_word(DriftSlot::Take, tier)}" }
                        }
                        button { class: "act", onclick: move |_| on_avoid.call(()), "Avoid" }
                        button { class: "act", onclick: move |_| on_attack_alien.call(()), "Attack" }
                    }
                }
            } else if vm.pending_choice {
                div { class: "surf-choice",
                    div { class: "muted", "Something living. It has not noticed you yet." }
                    button { class: "act", onclick: move |_| on_harvest.call(()), "{drift_word(DriftSlot::Take, tier)} it" }
                    button { class: "act ghost", onclick: move |_| on_leave.call(()), "Leave it" }
                }
            } else {
                // SVG node-graph map — scrollable, pixel-sized to content.
                // live-tinted: surf-current uses var(--accent) via CSS
                div { class: "surfmap-scroll",
                svg {
                    class: "surfmap",
                    view_box: "{view}",
                    width: "{svg_w}",
                    height: "{svg_h}",
                    // Edges — draw between placed nodes that share an edge.
                    for (node, x1v, y1v) in placed.clone() {
                        for eid in node.edges.clone() {
                            if let Some((x2v, y2v)) = pos_map.get(&eid) {
                                line {
                                    x1: "{x1v}",
                                    y1: "{y1v}",
                                    x2: "{x2v}",
                                    y2: "{y2v}",
                                    class: "surf-edge",
                                }
                            }
                        }
                    }
                    // Node circles.
                    for (node, xv, yv) in placed.clone() {
                        {
                            let id = node.id;
                            let reachable = cur_edges.contains(&id);
                            let node_class = {
                                let mut c = "surf-node".to_string();
                                if node.is_current {
                                    c.push_str(" surf-current");
                                } else if reachable {
                                    c.push_str(" surf-reach");
                                } else if node.resolved {
                                    c.push_str(" surf-done");
                                } else {
                                    c.push_str(" surf-far");
                                }
                                if node.kind == "Hazard" {
                                    c.push_str(" kind-Hazard");
                                }
                                c
                            };
                            let r_val = if node.is_current { "4" } else { "3" };
                            rsx! {
                                circle {
                                    cx: "{xv}",
                                    cy: "{yv}",
                                    r: "{r_val}",
                                    class: "{node_class}",
                                    onclick: move |_| {
                                        if reachable && !busy {
                                            on_move.call(id);
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
                } // end surfmap-scroll div
                if cur_edges.is_empty() {
                    div { class: "muted", "No reachable nodes. Extract." }
                }
            }
            button { class: "act ghost", onclick: move |_| on_extract.call(()), "Extract to orbit" }
        }
    }
}
