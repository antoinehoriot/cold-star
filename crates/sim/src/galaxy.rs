//! The deterministic star map. The galaxy is a pure function of `world_seed`
//! and is regenerated on demand — never serialized. Only the mutable overlay
//! (`GalaxyState`, added later) is saved.

use crate::config::Tuning;
use crate::rng::{Rng, hash};
use serde::{Deserialize, Serialize};

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
}
