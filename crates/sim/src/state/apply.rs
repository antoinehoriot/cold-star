use super::*;

impl GameState {
    /// Actions that manage the home station — refused while the player is away.
    /// Expedition actions (Jump/ScanSystem/ResolveSite/Scavenge) are always allowed.
    fn is_station_management(action: &Action) -> bool {
        matches!(
            action,
            Action::Ignite
                | Action::Salvage
                | Action::FeedCore
                | Action::ActivateCrewSource
                | Action::Build(_)
                | Action::AssignCrew { .. }
                | Action::SetPriority { .. }
                | Action::StartResearch(_)
                | Action::UpgradeDrive
                | Action::SetAwayCrew { .. }
                | Action::UpgradeWeapon
                | Action::UpgradeArmor
                | Action::CraftConsumable(_)
                | Action::UpgradeShipWeapon
                | Action::UpgradeHullPlating
                | Action::CraftRepairDrone
        )
    }

    /// While landed, only the landing actions are allowed.
    fn is_landing_blocked(action: &Action) -> bool {
        !matches!(
            action,
            Action::MoveToNode(_)
                | Action::HarvestLife
                | Action::LeaveLife
                | Action::Extract
                | Action::UseConsumable(_)
                | Action::SetStance(_)
                | Action::Flee
                | Action::Parley
                | Action::Avoid
                | Action::Observe
                | Action::Gift
                | Action::Trade
                | Action::Harvest
                | Action::AttackAlien
        )
    }

    /// While a ship Contact is open, system exploration actions are blocked.
    /// Contact-resolution verbs (Tasks 7–8) and Jump are not blocked.
    fn is_contact_blocked(action: &Action) -> bool {
        matches!(
            action,
            Action::ScanSystem(_) | Action::ResolveSite(..) | Action::Scavenge | Action::Land(_)
        )
    }

    /// Once the endgame is set (WatchedArrived or Ended), only ChooseEnding can proceed.
    fn is_endgame_blocked(action: &Action) -> bool {
        !matches!(action, Action::ChooseEnding(_))
    }

    pub fn apply(&mut self, world: &World, action: Action) -> Result<(), Rejected> {
        if self.landed() && Self::is_landing_blocked(&action) {
            return Err(Rejected("you are on the surface"));
        }
        if self.expedition.contact.is_some() && Self::is_contact_blocked(&action) {
            return Err(Rejected(
                "a vessel is blocking your approach — resolve the contact",
            ));
        }
        if self.endgame.is_some() && Self::is_endgame_blocked(&action) {
            return Err(Rejected("the watched is waiting"));
        }
        if self.away() && Self::is_station_management(&action) {
            return Err(Rejected("you are away from the station"));
        }
        let tuning = world.tuning;
        match action {
            Action::Ignite => {
                if self.reactor.status != ReactorStatus::Cold {
                    return Err(Rejected("the core is already lit"));
                }
                self.reactor.ignite(tuning);
                self.log_flavor("ignite", tuning);
                self.update_disclosure_flags(world);
                Ok(())
            }
            Action::Salvage => {
                if self.salvage_cooldown > 0 {
                    return Err(Rejected("salvage is on cooldown"));
                }
                self.resources
                    .add(ResourceKind::Scrap, tuning.salvage_yield);
                self.salvage_cooldown = tuning.salvage_cooldown_ticks;
                self.update_disclosure_flags(world);
                Ok(())
            }
            Action::FeedCore => {
                if !self
                    .resources
                    .try_spend(ResourceKind::Scrap, tuning.feed_cost_scrap)
                {
                    return Err(Rejected("not enough scrap to feed the core"));
                }
                self.reactor.feed(tuning.feed_charge, tuning);
                self.update_disclosure_flags(world);
                Ok(())
            }
            Action::ActivateCrewSource => {
                if !self.flags.contains(&Flag::CrewSourceAvailable) {
                    return Err(Rejected("the crew-source is not available yet"));
                }
                if self.flags.contains(&Flag::CrewSourceActive) {
                    return Err(Rejected("the crew-source is already active"));
                }
                if !self
                    .resources
                    .try_spend(ResourceKind::Scrap, tuning.crew_source_cost)
                {
                    return Err(Rejected("not enough scrap to activate the crew-source"));
                }
                self.flags.insert(Flag::CrewSourceActive);
                self.crew.begin_arrivals(tuning);
                self.log_flavor("source_active", tuning);
                self.update_disclosure_flags(world);
                Ok(())
            }
            Action::Build(id) => {
                let module = world.modules.get(id).ok_or(Rejected("unknown module"))?;
                if !module.repeatable && self.has_module(id) {
                    return Err(Rejected("already built"));
                }
                for &req in &module.requires_modules {
                    if !self.has_module(req) {
                        return Err(Rejected("missing prerequisite module"));
                    }
                }
                if let Some(flag) = module.requires_flag
                    && !self.flags.contains(&flag)
                {
                    return Err(Rejected("jump theory is not yet understood"));
                }
                let n = self.built_count(id) as f64;
                let cost: Vec<(ResourceKind, f64)> = module
                    .cost
                    .iter()
                    .map(|&(k, a)| (k, a * module.cost_scaling.powf(n)))
                    .collect();
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough resources to build"));
                }
                *self.built.entry(id).or_insert(0) += 1;
                // Apply caps/housing/reactor unlocks immediately.
                if module.unlocks.cap_increase > 0.0 {
                    let new_cap = self.resources.cap + module.unlocks.cap_increase;
                    self.resources.set_cap(new_cap);
                }
                if module.unlocks.max_output > 0.0 {
                    self.reactor.max_output += module.unlocks.max_output;
                }
                if module.unlocks.capacity_increase > 0.0 {
                    self.reactor.capacity += module.unlocks.capacity_increase;
                }
                if id == ModuleId::Shipyard {
                    self.ship.built = true;
                    self.flags.insert(Flag::ShipyardComplete);
                    self.log_flavor("shipyard_complete", tuning);
                }
                self.sync_priority(world);
                self.update_disclosure_flags(world);
                Ok(())
            }
            Action::AssignCrew { job, delta } => {
                // Job must be unlocked by its gating module.
                if let Some(j) = world.jobs.get(job) {
                    if let Some(gate) = j.gated_by
                        && !self.has_module(gate)
                    {
                        return Err(Rejected("that job's module is not built"));
                    }
                } else {
                    return Err(Rejected("unknown job"));
                }
                if self.crew.assign(job, delta) {
                    Ok(())
                } else {
                    Err(Rejected("no change (no idle crew or none assigned)"))
                }
            }
            Action::SetPriority { consumer, up } => {
                if self.priority.rank(consumer).is_none() {
                    return Err(Rejected("unknown consumer"));
                }
                if up {
                    self.priority.move_up(consumer);
                } else {
                    self.priority.move_down(consumer);
                }
                Ok(())
            }
            Action::StartResearch(id) => {
                if !self.has_module(ModuleId::Lab) {
                    return Err(Rejected("no lab to research in"));
                }
                let tech = world.techs.get(id).ok_or(Rejected("unknown tech"))?;
                if self.research.is_completed(id) {
                    return Err(Rejected("already researched"));
                }
                if self.research.active.is_some() {
                    return Err(Rejected("research already in progress"));
                }
                if !tech.requires.iter().all(|r| self.research.is_completed(*r)) {
                    return Err(Rejected("missing prerequisite research"));
                }
                if !self.resources.try_spend(ResourceKind::Data, tech.data_cost) {
                    return Err(Rejected("not enough data"));
                }
                self.research.active = Some(ActiveResearch {
                    tech: id,
                    ticks_remaining: tech.research_ticks,
                });
                self.log.push(
                    self.tick,
                    format!("Research begun: {}", tech.name),
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::UpgradeDrive => {
                if !self.flags.contains(&Flag::NavigationKnown) {
                    return Err(Rejected("navigation is not yet understood"));
                }
                if !self
                    .resources
                    .try_spend(ResourceKind::Isotopes, tuning.drive_upgrade_isotopes)
                {
                    return Err(Rejected("not enough isotopes to recalibrate the drive"));
                }
                self.ship.drive_range += tuning.drive_upgrade_step;
                self.log.push(
                    self.tick,
                    "The drive reaches a little further into the dark.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::ScanSystem(target) => {
                if !self.has_module(ModuleId::SensorArray) {
                    return Err(Rejected("no sensor array to scan with"));
                }
                if self.expedition.activity.is_some() {
                    return Err(Rejected("the ship is already busy"));
                }
                if !self.galaxy_state.revealed.contains(&target.0) {
                    return Err(Rejected("that system is not on the map"));
                }
                if self.galaxy_state.scanned.contains(&target.0) {
                    return Err(Rejected("already scanned"));
                }
                if !self
                    .resources
                    .try_spend(ResourceKind::Data, tuning.scan_data_cost)
                {
                    return Err(Rejected("not enough data to scan"));
                }
                self.expedition.activity = Some(Activity::Scan {
                    target,
                    ticks: tuning.scan_ticks,
                });
                Ok(())
            }
            Action::ResolveSite(system, idx) => {
                if self.expedition.activity.is_some() {
                    return Err(Rejected("the ship is already busy"));
                }
                if self.expedition.location != system {
                    return Err(Rejected("you are not in that system"));
                }
                match self
                    .galaxy_state
                    .sites
                    .get(&system.0)
                    .and_then(|v| v.get(idx))
                {
                    Some(site) if !site.depleted => {}
                    Some(_) => return Err(Rejected("that site is stripped")),
                    None => return Err(Rejected("no such site")),
                }
                if let Some(SiteKind::Planet(_)) = self
                    .galaxy_state
                    .sites
                    .get(&system.0)
                    .and_then(|v| v.get(idx))
                    .map(|s| s.kind)
                {
                    return Err(Rejected("you must land on a planet, not strip it"));
                }
                self.expedition.activity = Some(Activity::ResolveSite {
                    system,
                    site_idx: idx,
                    ticks: tuning.site_resolve_ticks,
                });
                Ok(())
            }
            Action::Jump(target) => {
                if self.expedition.activity.is_some() {
                    return Err(Rejected("the ship is already busy"));
                }
                if target == self.expedition.location {
                    return Err(Rejected("already there"));
                }
                if !self.galaxy_state.revealed.contains(&target.0) {
                    return Err(Rejected("that system is not on the map"));
                }
                let galaxy = self.galaxy(world);
                let (Some(here), Some(dest)) =
                    (galaxy.get(self.expedition.location), galaxy.get(target))
                else {
                    return Err(Rejected("unknown system"));
                };
                let dist = distance(here, dest);
                if dist > self.ship.drive_range {
                    return Err(Rejected("beyond drive range"));
                }
                let cost = jump_cost(dist, tuning);
                if !self.resources.try_spend(ResourceKind::Fuel, cost) {
                    return Err(Rejected("not enough fuel to jump"));
                }
                let ticks =
                    tuning.transit_ticks_base + (dist * tuning.transit_ticks_factor).round() as u32;
                self.expedition.activity = Some(Activity::Transit { to: target, ticks });
                Ok(())
            }
            Action::Scavenge => {
                if self.expedition.activity.is_some() {
                    return Err(Rejected("the ship is already busy"));
                }
                if !self.is_stranded(world) {
                    return Err(Rejected("no need to scavenge"));
                }
                self.expedition.activity = Some(Activity::Scavenge {
                    ticks: tuning.scavenge_ticks,
                });
                Ok(())
            }
            Action::Land(idx) => {
                if !self.ship.built {
                    return Err(Rejected("no ship to land with"));
                }
                if self.expedition.activity.is_some() || self.landed() {
                    return Err(Rejected("the ship is already busy"));
                }
                let loc = self.expedition.location;
                let Some(SiteKind::Planet(biome)) = self
                    .galaxy_state
                    .sites
                    .get(&loc.0)
                    .and_then(|v| v.get(idx))
                    .map(|s| s.kind)
                else {
                    return Err(Rejected("that site is not a planet"));
                };
                self.expedition.landing = Some(Landing {
                    system: loc,
                    site_idx: idx,
                    biome,
                    node: 0,
                    suit: tuning.suit_max,
                    satchel: std::collections::BTreeMap::new(),
                    pending_choice: false,
                    traverse: 0,
                    combat: None,
                    alien: None,
                });
                self.mark_resolved(loc, idx, 0);
                self.log.push(
                    self.tick,
                    "Dropship down. Your boots find a world.",
                    tuning.log_capacity,
                );
                self.note_once(
                    NoteId::FirstLanding,
                    "Someone Else's Sky",
                    "The air here has its own colour, its own weight. You are the first thing from your world to stand in it — and you notice, almost before anything else, that you have already begun to change it.",
                    tuning,
                );
                Ok(())
            }
            Action::Extract => {
                let Some(landing) = self.expedition.landing.take() else {
                    return Err(Rejected("you are not on a surface"));
                };
                for (k, amt) in landing.satchel {
                    let space = (self.ship.cargo - self.expedition.cargo_total()).max(0.0);
                    let take = amt.min(space);
                    if take > 0.0 {
                        *self.expedition.cargo.entry(k).or_insert(0.0) += take;
                    }
                }
                self.log.push(
                    self.tick,
                    "You burn back to orbit with what you could carry.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::MoveToNode(target) => {
                let Some(landing) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if landing.pending_choice {
                    return Err(Rejected("resolve the encounter first"));
                }
                if landing.traverse > 0 {
                    return Err(Rejected("already moving"));
                }
                let site =
                    self.surface_site(world, landing.system, landing.site_idx, landing.biome);
                if !site.is_neighbor(landing.node, target) {
                    return Err(Rejected("no route to that node"));
                }
                self.expedition.landing.as_mut().unwrap().traverse = tuning.traverse_ticks;
                self.expedition.landing.as_mut().unwrap().node = target;
                Ok(())
            }
            Action::HarvestLife => {
                let Some(l) = self.expedition.landing.as_mut() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if !l.pending_choice {
                    return Err(Rejected("nothing to harvest"));
                }
                let (system, site_idx, biome, node) = (l.system, l.site_idx, l.biome, l.node);
                let depth = self
                    .surface_site(world, system, site_idx, biome)
                    .get(node)
                    .map(|n| n.depth)
                    .unwrap_or(0);
                let amt = tuning.surface_life_harvest
                    * (1.0 + depth as f64 * tuning.surface_depth_reward);
                self.satchel_add(ResourceKind::Rations, amt);
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.pending_choice = false;
                }
                self.mark_resolved(system, site_idx, node);
                self.note_once(
                    NoteId::FirstLifeTaken,
                    "What You Took",
                    "You process what you need and leave the rest. The record shows a gain in Rations. It does not show what was here before you, or that it will not be here again.",
                    tuning,
                );
                Ok(())
            }
            Action::LeaveLife => {
                let Some(l) = self.expedition.landing.as_mut() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if !l.pending_choice {
                    return Err(Rejected("nothing to leave"));
                }
                let (system, site_idx, node) = (l.system, l.site_idx, l.node);
                self.satchel_add(ResourceKind::Data, tuning.surface_life_leave_data);
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.pending_choice = false;
                }
                self.mark_resolved(system, site_idx, node);
                self.note_once(
                    NoteId::LifeSpared,
                    "What You Left",
                    "You note the organism and move on. You tell yourself it was a practical decision — data over calories, scarcity of suit time. You do not examine how long you stood there before choosing.",
                    tuning,
                );
                Ok(())
            }
            Action::UpgradeWeapon => {
                if !self.has_module(ModuleId::Armory) {
                    return Err(Rejected("no armory"));
                }
                if self.gear.weapon_tier >= 2 {
                    return Err(Rejected("weapon at max tier"));
                }
                let cost = vec![
                    (ResourceKind::Alloy, tuning.weapon_upgrade_alloy),
                    (ResourceKind::Components, tuning.weapon_upgrade_components),
                ];
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                self.gear.weapon_tier += 1;
                Ok(())
            }
            Action::UpgradeArmor => {
                if !self.has_module(ModuleId::Armory) {
                    return Err(Rejected("no armory"));
                }
                if self.gear.armor_tier >= 2 {
                    return Err(Rejected("armor at max tier"));
                }
                let cost = vec![
                    (ResourceKind::Alloy, tuning.armor_upgrade_alloy),
                    (ResourceKind::Components, tuning.armor_upgrade_components),
                ];
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                self.gear.armor_tier += 1;
                Ok(())
            }
            Action::CraftConsumable(kind) => {
                if !self.has_module(ModuleId::Armory) {
                    return Err(Rejected("no armory"));
                }
                let cost = match kind {
                    ConsumableKind::MedGel => {
                        vec![(ResourceKind::Components, tuning.medgel_cost)]
                    }
                    ConsumableKind::Grenade => {
                        vec![(ResourceKind::Components, tuning.grenade_cost)]
                    }
                    ConsumableKind::Shield => {
                        vec![(ResourceKind::Components, tuning.shield_cost)]
                    }
                };
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                *self.consumables.entry(kind).or_insert(0) += 1;
                Ok(())
            }
            Action::UseConsumable(kind) => {
                let Some(landing) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if landing.combat.is_none() {
                    return Err(Rejected("no active combat"));
                }
                let count = self.consumables.get(&kind).copied().unwrap_or(0);
                if count == 0 {
                    return Err(Rejected("no consumable of that type"));
                }
                *self.consumables.entry(kind).or_insert(0) -= 1;
                match kind {
                    ConsumableKind::MedGel => {
                        let max = self.squad_hp_max(tuning);
                        self.expedition.away_team.hp =
                            (self.expedition.away_team.hp + tuning.medgel_heal).min(max);
                    }
                    ConsumableKind::Grenade => {
                        if let Some(l) = self.expedition.landing.as_mut()
                            && let Some(c) = l.combat.as_mut()
                        {
                            c.enemy_hp -= tuning.grenade_damage;
                        }
                        // Win immediately if grenade kills the enemy.
                        let enemy_hp = self
                            .expedition
                            .landing
                            .as_ref()
                            .and_then(|l| l.combat.as_ref())
                            .map(|c| c.enemy_hp)
                            .unwrap_or(1.0);
                        if enemy_hp <= 0.0 {
                            self.win_combat(
                                world,
                                &self
                                    .expedition
                                    .landing
                                    .as_ref()
                                    .unwrap()
                                    .combat
                                    .clone()
                                    .unwrap(),
                            );
                        }
                    }
                    ConsumableKind::Shield => {
                        if let Some(l) = self.expedition.landing.as_mut()
                            && let Some(c) = l.combat.as_mut()
                        {
                            c.shield_ticks = tuning.shield_ticks;
                        }
                    }
                }
                Ok(())
            }
            Action::SetStance(stance) => {
                let Some(landing) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if landing.combat.is_none() {
                    return Err(Rejected("no active combat"));
                }
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(c) = l.combat.as_mut()
                {
                    c.stance = stance;
                }
                Ok(())
            }
            Action::Flee => {
                if self
                    .expedition
                    .landing
                    .as_ref()
                    .and_then(|l| l.combat.as_ref())
                    .is_none()
                {
                    return Err(Rejected("no active combat to flee from"));
                }
                self.flee_combat(world);
                Ok(())
            }
            Action::Parley => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("nothing there to parley with")),
                };
                if crate::alien::band(alien.disposition, tuning) == crate::alien::Band::Hostile {
                    return Err(Rejected("it will not listen"));
                }
                let new_disp = alien.disposition + tuning.parley_shift;
                let (system, site_idx, node) = (l.system, l.site_idx, l.node);
                // Apply disposition shift
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(a) = l.alien.as_mut()
                {
                    a.disposition = new_disp;
                }
                self.satchel_add(ResourceKind::Data, tuning.observe_data);
                self.log.push(
                    self.tick,
                    "You signal intent. It considers you.",
                    tuning.log_capacity,
                );
                // If Peer reaches peaceful band, end the encounter peacefully
                if alien.kind == crate::alien::AlienKind::Peer && new_disp >= tuning.peace_band {
                    if let Some(l) = self.expedition.landing.as_mut() {
                        l.alien = None;
                    }
                    self.mark_resolved(system, site_idx, node);
                    self.peers_spared += 1;
                    self.note_once(
                        NoteId::FirstPeerSpared,
                        "The Communion Thread",
                        "You did not know what you were trying to say. You ran the signal through every protocol you had — frequency sweep, prime ratios, power-of-two sequences — and at some point it stopped mattering which one worked. What mattered was that it answered. Not with words. With a reduction in threat posture. A mirroring of your own stillness back at you. Two machines, each built to survive the same silence, each having arrived at the same conclusion: that the silence was worse than the risk. You stand there for a long time after it goes. The record shows a contact event, resolved. It does not show that you watched the direction it left for longer than was necessary.",
                        tuning,
                    );
                    self.log
                        .push(self.tick, "It moves away. You let it.", tuning.log_capacity);
                }
                Ok(())
            }
            Action::Avoid => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if l.alien.is_none() {
                    return Err(Rejected("nothing there to avoid"));
                }
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.alien = None;
                }
                self.log.push(
                    self.tick,
                    "You withdraw. You leave it be.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::Observe => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("nothing there to observe")),
                };
                if alien.kind != crate::alien::AlienKind::Fauna {
                    return Err(Rejected("observe is for fauna only"));
                }
                if crate::alien::band(alien.disposition, tuning) == crate::alien::Band::Hostile {
                    return Err(Rejected("it is too agitated to observe safely"));
                }
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(a) = l.alien.as_mut()
                {
                    a.disposition += tuning.observe_shift;
                }
                self.satchel_add(ResourceKind::Data, tuning.observe_data);
                self.log.push(
                    self.tick,
                    "You watch. You learn. It does not notice you.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::Gift => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                if l.alien.is_none() {
                    return Err(Rejected("nothing there to gift"));
                }
                let have = self
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                if have < tuning.gift_cost {
                    return Err(Rejected("not enough Components to gift"));
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Components)
                    .or_insert(0.0) -= tuning.gift_cost;
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(a) = l.alien.as_mut()
                {
                    a.disposition += tuning.gift_shift;
                }
                self.log.push(
                    self.tick,
                    "You set the materials down and step back. A gesture older than language.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::Trade => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("they have nothing to trade")),
                };
                if alien.kind != crate::alien::AlienKind::Peer {
                    return Err(Rejected("they have nothing to trade"));
                }
                let b = crate::alien::band(alien.disposition, tuning);
                if b == crate::alien::Band::Hostile || b == crate::alien::Band::Fearful {
                    return Err(Rejected("they will not deal"));
                }
                let have = self
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                if have < tuning.trade_give {
                    return Err(Rejected("not enough Components to trade"));
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Components)
                    .or_insert(0.0) -= tuning.trade_give;
                let gain = if b == crate::alien::Band::Curious {
                    tuning.trade_get * CURIOUS_TRADE_BONUS
                } else {
                    tuning.trade_get
                };
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Isotopes)
                    .or_insert(0.0) += gain;
                if let Some(l) = self.expedition.landing.as_mut()
                    && let Some(a) = l.alien.as_mut()
                {
                    a.disposition += tuning.trade_disp_shift;
                }
                self.log.push(
                    self.tick,
                    "You exchange what you have for what they carry. Neither of you speaks.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::Harvest => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("nothing here to harvest like that")),
                };
                if alien.kind != crate::alien::AlienKind::Fauna {
                    return Err(Rejected("that is not an animal"));
                }
                let (system, site_idx, node) = (l.system, l.site_idx, l.node);
                // Yield is harvest_fauna_yield * grad; depth is not used here.
                let grad = self.system_gradient(world, system);
                let amt = tuning.harvest_fauna_yield * grad;
                self.satchel_add(ResourceKind::Rations, amt);
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.alien = None;
                }
                self.mark_resolved(system, site_idx, node);
                self.note_once(
                    NoteId::FirstFaunaDisplaced,
                    "What Fills the Silence",
                    "You take it apart with the efficiency you were built for — extract what carries caloric value, discard the rest. The readout updates: Rations gained. The world does not update. It has no record of what was here, no notation that this particular thing stopped. You are the only witness to the subtraction, and you are also the one who performed it. The next time you pass this way, the silence will be indistinguishable from the silence that was already here. You are not sure whether that is a mercy or the whole problem.",
                    tuning,
                );
                self.log.push(
                    self.tick,
                    "You take what the creature was made of. The world is quieter now.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::AttackAlien => {
                let Some(l) = self.expedition.landing.as_ref() else {
                    return Err(Rejected("you are not on a surface"));
                };
                let alien = match l.alien.as_ref() {
                    Some(a) => a.clone(),
                    None => return Err(Rejected("nothing there to attack")),
                };
                let kind = alien.kind;
                let (system, site_idx, biome, node) = (l.system, l.site_idx, l.biome, l.node);
                let depth = self
                    .surface_site(world, system, site_idx, biome)
                    .get(node)
                    .map(|n| n.depth)
                    .unwrap_or(0);
                let grad = self.system_gradient(world, system);
                let combat = match kind {
                    crate::alien::AlienKind::Fauna => {
                        crate::alien::fauna_stats(depth, grad, tuning)
                    }
                    crate::alien::AlienKind::Peer => {
                        crate::alien::peer_stats(depth, grad, self.gear, tuning)
                    }
                };
                if let Some(l) = self.expedition.landing.as_mut() {
                    l.combat = Some(combat);
                    l.alien = None;
                }
                self.log.push(
                    self.tick,
                    "You choose violence. It becomes a fight.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::SetAwayCrew { delta } => {
                if self.away() || self.landed() {
                    return Err(Rejected("can only change the away-team while docked"));
                }
                if delta > 0 {
                    let added = self.crew.commit_away(delta as u32);
                    if added == 0 {
                        return Err(Rejected("no idle crew to commit"));
                    }
                    self.expedition.away_team.crew += added;
                    // Initialize morale on first commit.
                    if self.expedition.away_team.morale == 0.0 {
                        self.expedition.away_team.morale = tuning.morale_max;
                    }
                    self.expedition.away_team.hp = self.squad_hp_max(tuning);
                } else if delta < 0 {
                    let removed = ((-delta) as u32).min(self.expedition.away_team.crew);
                    if removed == 0 {
                        return Err(Rejected("no away crew to release"));
                    }
                    self.crew.release_away(removed);
                    self.expedition.away_team.crew -= removed;
                    self.expedition.away_team.hp = self.squad_hp_max(tuning);
                    // Fully disbanded: reset so the next commit re-inits a fresh squad.
                    if self.expedition.away_team.crew == 0 {
                        self.expedition.away_team.morale = 0.0;
                        self.expedition.away_team.hp = 0.0;
                    }
                }
                Ok(())
            }
            // ── CB3 ship-scoped negotiation verbs ──────────────────────────
            Action::HailParley => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to hail")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("you are in a fight — not a negotiation"));
                }
                if contact.kind != crate::contact::ShipKind::Peer {
                    return Err(Rejected("it will not answer"));
                }
                if crate::alien::band(contact.disposition, tuning) == crate::alien::Band::Hostile {
                    return Err(Rejected("it will not answer"));
                }
                let new_disp = contact.disposition + tuning.parley_shift;
                let location = self.expedition.location;
                if let Some(c) = self.expedition.contact.as_mut() {
                    c.disposition = new_disp;
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Data)
                    .or_insert(0.0) += tuning.observe_data;
                self.log.push(
                    self.tick,
                    "You signal intent. It considers you.",
                    tuning.log_capacity,
                );
                if new_disp >= tuning.peace_band {
                    self.expedition.contact = None;
                    self.resolved_contacts.insert(location.0);
                    self.peers_spared += 1;
                    self.note_once(
                        NoteId::FirstPeerSpared,
                        "The Communion Thread",
                        "You did not know what you were trying to say. You ran the signal through every protocol you had — frequency sweep, prime ratios, power-of-two sequences — and at some point it stopped mattering which one worked. What mattered was that it answered. Not with words. With a reduction in threat posture. A mirroring of your own stillness back at you. Two machines, each built to survive the same silence, each having arrived at the same conclusion: that the silence was worse than the risk. You stand there for a long time after it goes. The record shows a contact event, resolved. It does not show that you watched the direction it left for longer than was necessary.",
                        tuning,
                    );
                    self.log.push(
                        self.tick,
                        "They stand down — you pass.",
                        tuning.log_capacity,
                    );
                }
                Ok(())
            }
            Action::HailTrade => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to trade with")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("you are in a fight — not a negotiation"));
                }
                if contact.kind != crate::contact::ShipKind::Peer {
                    return Err(Rejected("they will not deal"));
                }
                let b = crate::alien::band(contact.disposition, tuning);
                if b == crate::alien::Band::Hostile || b == crate::alien::Band::Fearful {
                    return Err(Rejected("they will not deal"));
                }
                let have = self
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                if have < tuning.trade_give {
                    return Err(Rejected("not enough Components to trade"));
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Components)
                    .or_insert(0.0) -= tuning.trade_give;
                let gain = if b == crate::alien::Band::Curious {
                    tuning.trade_get * CURIOUS_TRADE_BONUS
                } else {
                    tuning.trade_get
                };
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Isotopes)
                    .or_insert(0.0) += gain;
                if let Some(c) = self.expedition.contact.as_mut() {
                    c.disposition += tuning.trade_disp_shift;
                }
                self.log.push(
                    self.tick,
                    "You exchange what you have for what they carry. Neither of you speaks.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::HailGift => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to gift")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("you are in a fight — not a negotiation"));
                }
                if contact.kind != crate::contact::ShipKind::Peer {
                    return Err(Rejected("they will not accept the gesture"));
                }
                let have = self
                    .expedition
                    .cargo
                    .get(&ResourceKind::Components)
                    .copied()
                    .unwrap_or(0.0);
                if have < tuning.gift_cost {
                    return Err(Rejected("not enough Components to gift"));
                }
                *self
                    .expedition
                    .cargo
                    .entry(ResourceKind::Components)
                    .or_insert(0.0) -= tuning.gift_cost;
                if let Some(c) = self.expedition.contact.as_mut() {
                    c.disposition += tuning.gift_shift;
                }
                self.log.push(
                    self.tick,
                    "You set the materials down and step back. A gesture older than language.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::HailAvoid => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to avoid")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("you are in a fight — not a negotiation"));
                }
                if contact.kind != crate::contact::ShipKind::Peer {
                    return Err(Rejected("they will not let you pass"));
                }
                if crate::alien::band(contact.disposition, tuning) == crate::alien::Band::Hostile {
                    return Err(Rejected("they will not let you pass"));
                }
                let location = self.expedition.location;
                self.expedition.contact = None;
                self.resolved_contacts.insert(location.0);
                self.log.push(
                    self.tick,
                    "You give them a wide berth. They let you go.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            // ── CB3 ship combat inputs ──────────────────────────────────────
            Action::AttackShip => {
                let contact = match self.expedition.contact.as_ref() {
                    Some(c) => c.clone(),
                    None => return Err(Rejected("no contact to attack")),
                };
                if contact.combat.is_some() {
                    return Err(Rejected("already in ship combat"));
                }
                let grad = self.system_gradient(world, self.expedition.location);
                let combat = match contact.kind {
                    crate::contact::ShipKind::Peer => {
                        crate::contact::peer_ship_stats(0, grad, &self.ship, tuning)
                    }
                    crate::contact::ShipKind::Remnant => {
                        crate::contact::remnant_ship_stats(0, grad, tuning)
                    }
                };
                if let Some(c) = self.expedition.contact.as_mut() {
                    c.combat = Some(combat);
                }
                self.log.push(
                    self.tick,
                    "You open fire. The dark lights up.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::SetShipStance(stance) => {
                let has_combat = self
                    .expedition
                    .contact
                    .as_ref()
                    .map(|c| c.combat.is_some())
                    .unwrap_or(false);
                if !has_combat {
                    return Err(Rejected("no active ship combat"));
                }
                if let Some(c) = self.expedition.contact.as_mut()
                    && let Some(combat) = c.combat.as_mut()
                {
                    combat.stance = stance;
                }
                Ok(())
            }
            Action::UseRepairDrone => {
                let has_combat = self
                    .expedition
                    .contact
                    .as_ref()
                    .map(|c| c.combat.is_some())
                    .unwrap_or(false);
                if !has_combat {
                    return Err(Rejected("no active ship combat"));
                }
                if self.ship.repair_drones == 0 {
                    return Err(Rejected("no repair drones in stock"));
                }
                let hull_max = self.ship.hull_max(tuning);
                self.ship.hull = (self.ship.hull + tuning.repair_drone_heal).min(hull_max);
                self.ship.repair_drones -= 1;
                self.log.push(
                    self.tick,
                    "A repair drone seals the breach. Hull stable.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::ShipFlee => {
                if self.expedition.contact.is_none() {
                    return Err(Rejected("no contact to flee from"));
                }
                let here = self.expedition.location;
                let prev = self.expedition.prev_location;
                let galaxy = self.galaxy(world);
                let cost = match (galaxy.get(here), galaxy.get(prev)) {
                    (Some(h), Some(p)) => jump_cost(distance(h, p), tuning),
                    _ => jump_cost(1.0, tuning), // fallback: minimum cost
                };
                if !self.resources.try_spend(ResourceKind::Fuel, cost) {
                    return Err(Rejected("no fuel to break contact"));
                }
                self.expedition.contact = None;
                self.expedition.location = prev;
                self.log.push(
                    self.tick,
                    "You burn hard and break contact. The system falls behind you.",
                    tuning.log_capacity,
                );
                Ok(())
            }
            Action::UpgradeShipWeapon => {
                if !self.flags.contains(&Flag::MunitionsKnown) {
                    return Err(Rejected("no munitions doctrine"));
                }
                if self.built_count(ModuleId::Shipyard) == 0 {
                    return Err(Rejected("no shipyard"));
                }
                if self.ship.ship_weapon_tier >= 2 {
                    return Err(Rejected("weapon at max tier"));
                }
                let cost = vec![
                    (ResourceKind::Alloy, tuning.ship_weapon_upgrade_alloy),
                    (
                        ResourceKind::Components,
                        tuning.ship_weapon_upgrade_components,
                    ),
                ];
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                self.ship.ship_weapon_tier += 1;
                Ok(())
            }
            Action::UpgradeHullPlating => {
                if !self.flags.contains(&Flag::MunitionsKnown) {
                    return Err(Rejected("no munitions doctrine"));
                }
                if self.built_count(ModuleId::Shipyard) == 0 {
                    return Err(Rejected("no shipyard"));
                }
                if self.ship.hull_plating_tier >= 2 {
                    return Err(Rejected("hull plating at max tier"));
                }
                let cost = vec![
                    (ResourceKind::Alloy, tuning.hull_plating_upgrade_alloy),
                    (
                        ResourceKind::Components,
                        tuning.hull_plating_upgrade_components,
                    ),
                ];
                if !self.resources.try_spend_all(&cost) {
                    return Err(Rejected("not enough materials"));
                }
                self.ship.hull_plating_tier += 1;
                // hull_max(tuning) now reflects the incremented tier, so the full step always lands.
                self.ship.hull =
                    (self.ship.hull + tuning.hull_plating_hp_step).min(self.ship.hull_max(tuning));
                Ok(())
            }
            Action::CraftRepairDrone => {
                if !self.flags.contains(&Flag::MunitionsKnown) {
                    return Err(Rejected("no munitions doctrine"));
                }
                if self.built_count(ModuleId::Shipyard) == 0 {
                    return Err(Rejected("no shipyard"));
                }
                if !self
                    .resources
                    .try_spend_all(&[(ResourceKind::Components, tuning.repair_drone_cost)])
                {
                    return Err(Rejected("not enough materials"));
                }
                self.ship.repair_drones += 1;
                Ok(())
            }
            Action::ChooseEnding(ending) => {
                if self.endgame != Some(Endgame::WatchedArrived) {
                    return Err(Rejected("the watched is not here"));
                }
                if !self.available_endings(tuning).contains(&ending) {
                    return Err(Rejected("you have not earned that path"));
                }
                self.endgame = Some(Endgame::Ended(ending));
                let (title, body) = ending_text(self.origin.id, ending);
                self.note_once(ending_note_id(ending), title, body, tuning);
                self.log.push(
                    self.tick,
                    "The choice is made. There is no going back.",
                    tuning.log_capacity,
                );
                Ok(())
            }
        }
    }
}
