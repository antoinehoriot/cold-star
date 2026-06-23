//! The player-orderable power-consumer priority list. Lower index = higher
//! priority (pauses last under brownout).

use crate::job::JobId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Consumer {
    Atmosphere,
    Thermal,
    Job(JobId),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Priority(pub Vec<Consumer>);

impl Priority {
    pub fn rank(&self, c: Consumer) -> Option<usize> {
        self.0.iter().position(|&x| x == c)
    }

    /// Append any consumers not already present, in the given order.
    pub fn ensure_contains(&mut self, consumers: &[Consumer]) {
        for &c in consumers {
            if self.rank(c).is_none() {
                self.0.push(c);
            }
        }
    }

    /// Move `c` one slot toward higher priority (index 0).
    pub fn move_up(&mut self, c: Consumer) {
        if let Some(i) = self.rank(c)
            && i > 0
        {
            self.0.swap(i, i - 1);
        }
    }

    /// Move `c` one slot toward lower priority.
    pub fn move_down(&mut self, c: Consumer) {
        if let Some(i) = self.rank(c)
            && i + 1 < self.0.len()
        {
            self.0.swap(i, i + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_contains_appends_missing_only() {
        let mut p = Priority::default();
        p.ensure_contains(&[Consumer::Atmosphere, Consumer::Job(JobId::Refiner)]);
        p.ensure_contains(&[Consumer::Atmosphere, Consumer::Job(JobId::Fabricator)]);
        assert_eq!(
            p.0,
            vec![
                Consumer::Atmosphere,
                Consumer::Job(JobId::Refiner),
                Consumer::Job(JobId::Fabricator),
            ]
        );
    }

    #[test]
    fn move_up_and_down_swap_neighbors() {
        let mut p = Priority(vec![
            Consumer::Atmosphere,
            Consumer::Job(JobId::Refiner),
            Consumer::Job(JobId::Fabricator),
        ]);
        p.move_up(Consumer::Job(JobId::Fabricator));
        assert_eq!(p.rank(Consumer::Job(JobId::Fabricator)), Some(1));
        p.move_down(Consumer::Job(JobId::Fabricator));
        assert_eq!(p.rank(Consumer::Job(JobId::Fabricator)), Some(2));
        // Edges are no-ops.
        p.move_up(Consumer::Atmosphere);
        assert_eq!(p.rank(Consumer::Atmosphere), Some(0));
    }
}
