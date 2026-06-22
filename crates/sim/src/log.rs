//! The event log — terse, tick-stamped narration. A bounded ring buffer; the
//! oldest entries drop once `capacity` is exceeded.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub tick: u64,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Log {
    entries: VecDeque<LogEntry>,
}

impl Log {
    /// Append an entry, dropping the oldest if over `capacity`.
    pub fn push(&mut self, tick: u64, text: impl Into<String>, capacity: usize) {
        self.entries.push_back(LogEntry {
            tick,
            text: text.into(),
        });
        while self.entries.len() > capacity {
            self.entries.pop_front();
        }
    }

    pub fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_appends_in_order() {
        let mut log = Log::default();
        log.push(1, "a", 10);
        log.push(2, "b", 10);
        let v: Vec<_> = log.entries().iter().map(|e| e.text.as_str()).collect();
        assert_eq!(v, vec!["a", "b"]);
        assert_eq!(log.entries()[0].tick, 1);
    }

    #[test]
    fn ring_buffer_drops_oldest_past_capacity() {
        let mut log = Log::default();
        for i in 0..5 {
            log.push(i, format!("e{i}"), 3);
        }
        assert_eq!(log.entries().len(), 3);
        let v: Vec<_> = log.entries().iter().map(|e| e.text.as_str()).collect();
        assert_eq!(v, vec!["e2", "e3", "e4"]);
    }
}
