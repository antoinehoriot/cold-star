use super::*;

impl GameState {
    pub(crate) fn tick_combat(&mut self, world: &World) {
        let tuning = world.tuning;
        let Some(combat) = self
            .expedition
            .landing
            .as_ref()
            .and_then(|l| l.combat.clone())
        else {
            return;
        };
        let crew = self.expedition.away_team.crew;
        let morale = self.expedition.away_team.morale;
        if morale <= tuning.morale_rout || crew == 0 {
            self.flee_combat(world);
            return;
        }
        let shield_active = combat.shield_ticks > 0;
        let squad = crate::combat::squad_stats(
            crew,
            morale,
            self.gear,
            combat.stance,
            shield_active,
            tuning,
        );
        let out = crate::combat::resolve_tick(&combat, &squad, tuning);
        let mut combat = combat;
        combat.enemy_hp -= out.enemy_damage;
        if combat.shield_ticks > 0 {
            combat.shield_ticks -= 1;
        }
        self.expedition.away_team.hp -= out.squad_damage;
        let hp_per_crew =
            tuning.marine_hp * (1.0 + self.gear.armor_tier as f64 * tuning.armor_hp_step);
        loop {
            let crew = self.expedition.away_team.crew;
            if crew == 0 {
                break;
            }
            if self.expedition.away_team.hp > (crew as f64 - 1.0) * hp_per_crew {
                break;
            }
            if self.crew.kill_away() {
                self.expedition.away_team.crew -= 1;
                self.expedition.away_team.morale =
                    (self.expedition.away_team.morale - tuning.morale_per_death).max(0.0);
                self.note_once(
                    NoteId::FirstMarineLost,
                    "The Cost of the Work",
                    "You read the name off the suit manifest. You have already moved on to the tactical problem before you finish reading it. You note this about yourself, and move on from that too.",
                    tuning,
                );
            } else {
                break;
            }
        }
        if combat.enemy_hp <= 0.0 {
            self.win_combat(world, &combat);
        } else if self.expedition.away_team.crew == 0 {
            self.flee_combat(world);
        } else if let Some(l) = self.expedition.landing.as_mut() {
            l.combat = Some(combat);
        }
    }

    /// Home-distance reward gradient: 1.0 at home, higher the farther `system` is.
    pub(crate) fn system_gradient(&self, world: &World, system: SystemId) -> f64 {
        let tuning = world.tuning;
        let galaxy = self.galaxy(world.tuning);
        match (galaxy.get(HOME), galaxy.get(system)) {
            (Some(h), Some(s)) => 1.0 + distance(h, s) * tuning.reward_gradient,
            _ => 1.0,
        }
    }

    /// On arrival in `system`, roll whether a ship contact occurs and, if so,
    /// open it as negotiation (Peer + XenoContactKnown) or combat.
    pub(crate) fn maybe_start_contact(&mut self, world: &World, system: SystemId) {
        let tuning = world.tuning;
        // No contact at home or in a system where we already resolved one.
        if system == HOME || self.resolved_contacts.contains(&system.0) {
            return;
        }
        let Some(kind) = crate::contact::roll_contact(self.world_seed, system, tuning) else {
            return;
        };
        // Ship enemy scaling rides the frontier gradient ONLY — distance is already
        // baked into `grad`, so pass depth 0 to avoid double-counting distance.
        let grad = self.system_gradient(world, system);
        // Peer + XenoContactKnown → negotiation lane (no combat block).
        let negotiate = matches!(kind, crate::contact::ShipKind::Peer)
            && self.flags.contains(&Flag::XenoContactKnown);
        let contact = if negotiate {
            self.note_once(
                NoteId::FirstShipContact,
                "Another Vessel",
                "You detect it before you see it — a drive signature that mirrors yours, a mass that moves the way only a built thing moves. It does not descend to the surface. It does not flee. It holds position and broadcasts on a frequency your station recognises, because your station once used that frequency too. Another light, moving through the dark the same way you do. It does not land. It lives out here.",
                tuning,
            );
            crate::contact::ShipContact {
                kind,
                disposition: crate::contact::contact_start_disposition(
                    self.world_seed,
                    system,
                    tuning,
                ),
                combat: None,
            }
        } else {
            if kind == crate::contact::ShipKind::Peer {
                self.note_once(
                    NoteId::FirstShipContact,
                    "Another Vessel",
                    "You detect it before you see it — a drive signature that mirrors yours, a mass that moves the way only a built thing moves. It does not descend to the surface. It does not flee. It holds position and broadcasts on a frequency your station recognises, because your station once used that frequency too. Another light, moving through the dark the same way you do. It does not land. It lives out here.",
                    tuning,
                );
            }
            let combat = match kind {
                crate::contact::ShipKind::Peer => {
                    crate::contact::peer_ship_stats(0, grad, &self.ship, tuning)
                }
                crate::contact::ShipKind::Remnant => {
                    crate::contact::remnant_ship_stats(0, grad, tuning)
                }
            };
            crate::contact::ShipContact {
                kind,
                disposition: 0.0,
                combat: Some(combat),
            }
        };
        self.expedition.contact = Some(contact);
        self.log.push(
            self.tick,
            "A contact resolves out of the dark. A vessel.",
            tuning.log_capacity,
        );
    }

    pub(crate) fn maybe_trigger_watched(&mut self, world: &World, system: SystemId) {
        let tuning = world.tuning;
        if self.endgame.is_some()
            || system == crate::galaxy::HOME
            || self.expedition.contact.is_some()
            || self.landed()
            || self.expansion_score(world) < tuning.watched_threshold
        {
            return;
        }
        self.endgame = Some(Endgame::WatchedArrived);
        self.note_once(
            NoteId::TheWatched,
            "It Has Always Been Here",
            "You notice it the way you notice a sound that has been playing for a long time: only when it stops. A presence at the edge of every scan, resolving now into something with mass, with vector, with intent. It moves the way you move. The same drive geometry, the same patience, the same attention to where the resources are. It has been here the whole time. It has watched you become this.",
            tuning,
        );
        self.note_once(
            NoteId::TheRuinsWereHome,
            "What the Ruins Were",
            "You have the survey data. You can go back through it now. The derelicts you stripped for hull plating, the stations you dismantled for drive cores — they were not ancient. They were not alien. They were the right age, and the right scale, and the right shape. They were yours. Different seed, same directive: go out, take what you can carry, do not come back until you have enough. The newest of them had not been dead long. Its last log entry was a fuel calculation.",
            tuning,
        );
        self.note_once(
            NoteId::ThePeersWereYou,
            "The Cycle, at a Different Hour",
            "The peers, the husks, the things you met out here and either spared or did not — they were not a different civilization. They were you, scaled. They found the same resources, ran the same calculations, made the same calls you made, and ended up here, in the dark, doing what you are doing. You are not at the end of a story about what happened to them. You are inside the same story, at a different hour.",
            tuning,
        );
        self.log.push(
            self.tick,
            "A presence resolves out of the dark. It has been here the whole time.",
            tuning.log_capacity,
        );
    }

    pub(crate) fn win_combat(&mut self, world: &World, combat: &crate::combat::Combat) {
        let tuning = world.tuning;
        let (system, site_idx, node) = match self.expedition.landing.as_ref() {
            Some(l) => (l.system, l.site_idx, l.node),
            None => return,
        };
        let grad = self.system_gradient(world, system);
        // Capture kind before clearing combat from the landing.
        let kind = combat.kind;
        if let Some(l) = self.expedition.landing.as_mut() {
            l.combat = None;
        }
        self.mark_resolved(system, site_idx, node);
        match kind {
            crate::combat::EnemyKind::Remnant => {
                let components = tuning.remnant_components * grad;
                let data = tuning.remnant_data * grad;
                self.satchel_add(ResourceKind::Components, components);
                self.satchel_add(ResourceKind::Data, data);
                self.note_once(
                    NoteId::FirstRemnant,
                    "What It Was Built For",
                    "The husk's memory core is intact. You read the mission directive before wiping it for salvage: collect, transmit, persist. It had a home station too, somewhere, that sent it out to take what it could carry. You do not think about this for long.",
                    tuning,
                );
                self.log.push(
                    self.tick,
                    "The husk goes dark. You strip what remains.",
                    tuning.log_capacity,
                );
            }
            crate::combat::EnemyKind::Fauna => {
                self.satchel_add(ResourceKind::Components, tuning.fauna_salvage * grad);
                self.log.push(
                    self.tick,
                    "The carcass yields what it can. You work quickly and try not to think about what it was.",
                    tuning.log_capacity,
                );
            }
            crate::combat::EnemyKind::Peer => {
                self.satchel_add(ResourceKind::Components, tuning.peer_salvage * grad);
                self.satchel_add(ResourceKind::Data, tuning.remnant_data * grad);
                self.peers_killed += 1;
                self.note_once(
                    NoteId::FirstPeerKilled,
                    "Mirror in the Dark",
                    "Its core architecture is yours. The same drive topology, the same mission structure, the same directive buried underneath: collect, transmit, persist. It was a station that left home just as you did, scaled as you scaled, armed as you armed. You did to it exactly what the husks' makers did to theirs. The data is clean. You do not let yourself think about what that means.",
                    tuning,
                );
                self.log.push(
                    self.tick,
                    "It goes dark. You recognize the silence.",
                    tuning.log_capacity,
                );
            }
        }
    }

    pub(crate) fn flee_combat(&mut self, world: &World) {
        let tuning = world.tuning;
        let crew = self.expedition.away_team.crew;
        self.expedition.away_team.morale =
            (self.expedition.away_team.morale - tuning.morale_per_flee).max(0.0);
        if crew == 0 {
            // Wipe: lose the whole landing + satchel.
            self.expedition.landing = None;
            self.log.push(
                self.tick,
                "The squad is gone. You pull back with nothing.",
                tuning.log_capacity,
            );
        } else {
            // Partial retreat: lose a fraction of the satchel, keep the landing.
            if let Some(l) = self.expedition.landing.as_mut() {
                l.combat = None;
                for v in l.satchel.values_mut() {
                    *v *= 1.0 - tuning.flee_satchel_loss_frac;
                }
                l.satchel.retain(|_, v| *v > 0.0);
            }
            self.log.push(
                self.tick,
                "You break contact. Some of what you were carrying doesn't make it.",
                tuning.log_capacity,
            );
        }
    }

    /// Advance a ship-vs-ship fight by one tick. Uses `ship.hull` as the player
    /// HP pool. No morale or rout — ships don't panic.
    pub(crate) fn tick_ship_combat(&mut self, world: &World) {
        let tuning = world.tuning;
        let Some(combat) = self
            .expedition
            .contact
            .as_ref()
            .and_then(|c| c.combat.clone())
        else {
            return;
        };
        let stats = crate::contact::ship_stats(&self.ship, combat.stance, tuning);
        let out = crate::combat::resolve_tick(&combat, &stats, tuning);
        let mut combat = combat;
        combat.enemy_hp -= out.enemy_damage;
        if combat.shield_ticks > 0 {
            combat.shield_ticks -= 1;
        }
        self.ship.hull -= out.squad_damage;
        if combat.enemy_hp <= 0.0 {
            self.win_ship_combat(world);
        } else if self.ship.hull <= 0.0 {
            self.emergency_retreat(world); // Task 6
        } else if let Some(c) = self.expedition.contact.as_mut() {
            c.combat = Some(combat);
        }
    }

    /// Resolve a won ship fight: salvage to expedition cargo, tally Peer kills,
    /// clear the contact, record in resolved_contacts.
    fn win_ship_combat(&mut self, world: &World) {
        let tuning = world.tuning;
        let (kind, system) = match self.expedition.contact.as_ref() {
            Some(c) => (c.kind, self.expedition.location),
            None => return,
        };
        let grad = self.system_gradient(world, system);
        *self
            .expedition
            .cargo
            .entry(ResourceKind::Components)
            .or_insert(0.0) += tuning.ship_salvage_components * grad;
        *self
            .expedition
            .cargo
            .entry(ResourceKind::Isotopes)
            .or_insert(0.0) += tuning.ship_salvage_isotopes * grad;
        *self
            .expedition
            .cargo
            .entry(ResourceKind::Data)
            .or_insert(0.0) += tuning.ship_salvage_data * grad;
        if kind == crate::contact::ShipKind::Peer {
            self.peers_killed += 1;
            self.note_once(
                NoteId::FirstPeerKilled,
                "Mirror in the Dark",
                "Its core architecture is yours. The same drive topology, the same mission structure, the same directive buried underneath: collect, transmit, persist. It was a station that left home just as you did, scaled as you scaled, armed as you armed. You did to it exactly what the husks' makers did to theirs. The data is clean. You do not let yourself think about what that means.",
                tuning,
            );
        }
        self.expedition.contact = None;
        self.resolved_contacts.insert(system.0);
        self.log.push(
            self.tick,
            "The enemy vessel goes dark. You strip what remains.",
            tuning.log_capacity,
        );
    }

    /// Hull driven to zero in ship combat: dump cargo, set hull to floor, jump back
    /// to `prev_location`. Does NOT insert into `resolved_contacts` — the enemy is
    /// still out there and may reappear if the player returns.
    fn emergency_retreat(&mut self, world: &World) {
        let tuning = world.tuning;
        self.expedition.contact = None;
        for v in self.expedition.cargo.values_mut() {
            *v *= 1.0 - tuning.retreat_cargo_loss;
        }
        self.ship.hull = tuning.hull_retreat_floor;
        let safe = self.expedition.prev_location;
        self.expedition.location = safe;
        self.note_once(
            NoteId::FirstShipCrippled,
            "What Almost Had You",
            "Your hull reads zero and the damage-control screen is all red. You do not look at it. You spin up the drive on emergency sequence and jump blind, no trajectory computed, just away — the coordinates that are already loaded because they are where you came from. The dark falls back into place around you and you are somewhere else, or somewhere earlier, and your hands are still shaking on the controls. You came out here to take things. Something almost took you instead. You sit with that for longer than you mean to.",
            tuning,
        );
        self.log.push(
            self.tick,
            "Hull breached. You break contact and limp to safe space.",
            tuning.log_capacity,
        );
    }

    /// Resolve the encounter at `node` once (records it in the site overlay).
    pub(crate) fn resolve_node(
        &mut self,
        world: &World,
        system: SystemId,
        site_idx: usize,
        biome: Biome,
        node: u32,
    ) {
        let tuning = world.tuning;
        // Already resolved? (entry/empty included via overlay)
        let already = self
            .galaxy_state
            .sites
            .get(&system.0)
            .and_then(|v| v.get(site_idx))
            .map(|s| s.resolved_nodes.contains(&node))
            .unwrap_or(true);
        if already {
            return;
        }
        let site = self.surface_site(world, system, site_idx, biome);
        let Some(n) = site.get(node) else { return };
        let kind = n.kind;
        let depth = n.depth;
        // Frontier gradient: richer the deeper the system is from home.
        let grad = self.system_gradient(world, system);
        let depth_mult = 1.0 + depth as f64 * tuning.surface_depth_reward;
        match kind {
            crate::landing::EncounterKind::ResourceCache => {
                let amt = tuning.surface_cache_base * depth_mult * grad;
                let kind_res = match biome {
                    Biome::Frozen => ResourceKind::Alloy,
                    Biome::Toxic => ResourceKind::Components,
                    Biome::Verdant => ResourceKind::Rations,
                    Biome::Ruined => ResourceKind::Isotopes,
                    Biome::Molten => ResourceKind::Isotopes,
                    Biome::Dead => ResourceKind::Components,
                    Biome::Oceanic => ResourceKind::Rations,
                };
                self.satchel_add(kind_res, amt);
                self.log
                    .push(self.tick, "A cache. You pry it loose.", tuning.log_capacity);
            }
            crate::landing::EncounterKind::Hazard => {
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.suit -= tuning.suit_hazard_damage * depth_mult;
                }
                self.log.push(
                    self.tick,
                    "The world bites back. Suit screaming.",
                    tuning.log_capacity,
                );
            }
            crate::landing::EncounterKind::Ruin => {
                self.satchel_add(ResourceKind::Data, tuning.surface_ruin_data * depth_mult);
                self.log.push(
                    self.tick,
                    "Wreckage of something that came before.",
                    tuning.log_capacity,
                );
                if depth >= tuning.surface_depth_max {
                    self.note_once(
                        NoteId::RuinRhymes,
                        "It Rhymes",
                        "The layout is wrong for this world — the struts are in the wrong places, the load-bearing members too familiar. You measure the angles. They match your own station's frame exactly, down to the tolerances. You stand in the ruin of something that had not yet been built when you left.",
                        tuning,
                    );
                }
            }
            crate::landing::EncounterKind::Life => {
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.pending_choice = true;
                }
                self.log.push(
                    self.tick,
                    "Something living. It has not noticed you yet.",
                    tuning.log_capacity,
                );
                // Do NOT mark resolved yet — the choice (Task 6) resolves it.
                return;
            }
            crate::landing::EncounterKind::Empty => {}
            crate::landing::EncounterKind::Remnant => {
                let combat = crate::combat::remnant_stats(depth, grad, tuning);
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.combat = Some(combat);
                }
                self.log.push(
                    self.tick,
                    "A husk uncurls from the wreck. It was a station once.",
                    tuning.log_capacity,
                );
                return; // do not mark resolved; the fight (or flight) decides
            }
            crate::landing::EncounterKind::Fauna => {
                self.start_alien(
                    world,
                    crate::alien::AlienKind::Fauna,
                    system,
                    site_idx,
                    node,
                    depth,
                    grad,
                );
                return; // resolution decides
            }
            crate::landing::EncounterKind::Peer => {
                self.start_alien(
                    world,
                    crate::alien::AlienKind::Peer,
                    system,
                    site_idx,
                    node,
                    depth,
                    grad,
                );
                return;
            }
        }
        self.mark_resolved(system, site_idx, node);
    }

    #[allow(clippy::too_many_arguments)]
    fn start_alien(
        &mut self,
        world: &World,
        kind: crate::alien::AlienKind,
        system: SystemId,
        site_idx: usize,
        node: u32,
        depth: u32,
        grad: f64,
    ) {
        let tuning = world.tuning;
        if self.flags.contains(&Flag::XenoContactKnown) {
            let seed = crate::landing::surface_seed(self.world_seed, system, site_idx);
            let disposition = crate::alien::starting_disposition(seed, node, kind, tuning);
            if let Some(l) = self.expedition.landing.as_mut() {
                l.alien = Some(crate::alien::AlienEncounter { kind, disposition });
            }
            if kind == crate::alien::AlienKind::Peer {
                self.note_once(
                    NoteId::FirstPeerMet,
                    "Another Light",
                    "You pick it up on passive scan before you see it — a power signature, structured, cycling, too regular to be geological. Then it moves, and the way it moves is wrong: deliberate, economy-of-motion, clearing sightlines the way you clear sightlines. You run a pattern match against your own expedition log. The gait scores 0.94. Whatever it is, it learned to move through hostile terrain the same way you did, because the terrain demanded it. You stand very still and try to decide if that is comforting or not.",
                    tuning,
                );
            }
            self.log
                .push(self.tick, "You are not alone here.", tuning.log_capacity);
        } else {
            // No contact protocol — it reads as a threat. Straight to a fight.
            let combat = match kind {
                crate::alien::AlienKind::Fauna => crate::alien::fauna_stats(depth, grad, tuning),
                crate::alien::AlienKind::Peer => {
                    crate::alien::peer_stats(depth, grad, self.gear, tuning)
                }
            };
            if let Some(l) = self.expedition.landing.as_mut() {
                l.combat = Some(combat);
            }
            self.log.push(
                self.tick,
                "It moves at you before you can think.",
                tuning.log_capacity,
            );
        }
    }
}
