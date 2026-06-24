//! Balance harness: drive the real M3 content (tuning + jobs + modules + techs)
//! under fixed scripted policies and assert the economy behaves (reaches
//! milestones in windows; never permanently deadlocks). Tuning regression
//! test (spec §9).

use sim::job::JobId;
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
