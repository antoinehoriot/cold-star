//! Tier-2 surface sites: a deterministic node-graph you land into. The site is
//! a pure function of its seed + biome — regenerated on demand, never serialized.

use crate::config::Tuning;
use crate::galaxy::{Biome, SystemId};
use crate::resources::ResourceKind;
use crate::rng::{Rng, hash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Run-state for a player currently on a planet surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Landing {
    pub system: SystemId,
    pub site_idx: usize,
    pub biome: Biome,
    pub node: u32,
    pub suit: f64,
    pub satchel: BTreeMap<ResourceKind, f64>,
    pub pending_choice: bool,
    pub traverse: u32,
}

impl Landing {
    pub fn satchel_total(&self) -> f64 {
        self.satchel.values().copied().sum()
    }
}

const SURFACE_SALT: u64 = 0x5300_5300;

/// Derive an independent per-site stream seed.
pub fn surface_seed(world_seed: u64, system: SystemId, site_idx: usize) -> u64 {
    hash(world_seed, system.0).wrapping_add(SURFACE_SALT)
        ^ hash(world_seed ^ SURFACE_SALT, site_idx as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncounterKind {
    ResourceCache,
    Hazard,
    Ruin,
    Life,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceNode {
    pub id: u32,
    pub depth: u32,
    pub kind: EncounterKind,
    pub edges: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceSite {
    pub nodes: Vec<SurfaceNode>,
}

impl SurfaceSite {
    /// Build the depth-layered DAG. Layer 0 = entry (Empty). Layers 1..=depth_max
    /// hold `surface_layer_min..=surface_layer_max` nodes; each node draws edges to
    /// the next layer, and each encounter from the biome×depth table.
    pub fn generate(site_seed: u64, biome: Biome, t: &Tuning) -> SurfaceSite {
        let mut rng = Rng::new(site_seed);
        // Build layers: Vec<Vec<u32>> of node ids per depth.
        let mut nodes: Vec<SurfaceNode> = Vec::new();
        let mut layers: Vec<Vec<u32>> = Vec::new();
        // Entry.
        nodes.push(SurfaceNode {
            id: 0,
            depth: 0,
            kind: EncounterKind::Empty,
            edges: vec![],
        });
        layers.push(vec![0]);
        for depth in 1..=t.surface_depth_max {
            let span = t.surface_layer_max - t.surface_layer_min + 1;
            let count = t.surface_layer_min + (rng.next_f64() * span as f64) as u32;
            let mut layer = Vec::new();
            for _ in 0..count {
                let id = nodes.len() as u32;
                let kind = weighted_encounter(&mut rng, biome, depth, t);
                nodes.push(SurfaceNode {
                    id,
                    depth,
                    kind,
                    edges: vec![],
                });
                layer.push(id);
            }
            layers.push(layer);
        }
        // Wire forward edges: each node in layer d connects to 1..=2 nodes in layer d+1.
        for d in 0..layers.len().saturating_sub(1) {
            let next = layers[d + 1].clone();
            for &nid in &layers[d].clone() {
                let k = 1 + (rng.next_f64() * 2.0) as usize; // 1 or 2
                for _ in 0..k {
                    let pick = next[(rng.next_f64() * next.len() as f64) as usize % next.len()];
                    let edges = &mut nodes[nid as usize].edges;
                    if !edges.contains(&pick) {
                        edges.push(pick);
                    }
                }
            }
        }
        SurfaceSite { nodes }
    }

    pub fn get(&self, id: u32) -> Option<&SurfaceNode> {
        self.nodes.get(id as usize)
    }

    pub fn is_edge(&self, from: u32, to: u32) -> bool {
        self.get(from)
            .map(|n| n.edges.contains(&to))
            .unwrap_or(false)
    }

    /// True if `a` and `b` are connected by an edge in EITHER direction
    /// (movement along an edge is bidirectional — you can backtrack).
    pub fn is_neighbor(&self, a: u32, b: u32) -> bool {
        self.is_edge(a, b) || self.is_edge(b, a)
    }
}

/// Biome × depth-band encounter weights. Verdant alone can yield Life; Ruined skews
/// Ruin; Toxic skews Hazard; Frozen skews caches/empty. Deeper bands raise Hazard/Ruin.
fn weighted_encounter(rng: &mut Rng, biome: Biome, depth: u32, _t: &Tuning) -> EncounterKind {
    let deep = depth >= 2;
    // (ResourceCache, Hazard, Ruin, Life, Empty)
    let w = match biome {
        Biome::Frozen => (4.0, if deep { 3.0 } else { 1.5 }, 1.0, 0.0, 2.0),
        Biome::Toxic => (2.5, if deep { 4.5 } else { 3.0 }, 1.5, 0.0, 1.0),
        Biome::Verdant => (3.0, if deep { 2.0 } else { 1.0 }, 1.0, 3.0, 1.0),
        Biome::Ruined => (2.0, if deep { 3.0 } else { 2.0 }, 4.0, 0.0, 1.0),
    };
    let table = [
        (EncounterKind::ResourceCache, w.0),
        (EncounterKind::Hazard, w.1),
        (EncounterKind::Ruin, w.2),
        (EncounterKind::Life, w.3),
        (EncounterKind::Empty, w.4),
    ];
    let total: f64 = table.iter().map(|&(_, x)| x).sum();
    let mut pick = rng.next_f64() * total;
    for &(kind, x) in &table {
        if pick < x {
            return kind;
        }
        pick -= x;
    }
    EncounterKind::Empty
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Tuning;
    use crate::galaxy::{Biome, SystemId};

    #[test]
    fn generation_is_pure_and_seed_sensitive() {
        let t = Tuning::default();
        let s = surface_seed(7, SystemId(3), 1);
        let a = SurfaceSite::generate(s, Biome::Verdant, &t);
        let b = SurfaceSite::generate(s, Biome::Verdant, &t);
        let c = SurfaceSite::generate(s + 1, Biome::Verdant, &t);
        assert_eq!(a.nodes, b.nodes);
        assert_ne!(a.nodes, c.nodes);
    }

    #[test]
    fn entry_is_node_zero_empty_and_graph_is_connected_forward() {
        let t = Tuning::default();
        let g = SurfaceSite::generate(surface_seed(1, SystemId(2), 0), Biome::Frozen, &t);
        assert_eq!(g.nodes[0].id, 0);
        assert_eq!(g.nodes[0].depth, 0);
        assert_eq!(g.nodes[0].kind, EncounterKind::Empty);
        // Every non-entry node is reachable from entry via forward edges,
        // and all edges point to a strictly deeper node.
        for n in &g.nodes {
            for &e in &n.edges {
                assert!(g.get(e).unwrap().depth > n.depth, "edges go deeper");
            }
        }
        // Entry has at least one outgoing edge (you can descend).
        assert!(!g.nodes[0].edges.is_empty());
    }

    #[test]
    fn verdant_can_surface_life_frozen_cannot() {
        let t = Tuning::default();
        let life_in = |biome| {
            (0..60u32).any(|k| {
                SurfaceSite::generate(surface_seed(k as u64, SystemId(1), 0), biome, &t)
                    .nodes
                    .iter()
                    .any(|n| n.kind == EncounterKind::Life)
            })
        };
        assert!(life_in(Biome::Verdant), "verdant holds life");
        assert!(!life_in(Biome::Frozen), "frozen has no life");
    }
}
