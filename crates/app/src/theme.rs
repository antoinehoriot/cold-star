/// Word-drift slots whose label cools as expansion rises (cold vocab shared across origins).
#[derive(Clone, Copy)]
pub(crate) enum DriftSlot {
    AwayTeam,
    Take,
    Salvage,
    FeedCore,
}

/// The cold-vocabulary table: [T0, T1, T2, T3] per slot.
pub(crate) fn drift_word(slot: DriftSlot, tier: u8) -> &'static str {
    let row: [&str; 4] = match slot {
        DriftSlot::AwayTeam => ["Marines", "the team", "the detail", "the asset"],
        DriftSlot::Take => ["Harvest", "Extract", "Process", "Strip"],
        DriftSlot::Salvage => ["Salvage", "Recover", "Reclaim", "Render"],
        DriftSlot::FeedCore => [
            "Feed the core",
            "Fuel the core",
            "Charge the core",
            "Burn it",
        ],
    };
    row[(tier as usize).min(3)]
}

/// The crew noun: the origin's warm word at T0, then shared cold words.
pub(crate) fn crew_word(tier: u8, origin_noun: &str) -> String {
    match tier {
        0 => origin_noun.to_string(),
        1 => "crew".into(),
        2 => "units".into(),
        _ => "yield".into(),
    }
}

/// Emberfall per-tier palette stops (ember → ice). Returns (accent, text, warm-hue).
pub(crate) fn emberfall_stops(tier: u8) -> (&'static str, &'static str, &'static str) {
    match tier.min(3) {
        0 => ("#e89255", "#d9cdbf", "22"),
        1 => ("#cbb079", "#d2cdc6", "16"),
        2 => ("#9ec3d0", "#c9d1d6", "10"),
        _ => ("#c2e2f2", "#c3d2dd", "6"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drift_word_per_slot_and_tier() {
        // Take slot
        assert_eq!(drift_word(DriftSlot::Take, 0), "Harvest");
        assert_eq!(drift_word(DriftSlot::Take, 1), "Extract");
        assert_eq!(drift_word(DriftSlot::Take, 2), "Process");
        assert_eq!(drift_word(DriftSlot::Take, 3), "Strip");
        // Clamp at 3
        assert_eq!(drift_word(DriftSlot::Take, 4), "Strip");
        assert_eq!(drift_word(DriftSlot::Take, 255), "Strip");

        // Salvage slot
        assert_eq!(drift_word(DriftSlot::Salvage, 0), "Salvage");
        assert_eq!(drift_word(DriftSlot::Salvage, 3), "Render");

        // FeedCore slot
        assert_eq!(drift_word(DriftSlot::FeedCore, 0), "Feed the core");
        assert_eq!(drift_word(DriftSlot::FeedCore, 3), "Burn it");

        // AwayTeam slot
        assert_eq!(drift_word(DriftSlot::AwayTeam, 0), "Marines");
        assert_eq!(drift_word(DriftSlot::AwayTeam, 3), "the asset");
    }

    #[test]
    fn crew_word_origin_at_t0_cold_at_higher() {
        assert_eq!(crew_word(0, "survivor"), "survivor");
        assert_eq!(crew_word(1, "survivor"), "crew");
        assert_eq!(crew_word(2, "survivor"), "units");
        assert_eq!(crew_word(3, "survivor"), "yield");
        // Clamp beyond 3 still returns "yield"
        assert_eq!(crew_word(4, "survivor"), "yield");
    }
}
