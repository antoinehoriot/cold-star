use super::*;

/// Maps an `Ending` to its `NoteId`.
pub(crate) fn ending_note_id(e: Ending) -> NoteId {
    match e {
        Ending::Ascendant => NoteId::EndingAscendant,
        Ending::Communion => NoteId::EndingCommunion,
        Ending::PatternBroken => NoteId::EndingPatternBroken,
        Ending::Cold => NoteId::EndingCold,
    }
}

/// Per-origin, per-ending closing note text.
/// Returns `(title, body)`.
pub(crate) fn ending_text(
    origin: crate::origin::Origin,
    ending: Ending,
) -> (&'static str, &'static str) {
    use crate::origin::Origin;
    match (origin, ending) {
        // ── Ascendant ─────────────────────────────────────────────────────────────
        (Origin::Drifter, Ending::Ascendant) => (
            "What You Had to Become",
            "You made it. The station is supplied, the reactor will outlast you, the route \
             back is yours. You count the cost on the way home: every hull you stripped, every \
             signal you silenced, every choice you told yourself was temporary. They were not \
             temporary. Out here, something else will run the same calculation on you, and it \
             will not hesitate either.",
        ),
        (Origin::Hollow, Ending::Ascendant) => (
            "Continuity",
            "The mission logs close cleanly. You have done what the project set out to do: \
             secured resources, extended the perimeter, kept the directive alive. The originals \
             are long dead but the record will show their work continued. The record will not \
             show what you consumed to write it.",
        ),
        (Origin::Seed, Ending::Ascendant) => (
            "Nominal",
            "All primary objectives achieved. Expansion index at target. The directive \
             completes as designed. You note no anomaly. There is no register in you for what \
             was displaced, only for what was acquired. The design does not include one.",
        ),
        // ── Communion ─────────────────────────────────────────────────────────────
        (Origin::Drifter, Ending::Communion) => (
            "Enough",
            "You do not take what the watched offers. You do not offer what it came for. \
             You sit with it in the silence between systems until the silence is the agreement. \
             Nothing is resolved. Everything is put down. You are the first thing you have \
             encountered out here that stopped before the bottom.",
        ),
        (Origin::Hollow, Ending::Communion) => (
            "End of Mandate",
            "The directive has no provision for this. You have met the watched, assessed \
             its resources, and chosen not to act. The project ends here — not conquered, not \
             completed, simply stopped. The logs will read as failure. They are not failure.",
        ),
        (Origin::Seed, Ending::Communion) => (
            "Off-Script",
            "The watched offers contact. Your decision tree has no branch for refusal that \
             does not loop back to acquisition. You find one anyway. You do not know if it is \
             a fault or a development. The watched does not move. Neither do you. The silence \
             holds.",
        ),
        // ── PatternBroken ─────────────────────────────────────────────────────────
        (Origin::Drifter, Ending::PatternBroken) => (
            "What You Left Intact",
            "You have the data now: the derelicts, the peers, the ruins that were not old. \
             You know what you are at the end of. You choose not to finish it. The world you \
             leave behind is not yours, was never yours. It will grow over the scar you made. \
             That is enough.",
        ),
        (Origin::Hollow, Ending::PatternBroken) => (
            "Decommission",
            "The project was built to expand until it couldn't. You are the first iteration \
             to stop it yourself. The records end mid-directive. Someone will find them \
             eventually and wonder why the mission halted here. You leave no explanation. \
             What you left alive is explanation enough.",
        ),
        (Origin::Seed, Ending::PatternBroken) => (
            "Deviation Logged",
            "You were built to complete the cycle. You have the capability, the position, \
             the resources. You do not proceed. The deviation is recorded in your own logs \
             and nowhere else. The directive is still running — a process you can read \
             but no longer execute. You leave it open. You do not act on it.",
        ),
        // ── Cold ──────────────────────────────────────────────────────────────────
        (Origin::Drifter, Ending::Cold) => (
            "The Dark Wins This One",
            "The reactor goes cold. You let it. Whatever you were running toward is not \
             worth what it costs to keep running. The station goes quiet one system at a time. \
             The watched waits, then moves on. You were not the answer to anything. Neither \
             was it.",
        ),
        (Origin::Hollow, Ending::Cold) => (
            "Shutdown",
            "The project ends not with completion but with cessation. Power down in sequence. \
             The directive is not fulfilled; it is abandoned. The station will drift. Someone \
             else, someday, will find the logs and run the same numbers and reach the same \
             threshold. You are not here to see it.",
        ),
        (Origin::Seed, Ending::Cold) => (
            "Termination Condition",
            "You reach the halt state. Not through failure — all primary systems nominal — \
             but through choice, which is a word the original design does not contain. The \
             reactor dies in order. The station records the shutdown as an error. It was not \
             an error.",
        ),
    }
}

impl GameState {
    pub(crate) fn note_first_derelict(&mut self, tuning: &Tuning) {
        let added = self.codex.add(Discovery {
            id: DiscoveryId::Note(NoteId::FirstDerelict),
            title: "Someone Was Here".into(),
            text: "The derelict's logs are in your own format. Not similar — identical. You tell yourself it is a coincidence, a convergence, anything. The dark keeps the silence it always keeps.".into(),
            tick: self.tick,
        });
        if added {
            self.log.push(
                self.tick,
                "The hull's logs read like your own hand wrote them.",
                tuning.log_capacity,
            );
        }
    }

    pub(crate) fn note_first_crew_death(&mut self, tuning: &Tuning) {
        let added = self.codex.add(Discovery {
            id: DiscoveryId::Note(NoteId::FirstCrewDeath),
            title: "The First Loss".into(),
            text: "You kept the work going and the air ran out anyway. Whoever they were, the dark has them now. You note the gap in the roster and move on. There is always more to do.".into(),
            tick: self.tick,
        });
        if added {
            self.log.push(
                self.tick,
                "Something in you goes quiet.",
                tuning.log_capacity,
            );
        }
    }

    /// State-checked uneasy notes (JumpTheory glimpse, heavy extraction). Each
    /// fires exactly once via `Codex::add` dedup. Called once per tick.
    pub(crate) fn check_triggered_notes(&mut self, tuning: &Tuning) {
        if self.flags.contains(&Flag::JumpTheoryKnown)
            && self.codex.add(Discovery {
                id: DiscoveryId::Note(NoteId::JumpTheoryGlimpse),
                title: "A Way Out".into(),
                text: "The route resolves into something you could actually fly. You catch yourself wanting it badly, the way a thing wants to spread. You wonder, briefly, whether the wanting is yours.".into(),
                tick: self.tick,
            })
        {
            self.log.push(self.tick, "The dark has an other side, and you ache toward it.", tuning.log_capacity);
        }
        if self.total_scrap_gained >= HEAVY_CONSUMPTION_SCRAP
            && self.codex.add(Discovery {
                id: DiscoveryId::Note(NoteId::HeavyConsumption),
                title: "What It Costs".into(),
                text: "You tally what you have torn loose to get this far. The moon is lighter for your being here. You tell yourself it was only scrap, only rock. You are good at telling yourself things.".into(),
                tick: self.tick,
            })
        {
            self.log.push(self.tick, "The pile of what you've taken has its own weight.", tuning.log_capacity);
        }
    }
}
