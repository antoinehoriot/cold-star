//! Balance harness: drive the real M3 content (tuning + jobs + modules + techs)
//! under fixed scripted policies and assert the economy behaves (reaches
//! milestones in windows; never permanently deadlocks). Tuning regression
//! test (spec §9).

use sim::combat::remnant_stats;
use sim::flags::Flag;
use sim::galaxy::{Biome, Site, SiteKind, SystemId};
use sim::job::JobId;
use sim::landing::{SurfaceSite, surface_seed};
use sim::module::ModuleId;
use sim::reactor::ReactorStatus;
use sim::research::TechId;
use sim::resources::ResourceKind;
use sim::state::{Action, GameState, World};

fn setup() -> (
    sim::Tuning,
    sim::job::JobRegistry,
    sim::module::ModuleRegistry,
    sim::research::TechRegistry,
) {
    (
        content::load().unwrap(),
        content::load_jobs().unwrap(),
        content::load_modules().unwrap(),
        content::load_techs().unwrap(),
    )
}

#[test]
fn salvager_economy_can_fund_a_refinery() {
    let (t, j, m, tc) = setup();
    let w = World {
        tuning: &t,
        jobs: &j,
        modules: &m,
        techs: &tc,
    };
    let mut s = GameState::new(&w, 42);

    // Ignite, gather crew the manual way isn't modeled here; grant a small crew
    // and idle them onto salvage, then run and assert Scrap accrues to fund a
    // Refinery within a generous window.
    s.crew.total = 3;
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Salvager,
            delta: 3,
        },
    )
    .unwrap();

    let mut built = false;
    for _ in 0..200 {
        s.advance(&w, 1);
        if !built && s.resources.amount(ResourceKind::Scrap) >= 20.0 {
            assert!(s.apply(&w, Action::Build(ModuleId::Refinery)).is_ok());
            built = true;
        }
    }
    assert!(
        built,
        "should fund a Refinery within 200 ticks under 3 salvagers"
    );
    assert_eq!(s.built_count(ModuleId::Refinery), 1);
}

#[test]
fn over_assigned_refiners_brownout_is_recoverable() {
    let (t, j, m, tc) = setup();
    let w = World {
        tuning: &t,
        jobs: &j,
        modules: &m,
        techs: &tc,
    };
    let mut s = GameState::new(&w, 7);
    s.crew.total = 6;
    // Build refinery, dump scrap + crew into Refiners to force overdraw.
    s.resources.add(ResourceKind::Scrap, 200.0);
    s.apply(&w, Action::Build(ModuleId::Refinery)).unwrap();
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Refiner,
            delta: 6,
        },
    )
    .unwrap();
    s.advance(&w, 50);
    // Overdraw drained the buffer — but the sim must not panic / deadlock and
    // charge stays in [0, capacity].
    assert!(s.reactor.charge >= 0.0 && s.reactor.charge <= s.reactor.capacity);
    // Rebalancing down to 1 Refiner recovers (no permanent stuck state).
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Refiner,
            delta: -5,
        },
    )
    .unwrap();
    s.advance(&w, 50);
    assert!(s.reactor.charge >= 0.0);
}

#[test]
fn can_research_core_tuning_with_a_scientist_economy() {
    let (tn, j, m) = (
        content::load().unwrap(),
        content::load_jobs().unwrap(),
        content::load_modules().unwrap(),
    );
    let tc = content::load_techs().unwrap();
    let w = World {
        tuning: &tn,
        jobs: &j,
        modules: &m,
        techs: &tc,
    };
    let mut s = GameState::new(&w, 99);
    // Bootstrap: build Fabricator (Scrap 20) then Lab (Alloy 20), then assign a Scientist.
    // Lab costs Alloy, not Scrap, so we pre-seed both resources.
    s.resources.add(ResourceKind::Scrap, 200.0);
    s.resources.add(ResourceKind::Alloy, 20.0);
    s.apply(&w, Action::Build(ModuleId::Fabricator)).unwrap();
    s.apply(&w, Action::Build(ModuleId::Lab)).unwrap();
    s.crew.total = 1;
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Scientist,
            delta: 1,
        },
    )
    .unwrap();
    s.resources.rations = 1000.0; // keep crew alive
    // Run until Data suffices, then research CoreTuning (cost 5).
    let mut started = false;
    for _ in 0..400 {
        s.advance(&w, 1);
        if !started && s.resources.amount(ResourceKind::Data) >= 5.0 {
            if s.apply(&w, Action::StartResearch(TechId::CoreTuning))
                .is_ok()
            {
                started = true;
            }
        }
    }
    assert!(
        started,
        "should accumulate Data and start research within 400 ticks"
    );
    assert!(
        s.research.is_completed(TechId::CoreTuning),
        "research should complete within the run"
    );
}

#[test]
fn atmosphere_crisis_is_survivable_by_repowering() {
    let (tn, j, m) = (
        content::load().unwrap(),
        content::load_jobs().unwrap(),
        content::load_modules().unwrap(),
    );
    let tc = content::load_techs().unwrap();
    let w = World {
        tuning: &tn,
        jobs: &j,
        modules: &m,
        techs: &tc,
    };
    // Start with a Cold reactor (zero charge) so the atmosphere clock begins
    // ticking immediately once the module is built.
    let cold_origin = sim::origin::OriginDef {
        start: sim::origin::OriginStart {
            scrap: 0.0,
            reactor_capacity: tn.reactor_capacity,
            reactor_charge: 0.0,
        },
        ..sim::origin::default_origin_def(&tn)
    };
    let mut s = GameState::new_with_origin(&cold_origin, &w, 5);
    // Atmosphere costs Alloy 15; seed it directly (no production chain needed).
    s.resources.add(ResourceKind::Alloy, 15.0);
    s.apply(&w, Action::Build(ModuleId::Atmosphere)).unwrap();
    s.crew.total = 3;
    // Let the Cold reactor + Atmosphere failure count down the grace clock, but
    // stop one tick before it expires.
    s.advance(&w, (tn.atmosphere_grace_max as u64) - 1); // clock nearly out, not yet
    let survivors = s.crew.total;
    // Re-power: ignite + feed enough charge to clear Cold and cover atmosphere.
    if s.reactor.status == ReactorStatus::Cold {
        s.apply(&w, Action::Ignite).ok();
    }
    s.resources.add(ResourceKind::Scrap, 200.0);
    for _ in 0..5 {
        let _ = s.apply(&w, Action::FeedCore);
    }
    s.advance(&w, 5);
    assert_eq!(
        s.crew.total, survivors,
        "repowering before the clock expires saves the crew"
    );
}

#[test]
fn m4_travel_loop_brings_isotopes_home() {
    let (t, j, m, tc) = setup();
    let w = World {
        tuning: &t,
        jobs: &j,
        modules: &m,
        techs: &tc,
    };
    // Seed 6 is pinned: its first reachable revealed neighbor holds an AsteroidField,
    // making the full loop (scan → jump → resolve → return with Isotopes) possible.
    // Seeds 4 and 5 have no asteroid field on their nearest reachable neighbor.
    let mut s = GameState::new(&w, 6);

    // Bootstrap to the stars with seeded resources (we test mechanics, not grind):
    // build the prerequisite chain and unlock jump theory + sensors.
    // Resources are added just before each build because the base storage cap is 100;
    // adding all up front would overflow and leave insufficient for later builds.
    s.flags.insert(sim::flags::Flag::JumpTheoryKnown);
    s.resources.add(ResourceKind::Scrap, 100.0);
    s.apply(&w, Action::Build(ModuleId::Fabricator)).unwrap(); // costs Scrap 20
    s.resources.add(ResourceKind::Alloy, 100.0);
    s.apply(&w, Action::Build(ModuleId::Lab)).unwrap(); // costs Alloy 20
    s.resources.add(ResourceKind::Alloy, 100.0);
    s.resources.add(ResourceKind::Data, 100.0);
    s.apply(&w, Action::Build(ModuleId::Assembler)).unwrap(); // costs Alloy 25 + Data 10
    s.resources.add(ResourceKind::Alloy, 100.0);
    s.resources.add(ResourceKind::Components, 100.0);
    s.resources.add(ResourceKind::Fuel, 100.0);
    s.apply(&w, Action::Build(ModuleId::Shipyard)).unwrap(); // costs Alloy 40 + Components 20 + Fuel 15
    s.resources.add(ResourceKind::Alloy, 100.0);
    s.resources.add(ResourceKind::Components, 100.0);
    s.apply(&w, Action::Build(ModuleId::SensorArray)).unwrap(); // costs Alloy 20 + Components 10
    assert!(s.ship.built, "shipyard must produce a ship");

    // Top up fuel so we have enough for both jumps plus reactor draw during transits.
    s.resources.add(ResourceKind::Fuel, 100.0);

    // Pick a revealed, reachable neighbor and scan it.
    let galaxy = s.galaxy(&w);
    let here = galaxy.get(s.expedition.location).unwrap();
    let target = s
        .galaxy_state
        .revealed
        .iter()
        .map(|&id| sim::galaxy::SystemId(id))
        .filter(|&id| id != s.expedition.location)
        .find(|&id| sim::galaxy::distance(here, galaxy.get(id).unwrap()) <= s.ship.drive_range)
        .expect("a reachable neighbor is revealed from home at base drive range");

    s.apply(&w, Action::ScanSystem(target)).unwrap();
    s.advance(&w, t.scan_ticks as u64);
    assert!(s.galaxy_state.scanned.contains(&target.0));

    // Helper: advance tick-by-tick until the ship's current activity completes,
    // capped at 500 ticks for safety. Real transits are ~8-12 ticks; site
    // resolves are driven by site_resolve_ticks (similarly short). This avoids
    // ~190 idle ticks of reactor-fuel burn that the old fixed-200 advances caused.
    macro_rules! advance_until_idle {
        ($s:expr, $w:expr) => {{
            let mut guard = 0u32;
            while $s.expedition.activity.is_some() && guard < 500 {
                $s.advance($w, 1);
                guard += 1;
            }
        }};
    }

    // Jump out, resolve every site present, jump home.
    s.apply(&w, Action::Jump(target)).unwrap();
    advance_until_idle!(s, &w);
    assert_eq!(s.expedition.location, target, "ship should arrive");

    let n_sites = s
        .galaxy_state
        .sites
        .get(&target.0)
        .map(|v| v.len())
        .unwrap_or(0);
    for idx in 0..n_sites {
        if s.apply(&w, Action::ResolveSite(target, idx)).is_ok() {
            advance_until_idle!(s, &w);
        }
    }

    // No fuel top-up here — all fuel was provisioned at home before departure.
    // With short transits (~8-12 ticks each) the round trip burns well under 100 units.
    let iso_home_before = s.resources.amount(ResourceKind::Isotopes);
    s.apply(&w, Action::Jump(sim::galaxy::HOME)).unwrap();
    advance_until_idle!(s, &w);
    assert_eq!(
        s.expedition.location,
        sim::galaxy::HOME,
        "ship should return home"
    );
    assert!(!s.away());

    // The station kept ticking and never deadlocked; charge stayed in range.
    assert!(s.reactor.charge >= 0.0 && s.reactor.charge <= s.reactor.capacity);

    // Seed 6's reachable neighbor is asteroid-bearing; this asserts the import
    // path end to end: scan → jump → resolve → return deposits Isotopes at home.
    assert!(
        s.resources.amount(ResourceKind::Isotopes) > iso_home_before,
        "an asteroid run should deposit Isotopes at the station"
    );
}

#[test]
fn m5_landing_is_survivable_and_profitable() {
    let (t, j, m, tc) = setup();
    let w = World {
        tuning: &t,
        jobs: &j,
        modules: &m,
        techs: &tc,
    };
    // Seed 6 is pinned: generates a Verdant surface where the greedy descend
    // banks enough Rations to out-earn an asteroid quick-resolve before Suit runs low.
    let mut s = GameState::new(&w, 6);
    s.ship.built = true;
    s.ship.cargo = 500.0;
    // Place the player at system 1 with a known Verdant planet.
    s.expedition.location = sim::galaxy::SystemId(1);
    let v = s.galaxy_state.sites.entry(1).or_default();
    v.push(sim::galaxy::Site::new(sim::galaxy::SiteKind::Planet(
        sim::galaxy::Biome::Verdant,
    )));
    let idx = v.len() - 1;

    s.apply(&w, Action::Land(idx)).unwrap();
    // Greedily descend: at each settled step, move to the first edge of the current node,
    // auto-pick Leave on any Life choice, and bail to Extract before the Suit is gone.
    let mut guard = 0;
    loop {
        guard += 1;
        assert!(guard < 200, "landing should terminate");
        let Some(l) = s.expedition.landing.clone() else {
            break;
        };
        if l.pending_choice {
            s.apply(&w, Action::LeaveLife).ok();
            continue;
        }
        if l.traverse > 0 {
            s.advance(&w, 1);
            continue;
        }
        // Extract once Suit is getting low, else push deeper.
        let low_suit = l.suit < t.suit_max * 0.45;
        let site = SurfaceSite::generate(
            surface_seed(s.world_seed, l.system, l.site_idx),
            l.biome,
            &t,
        );
        let next = site.get(l.node).and_then(|n| n.edges.first().copied());
        match next {
            Some(target) if !low_suit => {
                s.apply(&w, Action::MoveToNode(target)).ok();
                s.advance(&w, 1);
            }
            _ => {
                s.apply(&w, Action::Extract).unwrap();
                break;
            }
        }
    }
    // Banked a haul into ship cargo, and survived (not force-retreated empty if we extracted).
    let banked: f64 = s.expedition.cargo.values().copied().sum();
    assert!(
        banked > t.site_asteroid_isotopes,
        "a landing should out-earn an asteroid quick-resolve"
    );
}

/// CB1 — a geared 5-marine squad (weapon tier 1, armor tier 1) clears a
/// depth-1 Remnant without a wipe and banks Components+Data on Extract.
///
/// Seed 6. Squad: 5 crew, Aggressive stance, Rations in cargo.
/// Encounter injected directly (setup); fight resolved via advance().
#[test]
fn cb1_geared_squad_clears_a_remnant() {
    let (t, j, m, tc) = setup();
    let w = World {
        tuning: &t,
        jobs: &j,
        modules: &m,
        techs: &tc,
    };
    let mut s = GameState::new(&w, 6);

    // Bootstrap: build Fabricator then Assembler (Armory prerequisite).
    // Armory requires Assembler + CombatDoctrineKnown flag.
    s.resources.add(ResourceKind::Scrap, 100.0);
    s.apply(&w, Action::Build(ModuleId::Fabricator)).unwrap();

    s.resources.add(ResourceKind::Alloy, 100.0);
    s.resources.add(ResourceKind::Data, 100.0);
    s.apply(&w, Action::Build(ModuleId::Assembler)).unwrap(); // Alloy 25 + Data 10

    s.flags.insert(Flag::CombatDoctrineKnown);
    s.resources.add(ResourceKind::Alloy, 100.0);
    s.resources.add(ResourceKind::Components, 100.0);
    s.apply(&w, Action::Build(ModuleId::Armory)).unwrap(); // Alloy 30 + Components 18

    // Build a ship so we can land. All station management must happen while docked at HOME.
    s.ship.built = true;
    s.ship.cargo = 500.0;

    // Gear up at HOME: weapon tier 1 then armor tier 1.
    s.resources.add(ResourceKind::Alloy, 100.0);
    s.resources.add(ResourceKind::Components, 100.0);
    s.apply(&w, Action::UpgradeWeapon).unwrap();
    assert_eq!(s.gear.weapon_tier, 1);

    s.resources.add(ResourceKind::Alloy, 100.0);
    s.resources.add(ResourceKind::Components, 100.0);
    s.apply(&w, Action::UpgradeArmor).unwrap();
    assert_eq!(s.gear.armor_tier, 1);

    // Commit 5 crew to the away-team while still at HOME.
    s.crew.total = 6; // 1 spare for station
    s.apply(&w, Action::SetAwayCrew { delta: 5 }).unwrap();
    assert_eq!(s.expedition.away_team.crew, 5);

    // Stock 50 Rations in expedition cargo — enough for 5 crew over 60+ ticks
    // but leaves ample cargo space for the salvage Components and Data.
    s.expedition.cargo.insert(ResourceKind::Rations, 50.0);

    // Move expedition to a remote system and register a Ruined planet.
    s.expedition.location = SystemId(1);
    let sites = s.galaxy_state.sites.entry(1).or_default();
    sites.push(Site::new(SiteKind::Planet(Biome::Ruined)));

    // Land on the Ruined planet (site index 0).
    s.apply(&w, Action::Land(0)).unwrap();
    assert!(s.expedition.landing.is_some(), "should be on the surface");

    // Inject a depth-1 Remnant encounter directly (scenario setup — the fight
    // itself runs through advance/tick_combat below).
    {
        let landing = s.expedition.landing.as_mut().unwrap();
        landing.combat = Some(remnant_stats(1, 1.0, &t));
    }

    // Switch to Aggressive stance for maximum damage output.
    s.apply(&w, Action::SetStance(sim::combat::Stance::Aggressive))
        .unwrap();

    // Advance tick-by-tick until combat resolves (enemy defeated or wipe).
    // Cap at 200 ticks — a depth-1 fight should end in a handful of ticks.
    let crew_before = s.expedition.away_team.crew;
    for _ in 0..200 {
        let in_combat = s
            .expedition
            .landing
            .as_ref()
            .and_then(|l| l.combat.as_ref())
            .is_some();
        if !in_combat {
            break;
        }
        s.advance(&w, 1);
    }

    // Combat must have resolved (no combat field remaining).
    let combat_still_active = s
        .expedition
        .landing
        .as_ref()
        .and_then(|l| l.combat.as_ref())
        .is_some();
    assert!(
        !combat_still_active,
        "combat should have resolved within 200 ticks"
    );

    // Squad must NOT be wiped — at least 1 marine survived.
    assert!(
        s.expedition.away_team.crew > 0,
        "geared squad should survive a depth-1 Remnant (crew={crew_before} before fight)"
    );

    // The win path adds Components and Data to the satchel. Extract to bank them.
    let satchel_components = s
        .expedition
        .landing
        .as_ref()
        .and_then(|l| l.satchel.get(&ResourceKind::Components))
        .copied()
        .unwrap_or(0.0);
    let satchel_data = s
        .expedition
        .landing
        .as_ref()
        .and_then(|l| l.satchel.get(&ResourceKind::Data))
        .copied()
        .unwrap_or(0.0);
    assert!(
        satchel_components > 0.0,
        "win should bank Components in satchel (got {satchel_components})"
    );
    assert!(
        satchel_data > 0.0,
        "win should bank Data in satchel (got {satchel_data})"
    );

    // Extract — moves satchel into expedition cargo.
    s.apply(&w, Action::Extract).unwrap();
    let banked_components = s
        .expedition
        .cargo
        .get(&ResourceKind::Components)
        .copied()
        .unwrap_or(0.0);
    let banked_data = s
        .expedition
        .cargo
        .get(&ResourceKind::Data)
        .copied()
        .unwrap_or(0.0);
    assert!(
        banked_components > 0.0,
        "Components should be in cargo after Extract (got {banked_components})"
    );
    assert!(
        banked_data > 0.0,
        "Data should be in cargo after Extract (got {banked_data})"
    );
}

/// CB1 — a 2-crew no-gear squad facing a deep Remnant (depth 3) can Flee
/// immediately (player-initiated, before auto-resolve) and walks away with
/// the landing still active and crew intact. Validates the CB1 done-when:
/// "a player who avoids fights remains viable."
#[test]
fn cb1_weak_squad_can_flee() {
    let (t, j, m, tc) = setup();
    let w = World {
        tuning: &t,
        jobs: &j,
        modules: &m,
        techs: &tc,
    };
    let mut s = GameState::new(&w, 6);

    // Minimal bootstrap: ship built. Crew assignment must happen while docked at HOME.
    s.ship.built = true;
    s.ship.cargo = 500.0;

    // 2-crew away-team (1 spare for station), no gear (default tier 0).
    // 2 crew ensures a single tick cannot wipe the squad before we flee.
    // Must assign while at HOME.
    s.crew.total = 3; // 1 spare for station
    s.apply(&w, Action::SetAwayCrew { delta: 2 }).unwrap();
    assert_eq!(s.expedition.away_team.crew, 2);

    // Now move expedition to a remote system and register a Ruined planet.
    s.expedition.location = SystemId(2);
    let sites = s.galaxy_state.sites.entry(2).or_default();
    sites.push(Site::new(SiteKind::Planet(Biome::Ruined)));
    assert_eq!(s.gear.weapon_tier, 0);
    assert_eq!(s.gear.armor_tier, 0);

    // Stock minimal Rations so starvation doesn't confound the test.
    s.expedition.cargo.insert(ResourceKind::Rations, 500.0);

    // Land.
    s.apply(&w, Action::Land(0)).unwrap();
    assert!(s.expedition.landing.is_some());

    // Inject a deep (depth=3) Remnant — very dangerous for an unequipped squad.
    {
        let landing = s.expedition.landing.as_mut().unwrap();
        landing.combat = Some(remnant_stats(3, 1.0, &t));
    }

    // Confirm combat is now active BEFORE any advance tick.
    assert!(
        s.expedition
            .landing
            .as_ref()
            .and_then(|l| l.combat.as_ref())
            .is_some(),
        "combat must be active immediately after injection"
    );

    // Player-initiated Flee — UNCONDITIONAL, dispatched before any advance.
    // This is the "avoid the fight" path: the player sees danger and runs.
    // Must be accepted by the engine (combat is active).
    s.apply(&w, Action::Flee)
        .expect("Action::Flee must be accepted while combat is active");

    // Flee semantics (crew > 0 path): landing persists, combat is cleared.
    assert!(
        s.expedition.landing.is_some(),
        "landing must persist after a non-wipe flee (partial retreat keeps the surface)"
    );
    let combat_active = s
        .expedition
        .landing
        .as_ref()
        .and_then(|l| l.combat.as_ref())
        .is_some();
    assert!(!combat_active, "combat must be cleared after Flee");

    // Squad survived: player can continue.
    assert!(
        s.expedition.away_team.crew > 0,
        "away_team.crew must be > 0 after fleeing"
    );
    assert!(
        s.crew.total > 0,
        "crew.total must be > 0 after fleeing (player remains viable)"
    );

    // Extract still works — confirms the landing is navigable post-flee.
    s.apply(&w, Action::Extract)
        .expect("Extract must succeed after fleeing (landing still active)");
    assert!(
        s.expedition.landing.is_none(),
        "landing should be cleared after Extract"
    );
}
