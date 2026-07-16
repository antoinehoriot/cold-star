use super::*;

impl GameState {
    pub fn advance(&mut self, world: &World, ticks: u64) {
        let tuning = world.tuning;
        for _ in 0..ticks {
            let before: [f64; 7] = ResourceKind::ALL.map(|k| self.resources.amount(k));
            let prev_status = self.reactor.status;
            self.tick += 1;
            if self.salvage_cooldown > 0 {
                self.salvage_cooldown -= 1;
            }

            // --- crew upkeep (Rations) -> starving efficiency ---
            let upkeep = self.crew.ration_upkeep(tuning);
            let starving = !self.resources.try_spend(ResourceKind::Rations, upkeep);
            let efficiency = if starving {
                tuning.starving_efficiency
            } else {
                1.0
            };

            // --- power budget ---
            let consumers = self.ordered_consumers(world);
            let outcome = resolve_power(
                &consumers,
                self.reactor.max_output,
                self.reactor.charge,
                self.reactor.capacity,
                self.resources.fuel,
                tuning.fuel_per_output,
                tuning.reactor_baseline_draw,
            );
            self.resources.fuel = (self.resources.fuel - outcome.fuel_burned).max(0.0);
            // Overdraw status is driven by consumer demand vs generation only;
            // the baseline is the reactor's own idle cost and does not trigger
            // the Straining/Critical branch.
            let overdrawn = outcome.consumer_draw > outcome.generation;
            self.reactor
                .set_from_power(outcome.new_charge, overdrawn, tuning);

            // --- thermal failure: unpowered thermal lets the cold creep back ---
            if self.has_module(ModuleId::Thermal) && outcome.paused.contains(&Consumer::Thermal) {
                self.reactor.drain(tuning.thermal_failure_drain, tuning);
            }

            // --- economy (two-phase) ---
            let report = run_economy(
                &self.crew.assignments,
                world.jobs,
                &outcome.paused,
                efficiency,
                &self.research_output_mult,
                &mut self.resources,
            );

            // Track cumulative scrap gained this tick (for the heavy-consumption note).
            let scrap_after = self.resources.amount(ResourceKind::Scrap);
            let scrap_before = before[0]; // ResourceKind::ALL[0] == Scrap
            if scrap_after > scrap_before {
                self.total_scrap_gained += scrap_after - scrap_before;
            }

            // --- research progress ---
            if let Some(active) = self.research.active.as_mut() {
                if active.ticks_remaining > 0 {
                    active.ticks_remaining -= 1;
                }
                if active.ticks_remaining == 0 {
                    let id = active.tech;
                    self.research.active = None;
                    self.research.completed.insert(id);
                    if let Some(tech) = world.techs.get(id) {
                        let grant = tech.grant.clone();
                        self.apply_grant(&grant);
                        let (name, lore) = (tech.name.clone(), tech.lore.clone());
                        self.log.push(
                            self.tick,
                            format!("Research complete: {name}"),
                            tuning.log_capacity,
                        );
                        self.codex.add(Discovery {
                            id: DiscoveryId::Tech(id),
                            title: name,
                            text: lore,
                            tick: self.tick,
                        });
                    }
                }
            }

            // --- expedition activity (jump/scan/resolve/scavenge timers) ---
            self.tick_expedition(world);

            // --- surface landing (Suit drain + forced retreat) ---
            self.tick_landing(world);

            // --- away-team rations consumption + starvation ---
            self.tick_away_team(world);

            // --- docked hull regen ---
            self.tick_hull_regen(world);

            // --- atmosphere failure: survival clock -> crew loss ---
            let atmosphere_failing = self.has_module(ModuleId::Atmosphere)
                && self.crew.total > 0
                && (self.reactor.status == ReactorStatus::Cold
                    || outcome.paused.contains(&Consumer::Atmosphere));
            if atmosphere_failing {
                if self.atmosphere_grace > 0 {
                    self.atmosphere_grace -= 1;
                    if self.atmosphere_grace == 0 {
                        self.crew_loss_timer = tuning.crew_loss_interval;
                        self.log.push(
                            self.tick,
                            "ATMOSPHERE FAILING. The air is going.",
                            tuning.log_capacity,
                        );
                    }
                } else {
                    // Clock exhausted: lose a crew every crew_loss_interval ticks.
                    if self.crew_loss_timer > 0 {
                        self.crew_loss_timer -= 1;
                    }
                    if self.crew_loss_timer == 0 && self.crew.lose_one() {
                        self.crew_loss_timer = tuning.crew_loss_interval;
                        self.note_first_crew_death(tuning);
                        self.log.push(
                            self.tick,
                            "Someone stops breathing in the dark.",
                            tuning.log_capacity,
                        );
                    }
                }
            } else {
                // Powered again: reset the clock.
                self.atmosphere_grace = tuning.atmosphere_grace_max;
                self.crew_loss_timer = 0;
            }

            // --- triggered narrative notes (each fires once) ---
            self.check_triggered_notes(tuning);

            // --- crew arrivals (capped by housing) ---
            let housing = self.housing(world);
            if self.flags.contains(&Flag::CrewSourceActive)
                && self.crew.tick_arrival(housing, tuning)
            {
                self.log_flavor("crew_arrived", tuning);
            }

            // --- status transition logging ---
            if self.reactor.status != prev_status {
                match self.reactor.status {
                    ReactorStatus::Failing => self.log_flavor("failing", tuning),
                    ReactorStatus::Cold => self.log_flavor("cold", tuning),
                    _ => {}
                }
            }
            let mut net = std::collections::BTreeMap::new();
            for (i, k) in ResourceKind::ALL.iter().enumerate() {
                let delta = self.resources.amount(*k) - before[i];
                if delta != 0.0 {
                    net.insert(*k, delta);
                }
            }
            self.telemetry = Telemetry {
                generation: outcome.generation,
                consumer_draw: outcome.consumer_draw,
                active_draw: outcome.active_draw,
                paused: outcome.paused.clone(),
                starved_input: report.starved_input.clone(),
                ran: report.ran.clone(),
                net,
                starving,
            };
            // NOTE: telemetry is fresh-written just above; update_disclosure_flags
            // reads telemetry.consumer_draw for PowerBudgetVisible. Do not reorder.
            self.update_disclosure_flags(world);
        }
    }

    /// Advance the single expedition activity by one tick; apply its effect on
    /// completion (mutate-on-completion). Deterministic: one decrement per tick.
    fn tick_expedition(&mut self, world: &World) {
        let tuning = world.tuning;
        // Ship combat runs while activity is None (player has ARRIVED but a
        // contact fight is active). Guard must precede the activity match to
        // avoid the early `None => return` below consuming the tick.
        if self
            .expedition
            .contact
            .as_ref()
            .and_then(|c| c.combat.as_ref())
            .is_some()
        {
            self.tick_ship_combat(world);
            return;
        }
        let remaining = match &mut self.expedition.activity {
            None => return,
            Some(Activity::Transit { ticks, .. })
            | Some(Activity::Scan { ticks, .. })
            | Some(Activity::ResolveSite { ticks, .. })
            | Some(Activity::Scavenge { ticks }) => {
                if *ticks > 0 {
                    *ticks -= 1;
                }
                *ticks
            }
        };
        if remaining > 0 {
            return;
        }
        let finished = self.expedition.activity.take().expect("activity present");
        match finished {
            Activity::Transit { to, .. } => {
                self.expedition.prev_location = self.expedition.location;
                self.expedition.location = to;
                // Take `revealed` out while the memoized galaxy borrows `self`.
                let mut revealed = std::mem::take(&mut self.galaxy_state.revealed);
                let star_type = {
                    let galaxy = self.galaxy(world.tuning);
                    reveal_from(galaxy, to, tuning.galaxy_reveal_radius, &mut revealed);
                    galaxy.get(to).map(|sys| sys.star_type)
                };
                self.galaxy_state.revealed = revealed;
                if let std::collections::btree_map::Entry::Vacant(e) =
                    self.galaxy_state.sites.entry(to.0)
                    && let Some(star_type) = star_type
                {
                    e.insert(materialize_sites(self.world_seed, to.0, star_type, tuning));
                }
                if to == HOME && !self.expedition.cargo.is_empty() {
                    for (k, amt) in std::mem::take(&mut self.expedition.cargo) {
                        self.resources.add(k, amt);
                    }
                    self.log.push(
                        self.tick,
                        "You unload the haul into the station's holds.",
                        tuning.log_capacity,
                    );
                }
                if to == HOME && self.expedition.away_team.crew > 0 {
                    self.crew.release_away(self.expedition.away_team.crew);
                    self.expedition.away_team = crate::expedition::AwayTeam::default();
                }
                self.log.push(
                    self.tick,
                    "The drive cuts out. A new star fills the port.",
                    tuning.log_capacity,
                );
                self.maybe_start_contact(world, to);
                self.maybe_trigger_watched(world, to);
            }
            Activity::Scan { target, .. } => {
                self.galaxy_state.scanned.insert(target.0);
                let star_type = self
                    .galaxy(world.tuning)
                    .get(target)
                    .map(|sys| sys.star_type);
                if let std::collections::btree_map::Entry::Vacant(e) =
                    self.galaxy_state.sites.entry(target.0)
                    && let Some(star_type) = star_type
                {
                    e.insert(materialize_sites(
                        self.world_seed,
                        target.0,
                        star_type,
                        tuning,
                    ));
                }
                self.log.push(
                    self.tick,
                    "The scan resolves. You see what waits there now.",
                    tuning.log_capacity,
                );
            }
            Activity::ResolveSite {
                system, site_idx, ..
            } => {
                let grad = self.system_gradient(world, system);
                let kind = self
                    .galaxy_state
                    .sites
                    .get(&system.0)
                    .and_then(|v| v.get(site_idx))
                    .map(|s| s.kind);
                if let Some(kind) = kind {
                    let yields: Vec<(ResourceKind, f64)> = match kind {
                        crate::galaxy::SiteKind::AsteroidField => vec![
                            (ResourceKind::Isotopes, tuning.site_asteroid_isotopes * grad),
                            (ResourceKind::Fuel, tuning.site_asteroid_fuel * grad),
                        ],
                        crate::galaxy::SiteKind::Derelict => vec![
                            (
                                ResourceKind::Components,
                                tuning.site_derelict_components * grad,
                            ),
                            (ResourceKind::Data, tuning.site_derelict_data * grad),
                        ],
                        crate::galaxy::SiteKind::Planet(_) => vec![],
                    };
                    for (k, amt) in yields {
                        let space = (self.ship.cargo - self.expedition.cargo_total()).max(0.0);
                        let take = amt.min(space);
                        if take > 0.0 {
                            *self.expedition.cargo.entry(k).or_insert(0.0) += take;
                        }
                    }
                    // A Planet is landed on, never quick-resolved — defensively
                    // refuse to mark it depleted even if this arm is somehow reached.
                    if !matches!(kind, crate::galaxy::SiteKind::Planet(_))
                        && let Some(site) = self
                            .galaxy_state
                            .sites
                            .get_mut(&system.0)
                            .and_then(|v| v.get_mut(site_idx))
                    {
                        site.depleted = true;
                    }
                    if kind == crate::galaxy::SiteKind::Derelict {
                        self.note_first_derelict(tuning);
                    }
                    self.log.push(
                        self.tick,
                        "You strip what you can carry and move on.",
                        tuning.log_capacity,
                    );
                }
            }
            Activity::Scavenge { .. } => {
                self.resources.add(ResourceKind::Fuel, tuning.scavenge_fuel);
                self.log.push(
                    self.tick,
                    "You bleed fuel from cold tanks and wreckage. Enough to limp on.",
                    tuning.log_capacity,
                );
            }
        }
    }

    /// Per-tick away-team upkeep:
    /// - No squad (crew == 0): nothing.
    /// - Docked with squad: morale recovers toward max.
    /// - Away with squad: eat ship Rations; starve if out (morale + HP attrition,
    ///   then a casualty). Deterministic per tick.
    fn tick_away_team(&mut self, world: &World) {
        let tuning = world.tuning;
        if self.expedition.away_team.crew == 0 {
            return;
        }
        if !self.away() {
            // Docked with a committed squad: morale recovery.
            self.expedition.away_team.morale = (self.expedition.away_team.morale
                + tuning.morale_recover_per_tick)
                .min(tuning.morale_max);
            return;
        }
        // Away: Rations billing.
        let need = self.expedition.away_team.crew as f64 * tuning.marine_ration_upkeep;
        let have = self
            .expedition
            .cargo
            .get(&ResourceKind::Rations)
            .copied()
            .unwrap_or(0.0);
        if have >= need {
            *self
                .expedition
                .cargo
                .entry(ResourceKind::Rations)
                .or_insert(0.0) -= need;
        } else {
            self.expedition.cargo.remove(&ResourceKind::Rations);
            // Starvation.
            self.expedition.away_team.morale =
                (self.expedition.away_team.morale - tuning.morale_per_starve_tick).max(0.0);
            self.expedition.away_team.hp -= tuning.starve_hp_per_tick;
            if self.expedition.away_team.hp <= 0.0 && self.crew.kill_away() {
                self.expedition.away_team.crew = self.expedition.away_team.crew.saturating_sub(1);
                self.expedition.away_team.morale =
                    (self.expedition.away_team.morale - tuning.morale_per_death).max(0.0);
                self.expedition.away_team.hp = self.squad_hp_max(tuning);
            }
        }
    }

    /// Regenerate hull HP while docked at HOME with no activity or contact.
    /// Deterministic: fixed arithmetic, no RNG.
    fn tick_hull_regen(&mut self, world: &World) {
        let tuning = world.tuning;
        if self.expedition.location == crate::galaxy::HOME
            && self.expedition.activity.is_none()
            && self.expedition.contact.is_none()
            && self.ship.hull < self.ship.hull_max(tuning)
        {
            self.ship.hull =
                (self.ship.hull + tuning.hull_regen_docked).min(self.ship.hull_max(tuning));
        }
    }

    fn tick_landing(&mut self, world: &World) {
        let tuning = world.tuning;
        // Suit drain (always runs first).
        {
            let Some(landing) = self.expedition.landing.as_mut() else {
                return;
            };
            landing.suit -= tuning.suit_drain_per_tick;
        }
        if self
            .expedition
            .landing
            .as_ref()
            .is_some_and(|l| l.suit <= 0.0)
        {
            self.expedition.landing = None;
            self.log.push(
                self.tick,
                "Suit critical. You scramble back empty-handed.",
                tuning.log_capacity,
            );
            return;
        }
        // Combat consumes the tick — no traverse while fighting.
        if self
            .expedition
            .landing
            .as_ref()
            .and_then(|l| l.combat.as_ref())
            .is_some()
        {
            self.tick_combat(world);
            return;
        }
        let traverse_done = self
            .expedition
            .landing
            .as_ref()
            .is_some_and(|l| l.traverse > 0)
            && {
                let l = self.expedition.landing.as_mut().unwrap();
                l.traverse -= 1;
                l.traverse == 0
            };
        if traverse_done {
            let (system, site_idx, biome, node) = {
                let l = self.expedition.landing.as_ref().unwrap();
                (l.system, l.site_idx, l.biome, l.node)
            };
            self.resolve_node(world, system, site_idx, biome, node);
            // A fatal hazard this tick must force the retreat NOW.
            if self
                .expedition
                .landing
                .as_ref()
                .is_some_and(|l| l.suit <= 0.0)
            {
                self.expedition.landing = None;
                self.log.push(
                    self.tick,
                    "Suit critical. You scramble back empty-handed.",
                    tuning.log_capacity,
                );
            }
        }
    }
}
