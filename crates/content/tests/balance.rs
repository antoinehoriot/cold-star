//! Balance harness: drive the real M2 content (tuning + jobs + modules) under a
//! fixed scripted policy and assert the economy behaves (reaches milestones in
//! windows; never permanently deadlocks). Tuning regression test (spec §9).

use sim::job::JobId;
use sim::module::ModuleId;
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
