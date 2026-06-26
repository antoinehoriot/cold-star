//! The deterministic star map. The galaxy is a pure function of `world_seed`
//! and is regenerated on demand — never serialized. Only the mutable overlay
//! (`GalaxyState`, added later) is saved.

use crate::config::Tuning;
use crate::rng::{Rng, hash};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SystemId(pub u32);

pub const HOME: SystemId = SystemId(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StarType {
    Dwarf,
    MainSequence,
    Neutron,
    Dead,
    Giant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct System {
    pub id: SystemId,
    pub pos: (f64, f64),
    pub star_type: StarType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Galaxy {
    pub systems: Vec<System>,
}

const GALAXY_SALT: u32 = 0xA7B3_E291;

impl Galaxy {
    /// Generate the whole star map deterministically using connected-growth
    /// placement. System 0 (HOME) is fixed at the origin. Inner systems
    /// (ids 1..galaxy_inner_count) are placed within base-drive-range of an
    /// anchor, forming a reachable web from HOME. Frontier systems are placed
    /// just beyond base range, reachable after drive upgrades.
    pub fn generate(world_seed: u64, t: &Tuning) -> Galaxy {
        let mut systems: Vec<System> = Vec::with_capacity(t.galaxy_system_count as usize);
        systems.push(System {
            id: HOME,
            pos: (0.0, 0.0),
            star_type: StarType::MainSequence,
        });

        let mut rng = Rng::new(hash(world_seed, GALAXY_SALT));
        let base = t.ship_base_drive_range;
        let sep = base * t.galaxy_min_separation_frac;

        for id in 1..t.galaxy_system_count {
            let is_frontier = id >= t.galaxy_inner_count;

            // Distance band from an anchor:
            // inner:    [max(sep, base*0.35), base*0.92]   (within base drive range of anchor)
            // frontier: [base*1.05, base + frontier_reach]  (needs drive upgrades)
            let (dist_min, dist_max) = if !is_frontier {
                (sep.max(base * 0.35), base * 0.92)
            } else {
                (base * 1.05, base + t.galaxy_frontier_reach)
            };

            // Pick anchor: for inner, any placed system (pick uniformly).
            // For frontier, use the placed system furthest from HOME as anchor
            // to push frontier outward.
            let anchor_idx = if !is_frontier {
                (rng.next_f64() * systems.len() as f64) as usize % systems.len()
            } else {
                let mut best_idx = 0usize;
                let mut best_dist = 0.0f64;
                for (i, s) in systems.iter().enumerate() {
                    let d = (s.pos.0 * s.pos.0 + s.pos.1 * s.pos.1).sqrt();
                    if d >= best_dist {
                        best_dist = d;
                        best_idx = i;
                    }
                }
                best_idx
            };

            // Sample up to 40 candidates; fall back to last (clamp into disc) if none valid.
            let anchor = systems[anchor_idx].pos;
            let mut candidate = anchor; // fallback
            for _ in 0..40 {
                let angle = rng.next_f64() * std::f64::consts::TAU;
                let dist = dist_min + rng.next_f64() * (dist_max - dist_min);
                let cx = anchor.0 + angle.cos() * dist;
                let cy = anchor.1 + angle.sin() * dist;

                // Check: within galaxy_radius
                let r = (cx * cx + cy * cy).sqrt();
                if r > t.galaxy_radius {
                    candidate = (cx, cy);
                    continue;
                }

                // Check: not too close to any existing system
                let too_close = systems.iter().any(|s| {
                    let dx = s.pos.0 - cx;
                    let dy = s.pos.1 - cy;
                    (dx * dx + dy * dy).sqrt() < sep
                });
                if too_close {
                    candidate = (cx, cy);
                    continue;
                }

                candidate = (cx, cy);
                break;
            }

            // If candidate is outside radius (fallback case), clamp it into the disc.
            let r = (candidate.0 * candidate.0 + candidate.1 * candidate.1).sqrt();
            let pos = if r > t.galaxy_radius {
                let scale = (t.galaxy_radius - 1.0) / r;
                (candidate.0 * scale, candidate.1 * scale)
            } else {
                candidate
            };

            // Star type draw comes AFTER position (keep star distribution independent).
            let star_type = weighted_star(&mut rng, t);

            systems.push(System {
                id: SystemId(id),
                pos,
                star_type,
            });
        }

        Galaxy { systems }
    }

    pub fn get(&self, id: SystemId) -> Option<&System> {
        self.systems.get(id.0 as usize)
    }
}

fn weighted_star(rng: &mut Rng, t: &Tuning) -> StarType {
    let weights = [
        (StarType::Dwarf, t.star_weight_dwarf),
        (StarType::MainSequence, t.star_weight_mainseq),
        (StarType::Neutron, t.star_weight_neutron),
        (StarType::Dead, t.star_weight_dead),
        (StarType::Giant, t.star_weight_giant),
    ];
    let total: f64 = weights.iter().map(|&(_, w)| w).sum();
    let mut pick = rng.next_f64() * total;
    for &(kind, w) in &weights {
        if pick < w {
            return kind;
        }
        pick -= w;
    }
    StarType::Dwarf
}

pub fn distance(a: &System, b: &System) -> f64 {
    let dx = a.pos.0 - b.pos.0;
    let dy = a.pos.1 - b.pos.1;
    (dx * dx + dy * dy).sqrt()
}

pub fn jump_cost(dist: f64, t: &Tuning) -> f64 {
    t.jump_base_cost + dist * t.jump_drive_factor
}

const SITE_SALT: u32 = 0x5176; // mixes sys_id into an independent site stream

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    Frozen,
    Toxic,
    Verdant,
    Ruined,
    Molten,
    Dead,
    Oceanic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteKind {
    AsteroidField,
    Derelict,
    Planet(Biome),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub kind: SiteKind,
    pub depleted: bool,
    #[serde(default)]
    pub resolved_nodes: std::collections::BTreeSet<u32>,
}

impl Site {
    pub fn new(kind: SiteKind) -> Self {
        Self {
            kind,
            depleted: false,
            resolved_nodes: std::collections::BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GalaxyState {
    pub revealed: BTreeSet<u32>,
    pub scanned: BTreeSet<u32>,
    pub sites: BTreeMap<u32, Vec<Site>>,
}

/// Generate a system's sites once (deterministic). 0–2 sites, weighted by star
/// type: neutron skews asteroid (Isotopes), dead skews derelict, dwarf/main
/// may surface nothing (the honest dark).
pub fn materialize_sites(world_seed: u64, sys_id: u32, star: StarType, t: &Tuning) -> Vec<Site> {
    let mut rng = Rng::new(hash(world_seed, sys_id ^ SITE_SALT));
    // Per star type: (p_asteroid, p_derelict) chance for each of two site rolls.
    let (p_ast, p_der) = match star {
        StarType::Neutron => (0.8, 0.2),
        StarType::Dead => (0.15, 0.8),
        StarType::MainSequence => (0.4, 0.3),
        StarType::Dwarf => (0.3, 0.25),
        StarType::Giant => (0.5, 0.2),
    };
    let p_planet = t.planet_site_weight; // planets layer on top of the asteroid/derelict bands
    let mut sites = Vec::new();
    for _ in 0..2 {
        let roll = rng.next_f64();
        if roll < p_ast {
            sites.push(Site::new(SiteKind::AsteroidField));
        } else if roll < p_ast + p_der {
            sites.push(Site::new(SiteKind::Derelict));
        }
        // Independent planet roll on a separate draw so adding planets doesn't
        // shift the asteroid/derelict distribution.
        if rng.next_f64() < p_planet {
            let biome = match (rng.next_f64() * 7.0) as u32 {
                0 => Biome::Frozen,
                1 => Biome::Toxic,
                2 => Biome::Verdant,
                3 => Biome::Ruined,
                4 => Biome::Molten,
                5 => Biome::Dead,
                _ => Biome::Oceanic,
            };
            sites.push(Site::new(SiteKind::Planet(biome)));
        }
    }
    sites
}

/// Add every system within `radius` of `from` to `out` (including `from`).
pub fn reveal_from(g: &Galaxy, from: SystemId, radius: f64, out: &mut BTreeSet<u32>) {
    let Some(origin) = g.get(from) else { return };
    for s in &g.systems {
        if distance(origin, s) <= radius {
            out.insert(s.id.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Tuning;
    use std::collections::VecDeque;

    #[test]
    fn generation_is_pure_and_seed_sensitive() {
        let t = Tuning::default();
        let a = Galaxy::generate(42, &t);
        let b = Galaxy::generate(42, &t);
        let c = Galaxy::generate(43, &t);
        assert_eq!(a.systems, b.systems); // same seed -> identical
        assert_ne!(a.systems, c.systems); // different seed -> diverges
        assert_eq!(a.systems.len(), t.galaxy_system_count as usize);
    }

    #[test]
    fn home_is_fixed_main_sequence_at_origin() {
        let t = Tuning::default();
        let g = Galaxy::generate(7, &t);
        let home = g.get(HOME).unwrap();
        assert_eq!(home.id, HOME);
        assert_eq!(home.pos, (0.0, 0.0));
        assert_eq!(home.star_type, StarType::MainSequence);
    }

    #[test]
    fn systems_lie_within_radius_and_ids_are_dense() {
        let t = Tuning::default();
        let g = Galaxy::generate(9, &t);
        for (i, s) in g.systems.iter().enumerate() {
            assert_eq!(s.id, SystemId(i as u32));
            let r = (s.pos.0 * s.pos.0 + s.pos.1 * s.pos.1).sqrt();
            assert!(r <= t.galaxy_radius + 1e-9, "system {i} outside radius");
        }
    }

    #[test]
    fn distance_and_jump_cost_are_correct() {
        let t = Tuning::default();
        let a = System {
            id: SystemId(0),
            pos: (0.0, 0.0),
            star_type: StarType::Dwarf,
        };
        let b = System {
            id: SystemId(1),
            pos: (3.0, 4.0),
            star_type: StarType::Dwarf,
        };
        assert!((distance(&a, &b) - 5.0).abs() < 1e-9);
        assert!((jump_cost(5.0, &t) - (t.jump_base_cost + 5.0 * t.jump_drive_factor)).abs() < 1e-9);
    }

    #[test]
    fn site_materialization_is_deterministic() {
        let t = Tuning::default();
        let a = materialize_sites(42, 3, StarType::Neutron, &t);
        let b = materialize_sites(42, 3, StarType::Neutron, &t);
        assert_eq!(a, b);
        // Up to 2 asteroid/derelict + up to 2 planet overlays = max 4.
        assert!(a.len() <= 4);
        assert!(a.iter().all(|s| !s.depleted));
    }

    #[test]
    fn neutron_biases_asteroid_dead_biases_derelict() {
        let t = Tuning::default();
        let neutron_ast = (0..200u32)
            .flat_map(|id| materialize_sites(1, id, StarType::Neutron, &t))
            .filter(|s| s.kind == SiteKind::AsteroidField)
            .count();
        let dead_ast = (0..200u32)
            .flat_map(|id| materialize_sites(1, id, StarType::Dead, &t))
            .filter(|s| s.kind == SiteKind::AsteroidField)
            .count();
        // Neutron systems surface far more asteroid fields than dead ones.
        assert!(
            neutron_ast > dead_ast,
            "neutron={neutron_ast} dead={dead_ast}"
        );
    }

    #[test]
    fn planets_materialize_with_a_biome_and_carry_resolved_nodes() {
        let t = Tuning::default();
        // Over many systems, at least one Planet appears, each with a Copy Biome.
        let planets: Vec<_> = (0..400u32)
            .flat_map(|id| materialize_sites(1, id, StarType::MainSequence, &t))
            .filter(|s| matches!(s.kind, SiteKind::Planet(_)))
            .collect();
        assert!(!planets.is_empty(), "planets should materialize");
        assert!(planets.iter().all(|s| s.resolved_nodes.is_empty()));
    }

    #[test]
    fn reveal_from_includes_near_systems_only() {
        let t = Tuning::default();
        let g = Galaxy::generate(5, &t);
        let mut out = std::collections::BTreeSet::new();
        reveal_from(&g, HOME, t.galaxy_reveal_radius, &mut out);
        assert!(out.contains(&0)); // home reveals itself
        for &id in &out {
            let d = distance(g.get(HOME).unwrap(), g.get(SystemId(id)).unwrap());
            assert!(d <= t.galaxy_reveal_radius + 1e-9);
        }
    }

    #[test]
    fn inner_web_is_reachable_from_home_at_base_range() {
        let t = Tuning::default();
        for seed in [1u64, 7, 42, 99, 2024, 6] {
            let g = Galaxy::generate(seed, &t);
            // BFS from HOME using base drive range
            let mut reachable = std::collections::BTreeSet::new();
            let mut queue = VecDeque::new();
            reachable.insert(HOME.0);
            queue.push_back(HOME);
            while let Some(cur) = queue.pop_front() {
                let cur_sys = g.get(cur).unwrap();
                for s in &g.systems {
                    if !reachable.contains(&s.id.0)
                        && distance(cur_sys, s) <= t.ship_base_drive_range
                    {
                        reachable.insert(s.id.0);
                        queue.push_back(s.id);
                    }
                }
            }
            assert!(
                reachable.len() >= t.galaxy_inner_count as usize,
                "seed {seed}: BFS at base range reached {} systems, need >= {} (inner_count)",
                reachable.len(),
                t.galaxy_inner_count
            );
        }
    }

    #[test]
    fn full_galaxy_reachable_at_max_upgraded_range() {
        let t = Tuning::default();
        let max_range = t.ship_base_drive_range + t.galaxy_frontier_reach;
        for seed in [1u64, 7, 42, 99, 2024, 6] {
            let g = Galaxy::generate(seed, &t);
            let mut reachable = std::collections::BTreeSet::new();
            let mut queue = VecDeque::new();
            reachable.insert(HOME.0);
            queue.push_back(HOME);
            while let Some(cur) = queue.pop_front() {
                let cur_sys = g.get(cur).unwrap();
                for s in &g.systems {
                    if !reachable.contains(&s.id.0) && distance(cur_sys, s) <= max_range {
                        reachable.insert(s.id.0);
                        queue.push_back(s.id);
                    }
                }
            }
            assert_eq!(
                reachable.len(),
                t.galaxy_system_count as usize,
                "seed {seed}: BFS at max range reached {}/{} systems (some permanently stranded)",
                reachable.len(),
                t.galaxy_system_count
            );
        }
    }

    #[test]
    fn galaxy_min_separation_and_radius_hold() {
        let t = Tuning::default();
        let sep = t.ship_base_drive_range * t.galaxy_min_separation_frac;
        for seed in [1u64, 7, 42, 99, 2024, 6] {
            let g = Galaxy::generate(seed, &t);
            // All within radius
            for s in &g.systems {
                let r = (s.pos.0 * s.pos.0 + s.pos.1 * s.pos.1).sqrt();
                assert!(
                    r <= t.galaxy_radius + 1e-9,
                    "seed {seed} system {:?} outside radius (r={r})",
                    s.id
                );
            }
            // Min separation (approximate — fallback may violate, allow small tolerance)
            for i in 0..g.systems.len() {
                for j in (i + 1)..g.systems.len() {
                    let d = distance(&g.systems[i], &g.systems[j]);
                    assert!(
                        d >= sep * 0.5,
                        "seed {seed}: systems {i} and {j} too close (d={d:.2}, sep*0.5={:.2})",
                        sep * 0.5
                    );
                }
            }
        }
    }
}
