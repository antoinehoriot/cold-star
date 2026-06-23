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

impl Galaxy {
    /// Generate the whole star map deterministically. System 0 is home:
    /// fixed at the origin, Main-sequence. Systems 1.. draw an independent
    /// position + star type from a sub-seeded stream so each is reproducible.
    pub fn generate(world_seed: u64, t: &Tuning) -> Galaxy {
        let mut systems = Vec::with_capacity(t.galaxy_system_count as usize);
        systems.push(System {
            id: HOME,
            pos: (0.0, 0.0),
            star_type: StarType::MainSequence,
        });
        for id in 1..t.galaxy_system_count {
            let mut rng = Rng::new(hash(world_seed, id));
            // Uniform point in a disc: r = R*sqrt(u), theta = 2*pi*v.
            let r = t.galaxy_radius * rng.next_f64().sqrt();
            let theta = std::f64::consts::TAU * rng.next_f64();
            let pos = (r * theta.cos(), r * theta.sin());
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
pub enum SiteKind {
    AsteroidField,
    Derelict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub kind: SiteKind,
    pub depleted: bool,
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
pub fn materialize_sites(world_seed: u64, sys_id: u32, star: StarType, _t: &Tuning) -> Vec<Site> {
    let mut rng = Rng::new(hash(world_seed, sys_id ^ SITE_SALT));
    // Per star type: (p_asteroid, p_derelict) chance for each of two site rolls.
    let (p_ast, p_der) = match star {
        StarType::Neutron => (0.8, 0.2),
        StarType::Dead => (0.15, 0.8),
        StarType::MainSequence => (0.4, 0.3),
        StarType::Dwarf => (0.3, 0.25),
    };
    let mut sites = Vec::new();
    for _ in 0..2 {
        let roll = rng.next_f64();
        if roll < p_ast {
            sites.push(Site {
                kind: SiteKind::AsteroidField,
                depleted: false,
            });
        } else if roll < p_ast + p_der {
            sites.push(Site {
                kind: SiteKind::Derelict,
                depleted: false,
            });
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
        assert!(a.len() <= 2);
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
}
