//! The Codex — an append-only archive of discoveries (completed-tech lore + the
//! triggered "uneasy notes"). Dedup by id so each entry lands once.

use crate::research::TechId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NoteId {
    FirstCrewDeath,
    JumpTheoryGlimpse,
    HeavyConsumption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DiscoveryId {
    Tech(TechId),
    Note(NoteId),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Discovery {
    pub id: DiscoveryId,
    pub title: String,
    pub text: String,
    pub tick: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Codex {
    pub entries: Vec<Discovery>,
}

impl Codex {
    pub fn has(&self, id: DiscoveryId) -> bool {
        self.entries.iter().any(|d| d.id == id)
    }

    /// Append `d` unless an entry with its id already exists. Returns true if added.
    pub fn add(&mut self, d: Discovery) -> bool {
        if self.has(d.id) {
            false
        } else {
            self.entries.push(d);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: NoteId) -> Discovery {
        Discovery {
            id: DiscoveryId::Note(id),
            title: "t".into(),
            text: "x".into(),
            tick: 1,
        }
    }

    #[test]
    fn add_dedups_by_id() {
        let mut c = Codex::default();
        assert!(c.add(note(NoteId::FirstCrewDeath)));
        assert!(!c.add(note(NoteId::FirstCrewDeath))); // dup
        assert_eq!(c.entries.len(), 1);
        assert!(c.has(DiscoveryId::Note(NoteId::FirstCrewDeath)));
        assert!(!c.has(DiscoveryId::Note(NoteId::JumpTheoryGlimpse)));
    }
}
