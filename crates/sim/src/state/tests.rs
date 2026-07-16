use super::*;

fn jobs() -> JobRegistry {
    content_jobs()
}
fn modules() -> ModuleRegistry {
    content_modules()
}
// Minimal in-test registries mirroring content (keeps sim tests self-contained).
fn content_jobs() -> JobRegistry {
    use crate::job::{Job, Recipe};
    JobRegistry(vec![
        Job {
            id: JobId::Salvager,
            name: "Salvager".into(),
            gated_by: None,
            recipe: Recipe {
                inputs: vec![],
                outputs: vec![(ResourceKind::Scrap, 1.0)],
            },
            power_draw: 0.0,
            default_priority: 6,
        },
        Job {
            id: JobId::Refiner,
            name: "Refiner".into(),
            gated_by: Some(ModuleId::Refinery),
            recipe: Recipe {
                inputs: vec![(ResourceKind::Scrap, 2.0)],
                outputs: vec![(ResourceKind::Fuel, 1.0)],
            },
            power_draw: 2.0,
            default_priority: 2,
        },
    ])
}
fn content_modules() -> ModuleRegistry {
    use crate::module::{Module, ModuleUnlocks};
    ModuleRegistry(vec![
        Module {
            id: ModuleId::Refinery,
            name: "Refinery".into(),
            description: "test".into(),
            tier: 1,
            cost: vec![(ResourceKind::Scrap, 20.0)],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: None,
            unlocks: ModuleUnlocks {
                job: Some(JobId::Refiner),
                ..Default::default()
            },
        },
        Module {
            id: ModuleId::Quarters,
            name: "Quarters".into(),
            description: "test".into(),
            tier: 1,
            cost: vec![(ResourceKind::Scrap, 10.0)],
            cost_scaling: 1.6,
            repeatable: true,
            requires_modules: vec![],
            requires_flag: None,
            unlocks: ModuleUnlocks {
                housing: 4,
                ..Default::default()
            },
        },
        Module {
            id: ModuleId::Lab,
            name: "Lab".into(),
            description: "test".into(),
            tier: 2,
            cost: vec![(ResourceKind::Scrap, 20.0)],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: None,
            unlocks: ModuleUnlocks {
                job: Some(JobId::Scientist),
                ..Default::default()
            },
        },
        Module {
            id: ModuleId::Atmosphere,
            name: "Atmosphere".into(),
            description: "test".into(),
            tier: 1,
            cost: vec![(ResourceKind::Scrap, 10.0)],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: None,
            unlocks: ModuleUnlocks::default(),
        },
        Module {
            id: ModuleId::Thermal,
            name: "Thermal".into(),
            description: "test".into(),
            tier: 1,
            cost: vec![(ResourceKind::Scrap, 10.0)],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: None,
            unlocks: ModuleUnlocks::default(),
        },
        Module {
            id: ModuleId::Shipyard,
            name: "Shipyard".into(),
            description: "test".into(),
            tier: 2,
            cost: vec![(ResourceKind::Alloy, 10.0)],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: Some(Flag::JumpTheoryKnown),
            unlocks: ModuleUnlocks::default(),
        },
        Module {
            id: ModuleId::SensorArray,
            name: "Sensor Array".into(),
            description: "test".into(),
            tier: 2,
            cost: vec![(ResourceKind::Alloy, 10.0)],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: Some(Flag::JumpTheoryKnown),
            unlocks: ModuleUnlocks::default(),
        },
        Module {
            id: ModuleId::Armory,
            name: "Armory".into(),
            description: "test".into(),
            tier: 2,
            cost: vec![(ResourceKind::Alloy, 10.0)],
            cost_scaling: 1.0,
            repeatable: false,
            requires_modules: vec![],
            requires_flag: Some(Flag::CombatDoctrineKnown),
            unlocks: ModuleUnlocks::default(),
        },
    ])
}
fn techs() -> crate::research::TechRegistry {
    use crate::research::{Grant, Tech, TechId};
    crate::research::TechRegistry(vec![
        Tech {
            id: TechId::CoreTuning,
            name: "Core Tuning".into(),
            blurb: "".into(),
            data_cost: 5.0,
            research_ticks: 3,
            requires: vec![],
            grant: Grant::ReactorMaxOutput(2.0),
            lore: "lore".into(),
        },
        Tech {
            id: TechId::RefinedProcesses,
            name: "Refined".into(),
            blurb: "".into(),
            data_cost: 6.0,
            research_ticks: 2,
            requires: vec![],
            grant: Grant::JobOutputMult {
                job: JobId::Salvager,
                mult: 2.0,
            },
            lore: "lore".into(),
        },
    ])
}
fn world<'a>(
    t: &'a Tuning,
    j: &'a JobRegistry,
    m: &'a ModuleRegistry,
    tc: &'a crate::research::TechRegistry,
) -> World<'a> {
    World {
        tuning: t,
        jobs: j,
        modules: m,
        techs: tc,
    }
}

fn sim_home() -> crate::galaxy::SystemId {
    crate::galaxy::HOME
}

/// Find a revealed, reachable, non-home system for a given seed (drive range
/// is generous vs reveal radius in defaults, so a revealed neighbor is reachable).
fn a_reachable_target(s: &GameState, w: &World) -> crate::galaxy::SystemId {
    let g = s.galaxy(w.tuning);
    let here = g.get(s.expedition.location).unwrap();
    s.galaxy_state
        .revealed
        .iter()
        .map(|&id| crate::galaxy::SystemId(id))
        .filter(|&id| id != s.expedition.location)
        .find(|&id| crate::galaxy::distance(here, g.get(id).unwrap()) <= s.ship.drive_range)
        .expect("a reachable revealed neighbor exists for this seed")
}

#[test]
fn jump_costs_fuel_transits_then_arrives() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 4);
    s.resources.add(ResourceKind::Fuel, 100.0);
    let target = a_reachable_target(&s, &w);
    let g = s.galaxy(w.tuning);
    let dist = crate::galaxy::distance(g.get(crate::galaxy::HOME).unwrap(), g.get(target).unwrap());
    let cost = crate::galaxy::jump_cost(dist, &t);
    let fuel_before = s.resources.amount(ResourceKind::Fuel);
    s.apply(&w, Action::Jump(target)).unwrap();
    // Fuel deducted immediately; still at home, now in transit.
    assert!((fuel_before - s.resources.amount(ResourceKind::Fuel) - cost).abs() < 1e-9);
    assert_eq!(s.expedition.location, crate::galaxy::HOME);
    assert!(s.away());
    let ticks = t.transit_ticks_base + (dist * t.transit_ticks_factor).round() as u32;
    s.advance(&w, (ticks - 1) as u64);
    assert_ne!(s.expedition.location, target); // not yet
    s.advance(&w, 1);
    assert_eq!(s.expedition.location, target); // arrived
    assert!(s.expedition.activity.is_none());
    assert!(s.galaxy_state.sites.contains_key(&target.0)); // materialized on arrival
}

#[test]
fn jump_rejected_when_unreachable_or_unaffordable() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 4);
    let target = a_reachable_target(&s, &w);
    // No fuel.
    assert_eq!(
        s.apply(&w, Action::Jump(target)),
        Err(Rejected("not enough fuel to jump"))
    );
}

#[test]
fn new_game_initializes_m4_state() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let s = GameState::new(&w, 77);
    assert_eq!(s.world_seed, 77);
    assert!(!s.ship.built);
    assert_eq!(s.ship.drive_range, t.ship_base_drive_range);
    assert_eq!(s.expedition.location, sim_home());
    assert!(s.expedition.activity.is_none());
    assert!(!s.away());
    // Home is revealed at the start; the galaxy is regenerable, not stored.
    assert!(s.galaxy_state.revealed.contains(&0));
    assert_eq!(
        s.galaxy(w.tuning).systems.len(),
        t.galaxy_system_count as usize
    );
}

#[test]
fn new_game_is_versioned_v11() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let s = GameState::new(&world(&t, &j, &m, &tc), 1);
    assert_eq!(s.version, 11);
    assert_eq!(s.resources.cap, t.resource_base_cap);
}

#[test]
fn advance_n_equals_n_times_advance_1() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let mut bulk = GameState::new(&world(&t, &j, &m, &tc), 1);
    let mut step = GameState::new(&world(&t, &j, &m, &tc), 1);
    bulk.advance(&world(&t, &j, &m, &tc), 25);
    for _ in 0..25 {
        step.advance(&world(&t, &j, &m, &tc), 1);
    }
    assert_eq!(bulk, step);
}

#[test]
fn build_refinery_unlocks_refiner_assignment() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    // Need scrap + idle crew.
    s.resources.add(ResourceKind::Scrap, 30.0);
    s.crew.total = 2;
    // Can't assign Refiner before building Refinery.
    assert!(
        s.apply(
            &w,
            Action::AssignCrew {
                job: JobId::Refiner,
                delta: 1
            }
        )
        .is_err()
    );
    assert!(s.apply(&w, Action::Build(ModuleId::Refinery)).is_ok());
    assert_eq!(s.built_count(ModuleId::Refinery), 1);
    assert!(
        s.apply(
            &w,
            Action::AssignCrew {
                job: JobId::Refiner,
                delta: 1
            }
        )
        .is_ok()
    );
    assert_eq!(s.crew.assigned(JobId::Refiner), 1);
}

#[test]
fn repeatable_build_scales_cost() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.resources.add(ResourceKind::Scrap, 100.0);
    let before = s.resources.scrap;
    s.apply(&w, Action::Build(ModuleId::Quarters)).unwrap(); // 10
    let after1 = s.resources.scrap;
    s.apply(&w, Action::Build(ModuleId::Quarters)).unwrap(); // 10 * 1.6
    let after2 = s.resources.scrap;
    assert!((before - after1 - 10.0).abs() < 1e-9);
    assert!((after1 - after2 - 16.0).abs() < 1e-9);
}

#[test]
fn assigned_salvagers_produce_scrap_over_time() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.crew.total = 2;
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Salvager,
            delta: 2,
        },
    )
    .unwrap();
    let before = s.resources.scrap;
    s.advance(&w, 10);
    assert!(s.resources.scrap > before); // 2 salvagers × 1/tick × 10
}

#[test]
fn priority_reorder_via_action() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let c = Consumer::Job(JobId::Refiner);
    let before = s.priority.rank(c).unwrap();
    s.apply(
        &w,
        Action::SetPriority {
            consumer: c,
            up: true,
        },
    )
    .unwrap();
    assert!(s.priority.rank(c).unwrap() <= before);
}

fn bank(s: &mut GameState, w: &World, tn: &Tuning, times: u32) {
    for _ in 0..times {
        s.apply(w, Action::Salvage).unwrap();
        s.advance(w, tn.salvage_cooldown_ticks as u64);
    }
}

#[test]
fn advance_accumulates_ticks() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.advance(&w, 5);
    assert_eq!(s.tick, 5);
}

#[test]
fn ignite_only_from_cold() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let origin = OriginDef {
        start: crate::origin::OriginStart {
            scrap: 0.0,
            reactor_capacity: 60.0,
            reactor_charge: 0.0,
        },
        ..default_origin_def(&t)
    };
    let mut s = GameState::new_with_origin(&origin, &w, 1);
    assert_eq!(s.reactor.status, ReactorStatus::Cold);
    assert!(s.apply(&w, Action::Ignite).is_ok());
    assert_eq!(s.reactor.status, ReactorStatus::Igniting);
    assert!(s.flags.contains(&Flag::SalvageUnlocked));
    assert_eq!(
        s.apply(&w, Action::Ignite),
        Err(Rejected("the core is already lit"))
    );
}

#[test]
fn feed_rejected_without_scrap() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    assert_eq!(s.resources.scrap, 0.0);
    assert_eq!(
        s.apply(&w, Action::FeedCore),
        Err(Rejected("not enough scrap to feed the core"))
    );
}

#[test]
fn salvage_sets_cooldown_and_blocks_repeat() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    assert!(s.apply(&w, Action::Salvage).is_ok());
    assert_eq!(s.salvage_cooldown, t.salvage_cooldown_ticks);
    assert_eq!(
        s.apply(&w, Action::Salvage),
        Err(Rejected("salvage is on cooldown"))
    );
    s.advance(&w, t.salvage_cooldown_ticks as u64);
    assert_eq!(s.salvage_cooldown, 0);
    assert!(s.apply(&w, Action::Salvage).is_ok());
}

#[test]
fn crew_source_requires_availability_and_scrap() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    assert_eq!(
        s.apply(&w, Action::ActivateCrewSource),
        Err(Rejected("the crew-source is not available yet"))
    );
    bank(
        &mut s,
        &w,
        &t,
        (t.crew_source_cost / t.salvage_yield) as u32,
    );
    assert!(s.flags.contains(&Flag::CrewSourceAvailable));
    assert!(s.apply(&w, Action::ActivateCrewSource).is_ok());
    assert_eq!(
        s.apply(&w, Action::ActivateCrewSource),
        Err(Rejected("the crew-source is already active"))
    );
}

#[test]
fn hot_start_below_establish_is_igniting_and_unlocked() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let origin = OriginDef {
        start: crate::origin::OriginStart {
            scrap: 15.0,
            reactor_capacity: 140.0,
            reactor_charge: 35.0,
        },
        ..default_origin_def(&t)
    };
    let s = GameState::new_with_origin(&origin, &w, 1);
    assert_eq!(s.reactor.status, ReactorStatus::Igniting);
    assert!(!s.reactor.established);
    assert!(s.flags.contains(&Flag::SalvageUnlocked));
    assert!(!s.flags.contains(&Flag::CoreEstablished));
}

#[test]
fn telemetry_records_last_tick() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.crew.total = 2;
    s.resources.rations = 1.0; // ensure crew are not starving (upkeep = 2 × 0.1 = 0.2)
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Salvager,
            delta: 2,
        },
    )
    .unwrap();
    s.advance(&w, 1);
    // 2 salvagers produced +2 scrap this tick.
    assert_eq!(
        s.telemetry
            .net
            .get(&ResourceKind::Scrap)
            .copied()
            .unwrap_or(0.0),
        2.0
    );
    assert!(s.telemetry.ran.contains(&JobId::Salvager));
    // Salvager draws no power, so consumer_draw is 0 and nothing is paused.
    assert_eq!(s.telemetry.consumer_draw, 0.0);
    assert!(s.telemetry.paused.is_empty());
}

#[test]
fn telemetry_is_deterministic_across_batching() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let mut bulk = GameState::new(&world(&t, &j, &m, &tc), 1);
    let mut step = GameState::new(&world(&t, &j, &m, &tc), 1);
    bulk.advance(&world(&t, &j, &m, &tc), 10);
    for _ in 0..10 {
        step.advance(&world(&t, &j, &m, &tc), 1);
    }
    assert_eq!(bulk.telemetry, step.telemetry);
    assert_eq!(bulk, step);
}

#[test]
fn m2_disclosure_flags_set_at_milestones() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1); // full-charge default origin -> established
    // BuildMenuUnlocked is set once the core is established.
    assert!(s.flags.contains(&Flag::BuildMenuUnlocked));
    // No economy / power / tier-2 yet.
    assert!(!s.flags.contains(&Flag::EconomyUnlocked));
    assert!(!s.flags.contains(&Flag::PowerBudgetVisible));
    assert!(!s.flags.contains(&Flag::Tier2Unlocked));

    // Build a Refinery (job-unlocking module) -> EconomyUnlocked.
    s.resources.add(ResourceKind::Scrap, 30.0);
    s.apply(&w, Action::Build(ModuleId::Refinery)).unwrap();
    assert!(s.flags.contains(&Flag::EconomyUnlocked));

    // Assign a Refiner with scrap + crew and advance -> a power-drawing job
    // runs -> PowerBudgetVisible.
    s.crew.total = 1;
    s.resources.add(ResourceKind::Scrap, 20.0);
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Refiner,
            delta: 1,
        },
    )
    .unwrap();
    s.advance(&w, 1);
    assert!(s.flags.contains(&Flag::PowerBudgetVisible));

    // Data present -> Tier2Unlocked.
    s.resources.add(ResourceKind::Data, 1.0);
    s.advance(&w, 1);
    assert!(s.flags.contains(&Flag::Tier2Unlocked));
}

#[test]
fn m2_flag_graph_is_reachable_from_a_running_core() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    // A full-charge default origin is already established (BuildMenuUnlocked).
    let mut s = GameState::new(&w, 1);
    assert!(s.flags.contains(&Flag::BuildMenuUnlocked));

    // BuildMenu is reachable -> build a Refinery (Scrap-only, no prereqs).
    s.resources.add(ResourceKind::Scrap, 30.0);
    s.apply(&w, Action::Build(ModuleId::Refinery)).unwrap();
    assert!(s.flags.contains(&Flag::EconomyUnlocked));

    // Economy reachable -> assign a Refiner; feed it scrap + rations; advance.
    s.crew.total = 1;
    s.resources.add(ResourceKind::Scrap, 20.0);
    s.resources.rations = 5.0;
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Refiner,
            delta: 1,
        },
    )
    .unwrap();
    s.advance(&w, 1);
    // A power-drawing job ran -> PowerBudgetVisible; it produced Fuel/Data? Refiner makes Fuel.
    assert!(s.flags.contains(&Flag::PowerBudgetVisible));

    // Reaching Data unlocks Tier2 (simulate Data arriving).
    s.resources.add(ResourceKind::Data, 1.0);
    s.advance(&w, 1);
    assert!(s.flags.contains(&Flag::Tier2Unlocked));

    // The UI dashboard outer-gate condition (any panel flag) is satisfiable.
    assert!(
        s.flags.contains(&Flag::BuildMenuUnlocked)
            || s.flags.contains(&Flag::EconomyUnlocked)
            || s.flags.contains(&Flag::PowerBudgetVisible)
            || s.flags.contains(&Flag::SalvageUnlocked)
    );
}

#[test]
fn unfuelled_core_drains_to_failing_then_cold_with_log() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let mut flavor = std::collections::BTreeMap::new();
    flavor.insert("failing".to_string(), "FAILING".to_string());
    flavor.insert("cold".to_string(), "COLD".to_string());
    let origin = OriginDef {
        flavor,
        start: crate::origin::OriginStart {
            scrap: 0.0,
            reactor_capacity: 100.0,
            reactor_charge: 100.0,
        },
        ..default_origin_def(&t)
    };
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new_with_origin(&origin, &w, 1);
    // No fuel, no jobs: the baseline reactor load drains charge to Cold.
    s.advance(&w, 130);
    let texts: Vec<_> = s.log.entries().iter().map(|e| e.text.clone()).collect();
    assert!(texts.iter().any(|x| x == "FAILING"));
    assert!(texts.iter().any(|x| x == "COLD"));
}

#[test]
fn research_completes_and_applies_grant() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.apply(&w, Action::Build(ModuleId::Lab))
        .expect_err("no scrap"); // (built later)
    // Give scrap to build Lab, and data to research.
    s.resources.add(ResourceKind::Scrap, 50.0);
    s.apply(&w, Action::Build(ModuleId::Lab)).unwrap();
    s.resources.add(ResourceKind::Data, 10.0);
    let before_out = s.reactor.max_output;
    s.apply(&w, Action::StartResearch(TechId::CoreTuning))
        .unwrap();
    assert!(s.research.active.is_some());
    // research_ticks = 3 in the test registry.
    s.advance(&w, 3);
    assert!(s.research.is_completed(TechId::CoreTuning));
    assert_eq!(s.reactor.max_output, before_out + 2.0);
    assert!(
        s.codex
            .has(crate::codex::DiscoveryId::Tech(TechId::CoreTuning))
    );
    assert!(s.flags.contains(&Flag::CodexUnlocked));
}

#[test]
fn research_one_slot_and_prereq_and_data() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.resources.add(ResourceKind::Scrap, 50.0);
    s.apply(&w, Action::Build(ModuleId::Lab)).unwrap();
    // No data yet.
    assert_eq!(
        s.apply(&w, Action::StartResearch(TechId::CoreTuning)),
        Err(Rejected("not enough data"))
    );
    s.resources.add(ResourceKind::Data, 100.0);
    s.apply(&w, Action::StartResearch(TechId::CoreTuning))
        .unwrap();
    // One slot.
    assert_eq!(
        s.apply(&w, Action::StartResearch(TechId::RefinedProcesses)),
        Err(Rejected("research already in progress"))
    );
}

#[test]
fn job_output_mult_grant_boosts_economy() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.resources.add(ResourceKind::Scrap, 50.0);
    s.apply(&w, Action::Build(ModuleId::Lab)).unwrap();
    s.resources.add(ResourceKind::Data, 100.0);
    s.apply(&w, Action::StartResearch(TechId::RefinedProcesses))
        .unwrap();
    s.advance(&w, 2); // completes; Salvager mult x2.0 in test registry
    assert_eq!(
        s.research_output_mult.get(&JobId::Salvager).copied(),
        Some(2.0)
    );
    s.crew.total = 1;
    s.resources.add(ResourceKind::Rations, 10.0); // prevent starving efficiency
    s.apply(
        &w,
        Action::AssignCrew {
            job: JobId::Salvager,
            delta: 1,
        },
    )
    .unwrap();
    let before = s.resources.amount(ResourceKind::Scrap);
    s.advance(&w, 1);
    // 1 salvager x1 base x mult 2.0 = +2 scrap this tick.
    assert!((s.resources.amount(ResourceKind::Scrap) - before - 2.0).abs() < 1e-9);
}

/// Regression guard: first crew loss lands exactly `crew_loss_interval` ticks
/// after the atmosphere grace clock hits 0 — not interval+1.
///
/// Timing walkthrough (grace_max=G, interval=I):
///   Ticks 1..G   : grace counts down from G to 1.
///   Tick  G      : grace hits 0 → crew_loss_timer = I, warning log fires.
///   Ticks G+1..  : else branch; each tick decrements timer.
///   Tick  G+I    : timer decrements to 0 → lose_one() fires. ✓ exactly I ticks after G.
#[test]
fn atmosphere_crew_loss_first_fires_at_exactly_interval_ticks_after_grace_expires() {
    use crate::module::{Module, ModuleUnlocks};

    let t = Tuning {
        // Use small values so the test runs in few ticks.
        atmosphere_grace_max: 3,
        crew_loss_interval: 5,
        // Ensure no fuel / charge so the reactor stays Cold immediately.
        reactor_capacity: 100.0,
        reactor_baseline_draw: 0.0, // prevent incidental drain
        reactor_drain_per_tick: 0.0,
        ..Tuning::default()
    };

    let j = jobs();
    // Build a module registry that includes Atmosphere (zero cost for direct injection).
    let m = ModuleRegistry(vec![Module {
        id: ModuleId::Atmosphere,
        name: "Atmosphere".into(),
        description: "test".into(),
        tier: 1,
        cost: vec![],
        cost_scaling: 1.0,
        repeatable: false,
        requires_modules: vec![],
        requires_flag: None,
        unlocks: ModuleUnlocks::default(),
    }]);
    let tc = techs();
    let w = world(&t, &j, &m, &tc);

    // Start with a Cold reactor (zero charge).
    let origin = OriginDef {
        start: crate::origin::OriginStart {
            scrap: 0.0,
            reactor_capacity: 100.0,
            reactor_charge: 0.0,
        },
        ..default_origin_def(&t)
    };
    let mut s = GameState::new_with_origin(&origin, &w, 1);
    assert_eq!(s.reactor.status, ReactorStatus::Cold);

    // Inject crew and Atmosphere module directly (no scrap cost needed).
    s.crew.total = 3;
    *s.built.entry(ModuleId::Atmosphere).or_insert(0) += 1;
    // Grace starts at atmosphere_grace_max.
    assert_eq!(s.atmosphere_grace, t.atmosphere_grace_max);

    let crew_before = s.crew.total;

    // Advance through the grace period. At tick grace_max, grace hits 0 and
    // crew_loss_timer is set to crew_loss_interval; no loss yet.
    s.advance(&w, t.atmosphere_grace_max as u64);
    assert_eq!(s.atmosphere_grace, 0, "grace should be exhausted");
    assert_eq!(s.crew.total, crew_before, "no crew lost during grace");

    // Advance (interval - 1) more ticks: timer counting down, no loss yet.
    s.advance(&w, (t.crew_loss_interval - 1) as u64);
    assert_eq!(
        s.crew.total, crew_before,
        "no crew lost before the interval is fully elapsed"
    );

    // One more tick: timer hits 0, first crew lost.
    s.advance(&w, 1);
    assert_eq!(
        s.crew.total,
        crew_before - 1,
        "exactly one crew lost at crew_loss_interval ticks after grace expired"
    );
}

#[test]
fn atmosphere_failure_drains_clock_then_kills_crew() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    // Build Atmosphere, give crew, then force a Cold reactor (no ignite) so
    // atmosphere is failing every tick.
    let mut s = GameState::new(&w, 1);
    s.resources.add(ResourceKind::Scrap, 100.0);
    s.apply(&w, Action::Build(ModuleId::Atmosphere)).unwrap();
    s.crew.total = 2;
    // Drain the core to Cold: with no fuel the baseline load empties it.
    s.advance(&w, 200);
    assert_eq!(s.reactor.status, ReactorStatus::Cold);
    // After grace + at least one loss interval, crew should have died.
    assert!(
        s.crew.total < 2,
        "atmosphere failure should kill crew when Cold"
    );
    assert!(s.codex.has(crate::codex::DiscoveryId::Note(
        crate::codex::NoteId::FirstCrewDeath
    )));
}

#[test]
fn atmosphere_clock_resets_when_powered() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1); // default origin: established, powered
    s.resources.add(ResourceKind::Scrap, 100.0);
    s.apply(&w, Action::Build(ModuleId::Atmosphere)).unwrap();
    s.crew.total = 2;
    s.advance(&w, 5); // powered (charge high, fuel? baseline only) -> grace stays max
    assert_eq!(s.atmosphere_grace, t.atmosphere_grace_max);
    assert_eq!(s.crew.total, 2);
}

#[test]
fn thermal_failure_accelerates_drain() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    // Two identical games; one builds Thermal (which has no fuel to stay
    // powered, so it fails and drains faster).
    let mut with = GameState::new(&w, 1);
    let mut without = GameState::new(&w, 1);
    with.resources.add(ResourceKind::Scrap, 100.0);
    with.apply(&w, Action::Build(ModuleId::Thermal)).unwrap();
    with.advance(&w, 5);
    without.advance(&w, 5);
    assert!(
        with.reactor.charge < without.reactor.charge,
        "thermal failure should drain charge faster"
    );
}

#[test]
fn upgrade_drive_needs_navigation_and_isotopes() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.resources.add(ResourceKind::Isotopes, 100.0);
    // Navigation not researched yet.
    assert_eq!(
        s.apply(&w, Action::UpgradeDrive),
        Err(Rejected("navigation is not yet understood"))
    );
    s.flags.insert(Flag::NavigationKnown);
    let before = s.ship.drive_range;
    assert!(s.apply(&w, Action::UpgradeDrive).is_ok());
    assert_eq!(s.ship.drive_range, before + t.drive_upgrade_step);
    assert_eq!(
        s.resources.amount(ResourceKind::Isotopes),
        100.0 - t.drive_upgrade_isotopes
    );
}

#[test]
fn scan_needs_sensor_costs_data_and_reveals_contents() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 4);
    let target = a_reachable_target(&s, &w);
    s.resources.add(ResourceKind::Data, 100.0);
    // No sensor array yet.
    assert_eq!(
        s.apply(&w, Action::ScanSystem(target)),
        Err(Rejected("no sensor array to scan with"))
    );
    s.flags.insert(Flag::JumpTheoryKnown);
    s.resources.add(ResourceKind::Alloy, 100.0);
    s.apply(&w, Action::Build(ModuleId::SensorArray)).unwrap();
    let data_before = s.resources.amount(ResourceKind::Data);
    s.apply(&w, Action::ScanSystem(target)).unwrap();
    assert!((data_before - s.resources.amount(ResourceKind::Data) - t.scan_data_cost).abs() < 1e-9);
    s.advance(&w, t.scan_ticks as u64);
    assert!(s.galaxy_state.scanned.contains(&target.0));
    assert!(s.galaxy_state.sites.contains_key(&target.0));
    assert!(s.expedition.activity.is_none());
}

#[test]
fn resolve_site_fills_cargo_and_deposits_on_return() {
    // Disable contact rolls so no ship combat fires and emergency_retreat
    // cannot send the ship home before this test can assert the location.
    let t = Tuning {
        contact_chance: 0.0,
        ..Tuning::default()
    };
    let (j, m, tc) = (jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 4);
    s.resources.add(ResourceKind::Fuel, 200.0);
    let target = a_reachable_target(&s, &w);
    s.apply(&w, Action::Jump(target)).unwrap();
    // Arrive.
    s.advance(&w, 64);
    assert_eq!(s.expedition.location, target);
    // Clear any contact that opened on arrival — this test covers site-resolution,
    // not contact blocking. A real player would resolve the contact first.
    s.expedition.contact = None;
    let sites = s
        .galaxy_state
        .sites
        .get(&target.0)
        .cloned()
        .unwrap_or_default();
    if sites.is_empty() {
        return; // seed 4's neighbor is empty; covered by the deterministic seed-search test below
    }
    s.apply(&w, Action::ResolveSite(target, 0)).unwrap();
    s.advance(&w, t.site_resolve_ticks as u64);
    assert!(s.expedition.cargo_total() > 0.0);
    assert!(s.galaxy_state.sites.get(&target.0).unwrap()[0].depleted);
    // Jump home; on arrival cargo deposits into station resources.
    let iso_home_before = s.resources.amount(ResourceKind::Isotopes);
    s.apply(&w, Action::Jump(crate::galaxy::HOME)).unwrap();
    s.advance(&w, 64);
    assert_eq!(s.expedition.location, crate::galaxy::HOME);
    assert!(s.expedition.cargo.is_empty());
    // Something was deposited (Isotopes or Components, depending on site kind).
    let deposited = s.resources.amount(ResourceKind::Isotopes) > iso_home_before
        || s.resources.amount(ResourceKind::Components) > 0.0;
    assert!(deposited);
}

#[test]
fn resolving_an_asteroid_field_yields_isotopes() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    // Inject a known asteroid site at home (test-only) and resolve it in place.
    s.galaxy_state.sites.insert(
        0,
        vec![crate::galaxy::Site::new(
            crate::galaxy::SiteKind::AsteroidField,
        )],
    );
    s.apply(&w, Action::ResolveSite(crate::galaxy::HOME, 0))
        .unwrap();
    s.advance(&w, t.site_resolve_ticks as u64);
    assert!(
        s.expedition
            .cargo
            .get(&ResourceKind::Isotopes)
            .copied()
            .unwrap_or(0.0)
            > 0.0
    );
}

#[test]
fn resolving_a_derelict_records_the_uneasy_note() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.galaxy_state.sites.insert(
        0,
        vec![crate::galaxy::Site::new(crate::galaxy::SiteKind::Derelict)],
    );
    s.apply(&w, Action::ResolveSite(crate::galaxy::HOME, 0))
        .unwrap();
    s.advance(&w, t.site_resolve_ticks as u64);
    assert!(s.codex.has(crate::codex::DiscoveryId::Note(
        crate::codex::NoteId::FirstDerelict
    )));
}

#[test]
fn shipyard_is_flag_gated_and_builds_the_ship() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.resources.add(ResourceKind::Alloy, 100.0);
    // Without JumpTheoryKnown the build is refused.
    assert_eq!(
        s.apply(&w, Action::Build(ModuleId::Shipyard)),
        Err(Rejected("jump theory is not yet understood"))
    );
    s.flags.insert(Flag::JumpTheoryKnown);
    assert!(s.apply(&w, Action::Build(ModuleId::Shipyard)).is_ok());
    assert!(s.ship.built);
    assert!(s.flags.contains(&Flag::ShipyardComplete));
}

#[test]
fn station_management_locks_while_away() {
    // Disable contact rolls so no ship combat can emergency_retreat the
    // ship home before this test can assert it is still away.
    let t = Tuning {
        contact_chance: 0.0,
        ..Tuning::default()
    };
    let (j, m, tc) = (jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 4);
    s.resources.add(ResourceKind::Fuel, 200.0);
    let target = a_reachable_target(&s, &w);
    s.apply(&w, Action::Jump(target)).unwrap();
    s.advance(&w, 64); // arrive — now away from home
    assert!(s.away());
    // Station actions are refused while away.
    s.resources.add(ResourceKind::Scrap, 100.0);
    assert_eq!(
        s.apply(&w, Action::Salvage),
        Err(Rejected("you are away from the station"))
    );
    assert_eq!(
        s.apply(&w, Action::Build(ModuleId::Refinery)),
        Err(Rejected("you are away from the station"))
    );
    // Jumping home is allowed while away.
    assert!(s.apply(&w, Action::Jump(crate::galaxy::HOME)).is_ok());
}

#[test]
fn scavenge_only_when_stranded_and_refuels_home() {
    // Disable contact rolls so no ship combat can emergency_retreat the
    // ship home before this test strands and scavenges.
    let t = Tuning {
        contact_chance: 0.0,
        ..Tuning::default()
    };
    let (j, m, tc) = (jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 4);
    s.resources.add(ResourceKind::Fuel, 200.0);
    let target = a_reachable_target(&s, &w);
    s.apply(&w, Action::Jump(target)).unwrap();
    s.advance(&w, 64); // arrive
    // Clear any contact that opened on arrival — this test covers scavenge
    // behaviour, not contact blocking. A real player would resolve it first.
    s.expedition.contact = None;
    // Drain fuel to strand the ship.
    s.resources
        .try_spend(ResourceKind::Fuel, s.resources.amount(ResourceKind::Fuel));
    // At home, scavenge is not offered (you're not stranded there). Test the
    // away+stranded path: scavenge is accepted and yields fuel.
    let fuel_before = s.resources.amount(ResourceKind::Fuel);
    s.apply(&w, Action::Scavenge).unwrap();
    s.advance(&w, t.scavenge_ticks as u64);
    assert!((s.resources.amount(ResourceKind::Fuel) - fuel_before - t.scavenge_fuel).abs() < 1e-9);
}

#[test]
fn scavenge_refused_when_not_stranded() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 4);
    s.resources.add(ResourceKind::Fuel, 200.0);
    let target = a_reachable_target(&s, &w);
    s.apply(&w, Action::Jump(target)).unwrap();
    s.advance(&w, 64); // arrive, still fuel-rich
    // Clear any contact that opened on arrival so we test the "not stranded"
    // rejection, not the contact-blocking rejection.
    s.expedition.contact = None;
    assert_eq!(
        s.apply(&w, Action::Scavenge),
        Err(Rejected("no need to scavenge"))
    );
}

impl GameState {
    #[cfg(test)]
    fn surface_for_test(&self, world: &World) -> crate::landing::SurfaceSite {
        let l = self.expedition.landing.as_ref().unwrap();
        self.surface_site(world, l.system, l.site_idx, l.biome)
    }

    /// Scan the regenerated surface for the first Life node (any depth) and
    /// return its id, or None if no Life node exists on this surface.
    #[cfg(test)]
    pub fn find_life_node_for_test(&self, world: &World) -> Option<u32> {
        let l = self.expedition.landing.as_ref()?;
        let site = self.surface_site(world, l.system, l.site_idx, l.biome);
        site.nodes
            .iter()
            .find(|n| n.kind == crate::landing::EncounterKind::Life)
            .map(|n| n.id)
    }

    /// Teleport the current landing to `node` and advance 1 tick so
    /// `resolve_node` fires, leaving `pending_choice` set for a Life node.
    #[cfg(test)]
    pub fn walk_to_for_test(&mut self, world: &World, node: u32) {
        if let Some(l) = self.expedition.landing.as_mut() {
            l.node = node;
            l.traverse = 1;
        }
        self.advance(world, 1);
    }
}

/// Inject a Planet site at the current location and return its index.
fn plant_planet(s: &mut GameState, biome: crate::galaxy::Biome) -> usize {
    let loc = s.expedition.location.0;
    let v = s.galaxy_state.sites.entry(loc).or_default();
    v.push(crate::galaxy::Site::new(crate::galaxy::SiteKind::Planet(
        biome,
    )));
    v.len() - 1
}

#[test]
fn fatal_hazard_forces_retreat_same_tick_no_extract() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    // Find a seed whose Toxic surface has a Hazard node directly off the entry.
    for seed in 1..80u64 {
        let mut s = GameState::new(&w, seed);
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(&mut s, crate::galaxy::Biome::Toxic);
        s.apply(&w, Action::Land(idx)).unwrap();
        let site = s.surface_for_test(&w);
        let hazard =
            site.nodes[0].edges.iter().copied().find(|&e| {
                site.get(e).map(|n| n.kind) == Some(crate::landing::EncounterKind::Hazard)
            });
        let Some(hz) = hazard else { continue };
        // Suit survives the ambient drain to arrival, then the hazard hit (>=18) blows it.
        s.expedition.landing.as_mut().unwrap().suit = 10.0;
        s.apply(&w, Action::MoveToNode(hz)).unwrap();
        s.advance(&w, t.traverse_ticks as u64); // arrive + resolve hazard this tick
        assert!(
            !s.landed(),
            "a fatal hazard must force retreat the same tick (seed {seed})"
        );
        assert!(
            s.expedition.cargo.is_empty(),
            "no haul banked on a blown suit"
        );
        return;
    }
    panic!("no Toxic seed with a hazard adjacent to the entry in 1..80");
}

#[test]
fn advance_n_equals_n_times_advance_1_during_a_landing() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let setup = |s: &mut GameState| {
        s.ship.built = true;
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(s, crate::galaxy::Biome::Verdant);
        s.apply(&w, Action::Land(idx)).unwrap();
        // Begin a traverse so a move + resolve happens during advance.
        let site = s.surface_for_test(&w);
        let target = site.nodes[0].edges[0];
        s.apply(&w, Action::MoveToNode(target)).unwrap();
    };
    let mut bulk = GameState::new(&w, 1);
    setup(&mut bulk);
    let mut step = GameState::new(&w, 1);
    setup(&mut step);
    bulk.advance(&w, 20);
    for _ in 0..20 {
        step.advance(&w, 1);
    }
    assert_eq!(bulk, step);
}

#[test]
fn land_then_extract_banks_nothing_when_satchel_empty() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    // Move the player to a non-home system so they're "away".
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Frozen);
    assert!(s.apply(&w, Action::Land(idx)).is_ok());
    assert!(s.landed());
    assert_eq!(s.expedition.landing.as_ref().unwrap().suit, t.suit_max);
    assert!(s.apply(&w, Action::Extract).is_ok());
    assert!(!s.landed());
}

#[test]
fn suit_drains_and_zero_forces_retreat_losing_satchel() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Toxic);
    s.apply(&w, Action::Land(idx)).unwrap();
    // Put something in the satchel to prove it's lost on retreat.
    s.expedition
        .landing
        .as_mut()
        .unwrap()
        .satchel
        .insert(ResourceKind::Data, 9.0);
    let ticks = (t.suit_max / t.suit_drain_per_tick).ceil() as u64 + 1;
    s.advance(&w, ticks);
    assert!(!s.landed(), "suit ran out -> forced retreat");
    assert!(s.expedition.cargo.is_empty(), "satchel lost, not banked");
}

#[test]
fn station_and_jump_actions_locked_while_landed() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
    s.apply(&w, Action::Land(idx)).unwrap();
    assert!(s.apply(&w, Action::Jump(crate::galaxy::HOME)).is_err());
    assert!(
        s.apply(&w, Action::ScanSystem(crate::galaxy::SystemId(2)))
            .is_err()
    );
}

#[test]
fn life_harvest_and_leave_resolve_with_notes() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Verdant);
    s.apply(&w, Action::Land(idx)).unwrap();
    assert!(s.codex.has(crate::codex::DiscoveryId::Note(
        crate::codex::NoteId::FirstLanding
    )));
    // Drive the landing onto a Life node, then harvest.
    // Seed 1 + SystemId(1)/site_idx=0 + Verdant is verified to contain a Life node.
    let node = s
        .find_life_node_for_test(&w)
        .expect("verdant surface (seed 1, system 1, site 0) must contain a life node");
    s.walk_to_for_test(&w, node); // helper: set node + resolve
    assert!(s.expedition.landing.as_ref().unwrap().pending_choice);
    let before = s.expedition.landing.as_ref().unwrap().satchel_total();
    s.apply(&w, Action::HarvestLife).unwrap();
    assert!(s.expedition.landing.as_ref().unwrap().satchel_total() > before);
    assert!(!s.expedition.landing.as_ref().unwrap().pending_choice);
    assert!(s.codex.has(crate::codex::DiscoveryId::Note(
        crate::codex::NoteId::FirstLifeTaken
    )));
}

#[test]
fn move_traverses_then_resolves_cache_into_satchel() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Frozen);
    s.apply(&w, Action::Land(idx)).unwrap();
    // Find a reachable node from entry and move to it.
    let site = s.surface_for_test(&w);
    let target = site.nodes[0].edges[0];
    assert!(s.apply(&w, Action::MoveToNode(target)).is_ok());
    // Non-neighbor rejected.
    assert!(s.apply(&w, Action::MoveToNode(999)).is_err());
    s.advance(&w, t.traverse_ticks as u64);
    assert_eq!(s.expedition.landing.as_ref().unwrap().node, target);
    // Node is now resolved (recorded in overlay).
    let resolved = s.galaxy_state.sites.get(&1).unwrap()[idx]
        .resolved_nodes
        .contains(&target);
    assert!(resolved);
}

#[test]
fn backtrack_to_entry_is_accepted() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Frozen);
    s.apply(&w, Action::Land(idx)).unwrap();
    // Find the first outgoing edge from entry node 0.
    let site = s.surface_for_test(&w);
    let child = site.nodes[0].edges[0];
    // Move forward to child.
    assert!(s.apply(&w, Action::MoveToNode(child)).is_ok());
    // Advance traverse_ticks so the player arrives at child.
    s.advance(&w, t.traverse_ticks as u64);
    assert_eq!(s.expedition.landing.as_ref().unwrap().node, child);
    // Non-neighbor must still be rejected, with the unchanged message.
    assert_eq!(
        s.apply(&w, Action::MoveToNode(999)),
        Err(Rejected("no route to that node"))
    );
    // Move back to entry (node 0) — bidirectional backtrack.
    assert!(s.apply(&w, Action::MoveToNode(0)).is_ok());
    // Advancing arrives back at the entry node: the back-move is real.
    s.advance(&w, t.traverse_ticks as u64);
    assert_eq!(s.expedition.landing.as_ref().unwrap().node, 0);
}

#[test]
fn squad_defeats_a_remnant_and_salvages() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.ship.cargo = 500.0;
    s.crew.total = 5;
    s.apply(&w, Action::SetAwayCrew { delta: 5 }).unwrap();
    s.gear.weapon_tier = 2;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
    s.apply(&w, Action::Land(idx)).unwrap();
    // Force a combat at the current node.
    s.expedition.landing.as_mut().unwrap().combat = Some(crate::combat::remnant_stats(1, 1.0, &t));
    s.expedition.cargo.insert(ResourceKind::Rations, 1000.0);
    s.advance(&w, 60); // a strong squad should win within the suit window
    assert!(
        s.expedition.landing.is_none() || s.expedition.landing.as_ref().unwrap().combat.is_none(),
        "combat resolved"
    );
}

#[test]
fn advance_n_equals_n_times_advance_1_during_combat() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let setup = |s: &mut GameState| {
        s.ship.built = true;
        s.ship.cargo = 500.0;
        s.crew.total = 3;
        s.apply(&w, Action::SetAwayCrew { delta: 3 }).unwrap();
        // weapon_tier 0 (default) gives a weaker squad that will trade blows
        // for several ticks before defeating a depth-1 remnant.
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(s, crate::galaxy::Biome::Ruined);
        s.apply(&w, Action::Land(idx)).unwrap();
        // Inject combat so tick_combat runs from tick 1.
        s.expedition.landing.as_mut().unwrap().combat =
            Some(crate::combat::remnant_stats(1, 1.0, &t));
        // Rations keep the away team fed through the window.
        s.expedition.cargo.insert(ResourceKind::Rations, 1000.0);
    };
    let mut bulk = GameState::new(&w, 1);
    setup(&mut bulk);
    let mut step = GameState::new(&w, 1);
    setup(&mut step);
    // Verify combat is actually active at the start so tick_combat is exercised.
    assert!(
        bulk.expedition.landing.as_ref().unwrap().combat.is_some(),
        "combat must be active at tick 0 for this test to be meaningful"
    );
    bulk.advance(&w, 30);
    for _ in 0..30 {
        step.advance(&w, 1);
    }
    assert_eq!(bulk, step);
}

#[test]
fn armory_crafting_upgrades_gear_and_stocks_consumables() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    // No Armory yet → rejected.
    assert!(s.apply(&w, Action::UpgradeWeapon).is_err());
    s.flags.insert(Flag::CombatDoctrineKnown);
    s.resources.add(ResourceKind::Alloy, 500.0);
    s.resources.add(ResourceKind::Components, 500.0);
    s.apply(&w, Action::Build(ModuleId::Armory)).unwrap();
    assert!(s.apply(&w, Action::UpgradeWeapon).is_ok());
    assert_eq!(s.gear.weapon_tier, 1);
    assert!(
        s.apply(
            &w,
            Action::CraftConsumable(crate::gear::ConsumableKind::MedGel)
        )
        .is_ok()
    );
    assert_eq!(
        s.consumables
            .get(&crate::gear::ConsumableKind::MedGel)
            .copied()
            .unwrap_or(0),
        1
    );
}

#[test]
fn away_team_eats_ship_rations_then_starves() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.crew.total = 3;
    s.apply(&w, Action::SetAwayCrew { delta: 3 }).unwrap();
    s.expedition.location = crate::galaxy::SystemId(1); // away
    s.expedition.cargo.insert(ResourceKind::Rations, 10.0);
    s.advance(&w, 5);
    // Ate 3 * 0.15 * 5 = 2.25 Rations from cargo; no starvation yet.
    assert!(
        (s.expedition
            .cargo
            .get(&ResourceKind::Rations)
            .copied()
            .unwrap()
            - 7.75)
            .abs()
            < 1e-9
    );
    let morale_before = s.expedition.away_team.morale;
    s.expedition.cargo.remove(&ResourceKind::Rations); // out of food
    s.advance(&w, 3);
    assert!(
        s.expedition.away_team.morale < morale_before,
        "starvation hits morale"
    );
}

#[test]
fn set_away_crew_commits_idle_and_excludes_from_economy() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.crew.total = 4;
    assert_eq!(s.crew.idle(), 4);
    assert!(s.apply(&w, Action::SetAwayCrew { delta: 2 }).is_ok());
    assert_eq!(s.expedition.away_team.crew, 2);
    assert_eq!(s.crew.away, 2);
    assert_eq!(s.crew.idle(), 2); // away excluded from idle
    assert!(s.apply(&w, Action::SetAwayCrew { delta: -1 }).is_ok());
    assert_eq!(s.expedition.away_team.crew, 1);
    assert_eq!(s.crew.idle(), 3);
}

/// Task 9: arriving HOME releases the away-team back to idle; a docked squad
/// below max morale recovers morale each tick.
#[test]
fn return_home_releases_away_team_and_morale_recovers_while_docked() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 42);

    // Commit 3 crew to the away-team while docked.
    s.crew.total = 4;
    s.apply(&w, Action::SetAwayCrew { delta: 3 }).unwrap();
    assert_eq!(s.crew.away, 3);
    assert_eq!(s.expedition.away_team.crew, 3);

    // Simulate being away (ship in transit back to HOME).
    // We skip an actual Jump and manually drive transit completion:
    // put the ship in transit-to-HOME with 1 tick remaining.
    s.expedition.activity = Some(crate::expedition::Activity::Transit {
        to: crate::galaxy::HOME,
        ticks: 1,
    });
    s.expedition.location = crate::galaxy::SystemId(1); // pretend we left home

    // Advance 1 tick: transit completes, ship arrives at HOME.
    s.advance(&w, 1);

    // Ship is now back at HOME, activity cleared, away-team dissolved.
    assert_eq!(s.expedition.location, crate::galaxy::HOME);
    assert!(s.expedition.activity.is_none());
    assert_eq!(
        s.crew.away, 0,
        "away crew released back to idle on return home"
    );
    assert_eq!(
        s.expedition.away_team.crew, 0,
        "AwayTeam dissolved after return home"
    );
    // The 3 survivors rejoin idle (total unchanged since nobody died).
    assert_eq!(s.crew.idle(), s.crew.total, "all crew are idle again");

    // — Morale recovery while docked with a committed squad —
    // Re-commit 2 crew with low morale while docked.
    s.apply(&w, Action::SetAwayCrew { delta: 2 }).unwrap();
    // Artificially lower morale below max.
    let low_morale = t.morale_max * 0.5;
    s.expedition.away_team.morale = low_morale;

    // Advance a few ticks while still docked (NOT away).
    assert!(!s.away(), "must be docked for morale recovery");
    let ticks = 10u64;
    s.advance(&w, ticks);

    let expected_morale = (low_morale + t.morale_recover_per_tick * ticks as f64).min(t.morale_max);
    assert!(
        (s.expedition.away_team.morale - expected_morale).abs() < 1e-9,
        "morale recovered while docked: got {}, expected {}",
        s.expedition.away_team.morale,
        expected_morale
    );
}

#[test]
fn winning_a_peer_fight_tallies_and_notes() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.ship.cargo = 500.0;
    // weapon_tier=0 so the peer mirror stays at 1× (depth-1 peer stays beatable by
    // a full 6-crew squad before morale routs). gear is passed to peer_stats so the
    // two scale together — a weapon_tier=2 peer is 2× stronger and outpaces morale.
    s.crew.total = 6;
    s.gear.weapon_tier = 0;
    // SetAwayCrew must happen while docked (before moving away).
    s.apply(&w, Action::SetAwayCrew { delta: 6 }).unwrap();
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
    s.apply(&w, Action::Land(idx)).unwrap();
    s.expedition.landing.as_mut().unwrap().combat =
        Some(crate::alien::peer_stats(1, 1.0, s.gear, &t));
    s.expedition.cargo.insert(ResourceKind::Rations, 1000.0);
    s.advance(&w, 200);
    assert_eq!(s.peers_killed, 1);
    assert!(s.codex.has(crate::codex::DiscoveryId::Note(
        crate::codex::NoteId::FirstPeerKilled
    )));
}

#[test]
fn parley_de_escalates_and_avoid_leaves() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
    s.apply(&w, Action::Land(idx)).unwrap();
    s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
        kind: crate::alien::AlienKind::Peer,
        disposition: 0.0,
    });
    let before = s
        .expedition
        .landing
        .as_ref()
        .unwrap()
        .alien
        .as_ref()
        .unwrap()
        .disposition;
    s.apply(&w, Action::Parley).unwrap();
    assert!(
        s.expedition
            .landing
            .as_ref()
            .unwrap()
            .alien
            .as_ref()
            .unwrap()
            .disposition
            > before
    );
    s.apply(&w, Action::Avoid).unwrap();
    assert!(s.expedition.landing.as_ref().unwrap().alien.is_none());
}

#[test]
fn gift_de_escalates_trade_swaps() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Ruined);
    s.apply(&w, Action::Land(idx)).unwrap();
    // Hostile peer + Components to gift.
    s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
        kind: crate::alien::AlienKind::Peer,
        disposition: -60.0,
    });
    s.expedition.cargo.insert(ResourceKind::Components, 100.0);
    s.apply(&w, Action::Gift).unwrap();
    assert!(
        s.expedition
            .landing
            .as_ref()
            .unwrap()
            .alien
            .as_ref()
            .unwrap()
            .disposition
            > -60.0
    );

    // — Trade leg: Curious peer, verify cargo swap + disposition rise —
    // Set disposition to Curious band (>= disp_curious = 30.0).
    s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
        kind: crate::alien::AlienKind::Peer,
        disposition: 35.0,
    });
    let comp_before = s
        .expedition
        .cargo
        .get(&ResourceKind::Components)
        .copied()
        .unwrap_or(0.0);
    let iso_before = s
        .expedition
        .cargo
        .get(&ResourceKind::Isotopes)
        .copied()
        .unwrap_or(0.0);
    let disp_before = s
        .expedition
        .landing
        .as_ref()
        .unwrap()
        .alien
        .as_ref()
        .unwrap()
        .disposition;
    s.apply(&w, Action::Trade).unwrap();
    let comp_after = s
        .expedition
        .cargo
        .get(&ResourceKind::Components)
        .copied()
        .unwrap_or(0.0);
    let iso_after = s
        .expedition
        .cargo
        .get(&ResourceKind::Isotopes)
        .copied()
        .unwrap_or(0.0);
    let disp_after = s
        .expedition
        .landing
        .as_ref()
        .unwrap()
        .alien
        .as_ref()
        .unwrap()
        .disposition;
    assert!(
        comp_after < comp_before,
        "Components should decrease after Trade"
    );
    assert!(
        iso_after > iso_before,
        "Isotopes should increase after Trade"
    );
    assert!(
        disp_after > disp_before,
        "Disposition should rise after Trade"
    );
    // Curious band: isotope gain = trade_get * CURIOUS_TRADE_BONUS
    let expected_iso_gain = t.trade_get * crate::state::CURIOUS_TRADE_BONUS;
    assert!(
        (iso_after - iso_before - expected_iso_gain).abs() < 1e-9,
        "Curious trade bonus not applied correctly: got {}, expected {}",
        iso_after - iso_before,
        expected_iso_gain
    );
}

#[test]
fn harvest_fauna_and_attack_starts_combat() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.ship.cargo = 500.0;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Verdant);
    s.apply(&w, Action::Land(idx)).unwrap();
    s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
        kind: crate::alien::AlienKind::Fauna,
        disposition: -20.0,
    });
    s.apply(&w, Action::Harvest).unwrap();
    assert!(
        s.expedition
            .landing
            .as_ref()
            .unwrap()
            .satchel
            .values()
            .copied()
            .sum::<f64>()
            > 0.0
    );
    assert!(s.codex.has(crate::codex::DiscoveryId::Note(
        crate::codex::NoteId::FirstFaunaDisplaced
    )));
    assert!(s.expedition.landing.as_ref().unwrap().alien.is_none()); // harvested = resolved

    // — AttackAlien leg: set a fresh Fauna alien, dispatch AttackAlien, assert combat starts —
    s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
        kind: crate::alien::AlienKind::Fauna,
        disposition: -30.0,
    });
    s.apply(&w, Action::AttackAlien).unwrap();
    assert!(
        s.expedition.landing.as_ref().unwrap().combat.is_some(),
        "combat should be set after AttackAlien"
    );
    assert!(
        s.expedition.landing.as_ref().unwrap().alien.is_none(),
        "alien should be cleared after AttackAlien"
    );

    // — AttackAlien leg: Peer alien also converts to combat —
    if let Some(l) = s.expedition.landing.as_mut() {
        l.combat = None; // clear previous combat
    }
    s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
        kind: crate::alien::AlienKind::Peer,
        disposition: -50.0,
    });
    s.apply(&w, Action::AttackAlien).unwrap();
    assert!(
        s.expedition.landing.as_ref().unwrap().combat.is_some(),
        "combat should be set after AttackAlien on a Peer"
    );
    assert!(
        s.expedition.landing.as_ref().unwrap().alien.is_none(),
        "alien should be cleared after AttackAlien on a Peer"
    );
}

#[test]
fn harvest_rejects_non_fauna_and_missing_alien() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.ship.cargo = 500.0;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Verdant);
    s.apply(&w, Action::Land(idx)).unwrap();
    // No alien present → reject
    assert!(s.apply(&w, Action::Harvest).is_err());
    // Peer alien → reject
    s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
        kind: crate::alien::AlienKind::Peer,
        disposition: 0.0,
    });
    assert_eq!(
        s.apply(&w, Action::Harvest),
        Err(Rejected("that is not an animal"))
    );
}

#[test]
fn attack_alien_rejects_when_no_alien() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.ship.cargo = 500.0;
    s.expedition.location = crate::galaxy::SystemId(1);
    let idx = plant_planet(&mut s, crate::galaxy::Biome::Verdant);
    s.apply(&w, Action::Land(idx)).unwrap();
    assert!(s.apply(&w, Action::AttackAlien).is_err());
}

#[test]
fn advance_n_equals_n_times_advance_1_during_alien_negotiation() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let setup = |s: &mut GameState| {
        s.ship.built = true;
        s.ship.cargo = 500.0;
        s.crew.total = 3;
        s.apply(&w, Action::SetAwayCrew { delta: 3 }).unwrap();
        s.expedition.location = crate::galaxy::SystemId(1);
        let idx = plant_planet(s, crate::galaxy::Biome::Ruined);
        s.apply(&w, Action::Land(idx)).unwrap();
        // Inject an open Peer alien at neutral disposition.  No verb is applied so
        // disposition stays static; the Suit drains each tick — that is the live
        // sub-state being advanced deterministically.
        s.expedition.landing.as_mut().unwrap().alien = Some(crate::alien::AlienEncounter {
            kind: crate::alien::AlienKind::Peer,
            disposition: 0.0,
        });
        // Rations keep the away team fed through the window.
        s.expedition.cargo.insert(ResourceKind::Rations, 1000.0);
    };
    let mut bulk = GameState::new(&w, 1);
    setup(&mut bulk);
    let mut step = GameState::new(&w, 1);
    setup(&mut step);
    // Sanity: alien must be present at tick 0 or the test proves nothing.
    assert!(
        bulk.expedition.landing.as_ref().unwrap().alien.is_some(),
        "alien must be active at tick 0 for this test to be meaningful"
    );
    bulk.advance(&w, 20);
    for _ in 0..20 {
        step.advance(&w, 1);
    }
    assert_eq!(bulk, step);
}

// --- CB3 Task 4 tests ---

/// Arriving in a system that has a seeded Peer contact surfaces the contact
/// and blocks ScanSystem while it is open.
/// Pinned seed: world_seed=42 + system=4 → roll_contact returns Some(Peer).
#[test]
fn arrival_surfaces_contact_and_blocks_scan() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    // world_seed=42 so roll_contact(42, 4, ..) → Some(Peer)
    let mut s = GameState::new(&w, 42);
    s.ship.built = true;

    let target = crate::galaxy::SystemId(4);
    // Manually inject transit completion to system 4.
    s.expedition.activity = Some(crate::expedition::Activity::Transit {
        to: target,
        ticks: 1,
    });
    s.advance(&w, 1);

    // Contact must be open after arrival.
    assert!(
        s.expedition.contact.is_some(),
        "expected a contact at system 4 with seed 42"
    );
    assert_eq!(s.expedition.location, target);

    // ScanSystem must be rejected while the contact is open.
    let other = crate::galaxy::SystemId(5);
    let err = s.apply(&w, Action::ScanSystem(other)).unwrap_err();
    assert!(
        err.0.contains("resolve the contact"),
        "expected contact-block message, got: {}",
        err.0
    );
}

/// With XenoContactKnown set, a Peer contact opens negotiation (combat is None).
#[test]
fn peer_contact_with_xeno_known_opens_negotiation() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 42);
    s.ship.built = true;
    s.flags.insert(crate::flags::Flag::XenoContactKnown);

    let target = crate::galaxy::SystemId(4);
    s.expedition.activity = Some(crate::expedition::Activity::Transit {
        to: target,
        ticks: 1,
    });
    s.advance(&w, 1);

    let contact = s.expedition.contact.as_ref().expect("contact must be open");
    assert_eq!(contact.kind, crate::contact::ShipKind::Peer);
    assert!(
        contact.combat.is_none(),
        "XenoContactKnown Peer contact must open as negotiation, not combat"
    );
}

/// Without XenoContactKnown, a Peer contact opens as combat.
#[test]
fn peer_contact_without_xeno_known_opens_combat() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 42);
    s.ship.built = true;
    // XenoContactKnown is NOT set

    let target = crate::galaxy::SystemId(4);
    s.expedition.activity = Some(crate::expedition::Activity::Transit {
        to: target,
        ticks: 1,
    });
    s.advance(&w, 1);

    let contact = s.expedition.contact.as_ref().expect("contact must be open");
    assert_eq!(contact.kind, crate::contact::ShipKind::Peer);
    assert!(
        contact.combat.is_some(),
        "Peer contact without XenoContactKnown must open as combat"
    );
}

/// Ship combat: injecting a Peer contact with low enemy_hp wins within the
/// advance window, salvages to expedition cargo, tallies peers_killed, fires
/// the FirstPeerKilled note, clears the contact, and records resolved_contacts.
#[test]
fn ship_combat_peer_win_tallies_salvage_and_note() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.ship.hull = t.ship_base_hull; // full hull

    let system = crate::galaxy::SystemId(5);
    s.expedition.location = system;

    // Inject a Peer ship combat with very low enemy_hp so we win in 1 tick.
    let mut combat = crate::contact::peer_ship_stats(1, 1.0, &s.ship, &t);
    combat.enemy_hp = 0.1; // trivially low
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: Some(combat),
    });

    s.advance(&w, 5);

    assert_eq!(s.peers_killed, 1, "peer kill tallied");
    assert!(
        s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstPeerKilled
        )),
        "FirstPeerKilled note added"
    );
    assert!(s.expedition.contact.is_none(), "contact cleared after win");
    assert!(
        s.resolved_contacts.contains(&system.0),
        "system recorded in resolved_contacts"
    );
    assert!(
        s.expedition
            .cargo
            .get(&ResourceKind::Components)
            .copied()
            .unwrap_or(0.0)
            > 0.0,
        "Components salvaged"
    );
    assert!(
        s.expedition
            .cargo
            .get(&ResourceKind::Isotopes)
            .copied()
            .unwrap_or(0.0)
            > 0.0,
        "Isotopes salvaged"
    );
    assert!(
        s.expedition
            .cargo
            .get(&ResourceKind::Data)
            .copied()
            .unwrap_or(0.0)
            > 0.0,
        "Data salvaged"
    );
}

/// Resolved systems do not surface a second contact on re-arrival.
#[test]
fn resolved_system_skips_contact_roll() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 42);
    s.ship.built = true;

    let target = crate::galaxy::SystemId(4);
    // Mark system 4 as already resolved.
    s.resolved_contacts.insert(4);

    s.expedition.activity = Some(crate::expedition::Activity::Transit {
        to: target,
        ticks: 1,
    });
    s.advance(&w, 1);

    assert!(
        s.expedition.contact.is_none(),
        "no contact for an already-resolved system"
    );
}

/// Hull drops to zero mid ship-combat → emergency_retreat fires:
/// - location jumps back to prev_location
/// - contact cleared
/// - hull == hull_retreat_floor (clamped, not negative)
/// - cargo reduced by retreat_cargo_loss fraction
/// - FirstShipCrippled note fired
/// - system NOT added to resolved_contacts (a loss ≠ resolved)
/// - game still playable (no panic)
#[test]
fn emergency_retreat_on_hull_zero() {
    // Guarantee the player loses in a single tick: set enemy attack very high
    // and player hull very low. We will set hull directly after setup.
    let t = Tuning {
        retreat_cargo_loss: 0.4,
        hull_retreat_floor: 10.0,
        // Make enemy deal guaranteed damage per tick > player hull.
        remnant_ship_atk: 9999.0,
        remnant_ship_def: 0.0,
        ..Tuning::default()
    };

    let (j, m, tc) = (jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 7);
    s.ship.built = true;

    // Place the ship in a non-home system with a known prev_location.
    let away_system = crate::galaxy::SystemId(3);
    let prev_system = crate::galaxy::SystemId(2);
    s.expedition.location = away_system;
    s.expedition.prev_location = prev_system;

    // Put 100 units of Isotopes in cargo so we can measure the loss.
    *s.expedition
        .cargo
        .entry(ResourceKind::Isotopes)
        .or_insert(0.0) = 100.0;

    // Set hull to a tiny positive value — one tick will drive it to ≤ 0.
    s.ship.hull = 0.01;

    // Inject a Remnant ship combat (enemy still very much alive).
    let combat = crate::contact::remnant_ship_stats(1, 1.0, &t);
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Remnant,
        disposition: 0.0,
        combat: Some(combat),
    });

    // Advance — the first tick's ship combat will drive hull ≤ 0.
    s.advance(&w, 1);

    assert_eq!(
        s.expedition.location, prev_system,
        "ship retreated to prev_location"
    );
    assert!(
        s.expedition.contact.is_none(),
        "contact cleared after retreat"
    );
    assert!(
        (s.ship.hull - t.hull_retreat_floor).abs() < 1e-9,
        "hull clamped to retreat floor (was {}, want {})",
        s.ship.hull,
        t.hull_retreat_floor
    );
    let isotopes = s
        .expedition
        .cargo
        .get(&ResourceKind::Isotopes)
        .copied()
        .unwrap_or(0.0);
    let expected = 100.0 * (1.0 - t.retreat_cargo_loss);
    assert!(
        (isotopes - expected).abs() < 1e-6,
        "cargo reduced by retreat_cargo_loss: got {isotopes}, want {expected}"
    );
    assert!(
        s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstShipCrippled
        )),
        "FirstShipCrippled note fired"
    );
    assert!(
        !s.resolved_contacts.contains(&away_system.0),
        "a loss does NOT mark the system resolved"
    );
    // Game still playable: advance a few more ticks without panic.
    s.advance(&w, 5);
}

// ── CB3 Task 7: ship-scoped negotiation verbs ──────────────────────────────

/// Helper: inject a Peer ship contact with no combat (negotiation mode).
fn peer_negotiation_contact(disposition: f64) -> crate::contact::ShipContact {
    crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition,
        combat: None,
    }
}

/// HailParley on a Peer near peace_band: shifts disposition, crosses peace_band
/// → clears contact, inserts resolved_contacts, increments peers_spared,
/// fires FirstPeerSpared note, grants observe_data Data to cargo.
#[test]
fn hail_parley_peer_reaches_curious_clears_and_tallies() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;

    let system = crate::galaxy::SystemId(7);
    s.expedition.location = system;
    // peace_band = 30.0, parley_shift = 18.0 — start at 15.0 so 15+18=33 >= 30
    s.expedition.contact = Some(peer_negotiation_contact(15.0));

    s.apply(&w, Action::HailParley).unwrap();

    assert!(
        s.expedition.contact.is_none(),
        "contact cleared after reaching Curious"
    );
    assert!(
        s.resolved_contacts.contains(&system.0),
        "system recorded in resolved_contacts"
    );
    assert_eq!(s.peers_spared, 1, "peers_spared incremented");
    assert!(
        s.codex.has(crate::codex::DiscoveryId::Note(
            crate::codex::NoteId::FirstPeerSpared
        )),
        "FirstPeerSpared note fired"
    );
    let data = s
        .expedition
        .cargo
        .get(&ResourceKind::Data)
        .copied()
        .unwrap_or(0.0);
    assert!(data > 0.0, "observe_data granted to cargo (got {})", data);
}

/// HailGift de-escalates a Hostile Peer: spends Components, shifts disposition.
#[test]
fn hail_gift_de_escalates_hostile_peer() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;

    let system = crate::galaxy::SystemId(8);
    s.expedition.location = system;
    // Hostile disposition (disp_hostile <= -50 by default)
    s.expedition.contact = Some(peer_negotiation_contact(-60.0));
    s.expedition.cargo.insert(ResourceKind::Components, 100.0);

    let comp_before = 100.0f64;
    s.apply(&w, Action::HailGift).unwrap();

    let comp_after = s
        .expedition
        .cargo
        .get(&ResourceKind::Components)
        .copied()
        .unwrap_or(0.0);
    assert!(
        comp_after < comp_before,
        "Components spent: before={comp_before} after={comp_after}"
    );
    let disp_after = s.expedition.contact.as_ref().unwrap().disposition;
    assert!(
        disp_after > -60.0,
        "disposition improved after Gift: {disp_after}"
    );
    // Contact stays open (no resolve on Gift)
    assert!(
        s.expedition.contact.is_some(),
        "contact remains open after Gift"
    );
}

/// HailAvoid on a non-Hostile Peer: clears contact + resolved_contacts, no peers_spared.
#[test]
fn hail_avoid_non_hostile_clears_no_tally() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;

    let system = crate::galaxy::SystemId(9);
    s.expedition.location = system;
    // Neutral disposition
    s.expedition.contact = Some(peer_negotiation_contact(0.0));

    s.apply(&w, Action::HailAvoid).unwrap();

    assert!(
        s.expedition.contact.is_none(),
        "contact cleared after HailAvoid"
    );
    assert!(
        s.resolved_contacts.contains(&system.0),
        "system recorded in resolved_contacts"
    );
    assert_eq!(s.peers_spared, 0, "HailAvoid does NOT tally peers_spared");
}

/// HailAvoid on a Hostile Peer is rejected — "they will not let you pass".
#[test]
fn hail_avoid_hostile_rejected() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;

    let system = crate::galaxy::SystemId(10);
    s.expedition.location = system;
    // disp_hostile = -50.0 by default; -80 is firmly Hostile
    s.expedition.contact = Some(peer_negotiation_contact(-80.0));

    let err = s.apply(&w, Action::HailAvoid).unwrap_err();
    assert!(
        err.0.contains("will not let you pass"),
        "expected rejection, got: {}",
        err.0
    );
    assert!(
        s.expedition.contact.is_some(),
        "contact must remain open after rejection"
    );
}

// ── CB3 Task 8: ship combat inputs ────────────────────────────────────────

/// AttackShip on a Peer negotiation Contact opens ship combat with sane stats.
/// Crucially: enemy_hp must be < 500 (depth=0 scaling only, not distance).
#[test]
fn attack_ship_peer_starts_combat_sane_stats() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;

    let system = crate::galaxy::SystemId(11);
    s.expedition.location = system;
    // Peer in negotiation mode (no combat yet).
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: None,
    });

    s.apply(&w, Action::AttackShip).unwrap();

    let combat = s
        .expedition
        .contact
        .as_ref()
        .expect("contact still present")
        .combat
        .as_ref()
        .expect("combat now set");
    assert!(
        combat.enemy_hp > 0.0,
        "enemy_hp must be positive (got {})",
        combat.enemy_hp
    );
    // Depth=0 scaling: enemy_hp = peer_ship_hp * grad * mirror.
    // At system 11 from HOME the grad is small; with depth distance-as-depth
    // the value would be enormous. Cap at 500 as the regression guard.
    assert!(
        combat.enemy_hp < 500.0,
        "enemy_hp < 500 (depth=0 guard, got {})",
        combat.enemy_hp
    );
}

/// AttackShip on a Remnant contact opens combat.
#[test]
fn attack_ship_remnant_starts_combat() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;

    s.expedition.location = crate::galaxy::SystemId(12);
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Remnant,
        disposition: 0.0,
        combat: None,
    });

    s.apply(&w, Action::AttackShip).unwrap();
    assert!(
        s.expedition.contact.as_ref().unwrap().combat.is_some(),
        "combat opened for Remnant"
    );
}

/// AttackShip is rejected when there is no contact.
#[test]
fn attack_ship_rejected_no_contact() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let err = s.apply(&w, Action::AttackShip).unwrap_err();
    assert!(err.0.contains("no contact"), "got: {}", err.0);
}

/// AttackShip is rejected when combat is already ongoing.
#[test]
fn attack_ship_rejected_already_fighting() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(13);
    let grad = 1.0;
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
    });
    let err = s.apply(&w, Action::AttackShip).unwrap_err();
    assert!(err.0.contains("already"), "got: {}", err.0);
}

/// SetShipStance flips the combat stance.
#[test]
fn set_ship_stance_flips_stance() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(14);
    let grad = 1.0;
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
    });
    // Default stance is Aggressive; switch to Defensive.
    s.apply(&w, Action::SetShipStance(crate::combat::Stance::Defensive))
        .unwrap();
    let stance = s
        .expedition
        .contact
        .as_ref()
        .unwrap()
        .combat
        .as_ref()
        .unwrap()
        .stance;
    assert_eq!(
        stance,
        crate::combat::Stance::Defensive,
        "stance is now Defensive"
    );

    // Switch back.
    s.apply(&w, Action::SetShipStance(crate::combat::Stance::Aggressive))
        .unwrap();
    let stance2 = s
        .expedition
        .contact
        .as_ref()
        .unwrap()
        .combat
        .as_ref()
        .unwrap()
        .stance;
    assert_eq!(
        stance2,
        crate::combat::Stance::Aggressive,
        "stance toggled back"
    );
}

/// SetShipStance rejected when no active ship combat.
#[test]
fn set_ship_stance_rejected_no_combat() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let err = s
        .apply(&w, Action::SetShipStance(crate::combat::Stance::Defensive))
        .unwrap_err();
    assert!(err.0.contains("no active ship combat"), "got: {}", err.0);
}

/// UseRepairDrone heals hull by repair_drone_heal (clamped to hull_max)
/// and decrements repair_drones.
#[test]
fn use_repair_drone_heals_and_decrements() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(15);
    let grad = 1.0;
    // Damage the hull first.
    let hull_max = s.ship.hull_max(&t);
    s.ship.hull = hull_max - t.repair_drone_heal - 5.0;
    s.ship.repair_drones = 1;
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
    });
    let hull_before = s.ship.hull;
    s.apply(&w, Action::UseRepairDrone).unwrap();

    let expected = (hull_before + t.repair_drone_heal).min(hull_max);
    assert!(
        (s.ship.hull - expected).abs() < 1e-9,
        "hull healed: got {}, want {}",
        s.ship.hull,
        expected
    );
    assert_eq!(s.ship.repair_drones, 0, "drone count decremented");
}

/// UseRepairDrone clamps heal at hull_max.
#[test]
fn use_repair_drone_clamps_at_hull_max() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(16);
    let grad = 1.0;
    let hull_max = s.ship.hull_max(&t);
    // Hull already near full.
    s.ship.hull = hull_max - 1.0;
    s.ship.repair_drones = 1;
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
    });
    s.apply(&w, Action::UseRepairDrone).unwrap();
    assert!(
        (s.ship.hull - hull_max).abs() < 1e-9,
        "hull clamped to hull_max: got {}, want {}",
        s.ship.hull,
        hull_max
    );
}

/// UseRepairDrone rejected when no active combat.
#[test]
fn use_repair_drone_rejected_no_combat() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.repair_drones = 3;
    let err = s.apply(&w, Action::UseRepairDrone).unwrap_err();
    assert!(err.0.contains("no active ship combat"), "got: {}", err.0);
}

/// UseRepairDrone rejected when out of stock.
#[test]
fn use_repair_drone_rejected_out_of_stock() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(17);
    let grad = 1.0;
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: Some(crate::contact::peer_ship_stats(0, grad, &s.ship, &t)),
    });
    // repair_drones = 0 (default)
    let err = s.apply(&w, Action::UseRepairDrone).unwrap_err();
    assert!(err.0.contains("no repair drones"), "got: {}", err.0);
}

/// ShipFlee clears contact, moves to prev_location, and spends Fuel.
#[test]
fn ship_flee_clears_contact_and_spends_fuel() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;

    let away_system = crate::galaxy::SystemId(5);
    let prev_system = crate::galaxy::HOME;
    s.expedition.location = away_system;
    s.expedition.prev_location = prev_system;
    // Give enough fuel to flee.
    s.resources.fuel = 9999.0;
    let fuel_before = s.resources.fuel;
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: None,
    });

    s.apply(&w, Action::ShipFlee).unwrap();

    assert!(
        s.expedition.contact.is_none(),
        "contact cleared after ShipFlee"
    );
    assert_eq!(
        s.expedition.location, prev_system,
        "location is now prev_location"
    );
    // NOT in resolved_contacts (we fled, not resolved).
    assert!(
        !s.resolved_contacts.contains(&away_system.0),
        "ShipFlee must NOT mark system resolved"
    );
    let fuel_after = s.resources.fuel;
    assert!(
        fuel_after < fuel_before,
        "fuel spent fleeing: before={fuel_before} after={fuel_after}"
    );
}

/// ShipFlee is rejected when Fuel is insufficient.
#[test]
fn ship_flee_rejected_no_fuel() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.ship.built = true;
    s.expedition.location = crate::galaxy::SystemId(6);
    s.expedition.prev_location = crate::galaxy::HOME;
    // No fuel at all.
    s.resources.fuel = 0.0;
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: None,
    });

    let err = s.apply(&w, Action::ShipFlee).unwrap_err();
    assert!(
        err.0.contains("no fuel"),
        "expected no-fuel rejection, got: {}",
        err.0
    );
    // Contact must remain.
    assert!(
        s.expedition.contact.is_some(),
        "contact remains after failed flee"
    );
}

/// ShipFlee is rejected when no contact is present.
#[test]
fn ship_flee_rejected_no_contact() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let err = s.apply(&w, Action::ShipFlee).unwrap_err();
    assert!(err.0.contains("no contact"), "got: {}", err.0);
}

// ── CB3 Task 9: ship upgrade actions ─────────────────────────────────────

fn shipyard_ready(s: &mut GameState, w: &World) {
    s.flags.insert(Flag::MunitionsKnown);
    s.flags.insert(Flag::JumpTheoryKnown);
    s.resources.add(ResourceKind::Alloy, 500.0);
    s.resources.add(ResourceKind::Components, 500.0);
    s.resources.add(ResourceKind::Scrap, 500.0);
    s.apply(w, Action::Build(ModuleId::Shipyard)).unwrap();
}

#[test]
fn upgrade_ship_weapon_raises_tier_and_spends_resources() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    shipyard_ready(&mut s, &w);
    let alloy_before = s.resources.amount(ResourceKind::Alloy);
    let comp_before = s.resources.amount(ResourceKind::Components);
    assert!(s.apply(&w, Action::UpgradeShipWeapon).is_ok());
    assert_eq!(s.ship.ship_weapon_tier, 1);
    assert!(
        s.resources.amount(ResourceKind::Alloy) < alloy_before,
        "alloy spent"
    );
    assert!(
        s.resources.amount(ResourceKind::Components) < comp_before,
        "components spent"
    );
}

#[test]
fn upgrade_ship_weapon_rejected_at_max_tier() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    shipyard_ready(&mut s, &w);
    s.ship.ship_weapon_tier = 2;
    let err = s.apply(&w, Action::UpgradeShipWeapon).unwrap_err();
    assert!(
        err.0.contains("max tier"),
        "expected max-tier rejection, got: {}",
        err.0
    );
}

#[test]
fn upgrade_ship_weapon_rejected_without_munitions_known() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    // No MunitionsKnown flag, no shipyard.
    let err = s.apply(&w, Action::UpgradeShipWeapon).unwrap_err();
    assert!(
        err.0.contains("munitions"),
        "expected munitions rejection, got: {}",
        err.0
    );
}

#[test]
fn upgrade_ship_weapon_rejected_without_shipyard() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.flags.insert(Flag::MunitionsKnown);
    // No shipyard built.
    let err = s.apply(&w, Action::UpgradeShipWeapon).unwrap_err();
    assert!(
        err.0.contains("shipyard"),
        "expected no-shipyard rejection, got: {}",
        err.0
    );
}

#[test]
fn upgrade_hull_plating_raises_tier_and_hull() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    shipyard_ready(&mut s, &w);
    let hull_before = s.ship.hull;
    assert!(s.apply(&w, Action::UpgradeHullPlating).is_ok());
    assert_eq!(s.ship.hull_plating_tier, 1);
    assert!(
        s.ship.hull > hull_before,
        "hull should increase by hull_plating_hp_step"
    );
    assert_eq!(
        s.ship.hull,
        (hull_before + t.hull_plating_hp_step).min(s.ship.hull_max(&t))
    );
}

#[test]
fn upgrade_hull_plating_damaged_ship_no_clamp() {
    // Damaged ship: hull well below old max, so the full step lands without clamping.
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    shipyard_ready(&mut s, &w);
    let hull_max_before = s.ship.hull_max(&t);
    // Damage the ship so hull is far below the old max.
    s.ship.hull = 50.0;
    assert!(
        50.0 < hull_max_before,
        "sanity: 50 must be below pre-upgrade hull_max"
    );
    let hull_before = s.ship.hull;
    assert!(s.apply(&w, Action::UpgradeHullPlating).is_ok());
    let hull_max_after = s.ship.hull_max(&t);
    // hull_max must have increased by hull_plating_hp_step.
    assert!(
        hull_max_after > hull_max_before,
        "hull_max should increase after upgrade"
    );
    assert!(
        (hull_max_after - hull_max_before - t.hull_plating_hp_step).abs() < 1e-9,
        "hull_max increased by exactly hull_plating_hp_step"
    );
    // Hull rose by exactly the step (no clamping, since hull is still well below new max).
    assert!(
        (s.ship.hull - hull_before - t.hull_plating_hp_step).abs() < 1e-9,
        "hull rose by exactly hull_plating_hp_step with no clamping"
    );
    assert_eq!(s.ship.hull_plating_tier, 1, "tier incremented");
}

#[test]
fn upgrade_hull_plating_rejected_at_max_tier() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    shipyard_ready(&mut s, &w);
    s.ship.hull_plating_tier = 2;
    let err = s.apply(&w, Action::UpgradeHullPlating).unwrap_err();
    assert!(
        err.0.contains("max tier"),
        "expected max-tier rejection, got: {}",
        err.0
    );
}

#[test]
fn upgrade_hull_plating_rejected_without_munitions_known() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let err = s.apply(&w, Action::UpgradeHullPlating).unwrap_err();
    assert!(
        err.0.contains("munitions"),
        "expected munitions rejection, got: {}",
        err.0
    );
}

#[test]
fn upgrade_hull_plating_rejected_without_shipyard() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.flags.insert(Flag::MunitionsKnown);
    let err = s.apply(&w, Action::UpgradeHullPlating).unwrap_err();
    assert!(
        err.0.contains("shipyard"),
        "expected no-shipyard rejection, got: {}",
        err.0
    );
}

#[test]
fn craft_repair_drone_spends_components_and_increments_count() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    shipyard_ready(&mut s, &w);
    let comp_before = s.resources.amount(ResourceKind::Components);
    let drones_before = s.ship.repair_drones;
    assert!(s.apply(&w, Action::CraftRepairDrone).is_ok());
    assert_eq!(s.ship.repair_drones, drones_before + 1);
    assert!(
        s.resources.amount(ResourceKind::Components) < comp_before,
        "components spent"
    );
}

#[test]
fn craft_repair_drone_rejected_without_munitions_known() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let err = s.apply(&w, Action::CraftRepairDrone).unwrap_err();
    assert!(
        err.0.contains("munitions"),
        "expected munitions rejection, got: {}",
        err.0
    );
}

#[test]
fn craft_repair_drone_rejected_without_shipyard() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    s.flags.insert(Flag::MunitionsKnown);
    let err = s.apply(&w, Action::CraftRepairDrone).unwrap_err();
    assert!(
        err.0.contains("shipyard"),
        "expected no-shipyard rejection, got: {}",
        err.0
    );
}

#[test]
fn hull_regen_docked_at_home_below_max() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    // Damage hull below max.
    let hull_max = s.ship.hull_max(&t);
    s.ship.hull = hull_max - 10.0;
    let hull_before = s.ship.hull;
    // Advance 1 tick while docked at HOME with no activity/contact.
    assert_eq!(s.expedition.location, crate::galaxy::HOME);
    assert!(s.expedition.activity.is_none());
    assert!(s.expedition.contact.is_none());
    s.advance(&w, 1);
    assert!(
        s.ship.hull > hull_before,
        "hull should regen while docked at HOME"
    );
    assert!(
        (s.ship.hull - hull_before - t.hull_regen_docked).abs() < 1e-9,
        "hull should regen by exactly hull_regen_docked per tick"
    );
}

#[test]
fn hull_regen_capped_at_hull_max() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let hull_max = s.ship.hull_max(&t);
    // Hull is already at max.
    s.ship.hull = hull_max;
    s.advance(&w, 5);
    assert!(s.ship.hull <= hull_max, "hull must not exceed hull_max");
    assert_eq!(
        s.ship.hull, hull_max,
        "hull stays at max, does not overflow"
    );
}

#[test]
fn hull_regen_does_not_occur_away_from_home() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let hull_max = s.ship.hull_max(&t);
    s.ship.hull = hull_max - 10.0;
    let hull_before = s.ship.hull;
    // Move to a different system (simulate being away by changing location).
    s.expedition.location = crate::galaxy::SystemId(5);
    s.advance(&w, 1);
    assert_eq!(
        s.ship.hull, hull_before,
        "hull must not regen when away from HOME"
    );
}

#[test]
fn hull_regen_suppressed_by_active_contact_at_home() {
    // Even at HOME with hull below max, regen must NOT fire when a contact is present.
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    let hull_max = s.ship.hull_max(&t);
    s.ship.hull = hull_max - 10.0;
    let hull_before = s.ship.hull;
    // Stay at HOME (default location).
    assert_eq!(s.expedition.location, crate::galaxy::HOME);
    assert!(s.expedition.activity.is_none());
    // Inject a minimal contact.
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.0,
        combat: None,
    });
    s.advance(&w, 1);
    assert_eq!(
        s.ship.hull, hull_before,
        "hull must not regen while a contact is active, even at HOME"
    );
}

// ── CB3 Task 10: determinism + re-entry tests ─────────────────────────────

/// Ship-combat determinism: advance(20) == 20 × advance(1) with an active
/// Peer ship fight injected at tick 0.  Hull drains each tick; the fight
/// resolves inside the window, exercising both combat ticks and the win path.
#[test]
fn advance_n_equals_n_times_advance_1_during_ship_combat() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let setup = |s: &mut GameState| {
        s.ship.built = true;
        // Place ship in a non-home system so the expedition is "away".
        s.expedition.location = crate::galaxy::SystemId(5);
        // Inject an active Peer ship fight.  At tier-0, player atk=15 vs
        // peer def=4 → 11 damage/tick; peer hp=90 → resolves in ~9 ticks.
        let combat = crate::contact::peer_ship_stats(0, 1.0, &s.ship, &t);
        // High enough HP that the fight lasts multiple ticks (default 90.0 is fine).
        s.expedition.contact = Some(crate::contact::ShipContact {
            kind: crate::contact::ShipKind::Peer,
            disposition: 0.0,
            combat: Some(combat),
        });
    };
    let mut bulk = GameState::new(&w, 1);
    setup(&mut bulk);
    let mut step = GameState::new(&w, 1);
    setup(&mut step);

    // Sanity: combat must be active at tick 0 for this test to be meaningful.
    assert!(
        bulk.expedition.contact.as_ref().unwrap().combat.is_some(),
        "ship combat must be active at tick 0 for this test to be meaningful"
    );

    bulk.advance(&w, 20);
    for _ in 0..20 {
        step.advance(&w, 1);
    }
    assert_eq!(
        bulk, step,
        "bulk advance and stepped advance must be identical"
    );
}

/// Contact-roll determinism and re-entry skip:
/// 1. `roll_contact` returns the same result on two calls with the same args.
/// 2. A system in `resolved_contacts` is skipped by `maybe_start_contact`.
/// 3. A non-resolved system whose roll is `Some` DOES surface a contact.
#[test]
fn ship_contact_roll_is_deterministic_and_resolved_skips() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);

    // ── 1. roll_contact is pure (same result on two calls) ─────────────────
    let sys4 = crate::galaxy::SystemId(4);
    let first = crate::contact::roll_contact(42, sys4, &t);
    let second = crate::contact::roll_contact(42, sys4, &t);
    assert_eq!(
        first, second,
        "roll_contact must return the same result on repeated calls"
    );
    // Pinned: seed=42, system=4 → Some(Peer) (verified by existing T4 tests).
    assert_eq!(
        first,
        Some(crate::contact::ShipKind::Peer),
        "pinned seed: roll_contact(42, 4) must be Some(Peer)"
    );

    // ── 2. Resolved system skips contact on re-arrival ─────────────────────
    {
        let mut s = GameState::new(&w, 42);
        s.ship.built = true;
        // Mark system 4 as already resolved.
        s.resolved_contacts.insert(4);
        s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys4, ticks: 1 });
        s.advance(&w, 1);
        assert!(
            s.expedition.contact.is_none(),
            "resolved system must not surface a contact on re-arrival"
        );
        assert_eq!(s.expedition.location, sys4);
    }

    // ── 3. Non-resolved system with a hot roll DOES surface a contact ──────
    {
        let mut s = GameState::new(&w, 42);
        s.ship.built = true;
        // System 4 is NOT in resolved_contacts.
        assert!(
            !s.resolved_contacts.contains(&4),
            "system 4 must start unresolved"
        );
        s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys4, ticks: 1 });
        s.advance(&w, 1);
        assert!(
            s.expedition.contact.is_some(),
            "non-resolved system with a hot roll must surface a contact"
        );
    }
}

#[test]
fn expansion_score_and_gating() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    assert_eq!(s.expansion_score(&w), 0.0);
    // Ascendant + Cold always; Communion/PatternBroken not yet earned.
    let e = s.available_endings(&t);
    assert!(e.contains(&Ending::Ascendant) && e.contains(&Ending::Cold));
    assert!(!e.contains(&Ending::Communion) && !e.contains(&Ending::PatternBroken));
    // Peaceful history → Communion + PatternBroken unlock.
    s.peers_spared = 4;
    s.codex.add(crate::codex::Discovery {
        id: crate::codex::DiscoveryId::Note(crate::codex::NoteId::LifeSpared),
        title: "x".into(),
        text: "x".into(),
        tick: 0,
    });
    let e = s.available_endings(&t);
    assert!(e.contains(&Ending::Communion) && e.contains(&Ending::PatternBroken));
    assert!(s.expansion_score(&w) > 0.0); // peers count toward expansion
    // A conqueror loses the peaceful paths.
    s.peers_killed = 5;
    let e = s.available_endings(&t);
    assert!(!e.contains(&Ending::Communion) && !e.contains(&Ending::PatternBroken));
}

#[test]
fn expansion_tier_steps_with_score() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    assert_eq!(s.expansion_tier(&w), 0);
    // peers_spared * w_peers(8) drives the score; cross each threshold.
    s.peers_spared = 4; // 32 >= drift_t1 (30) → tier 1
    assert_eq!(s.expansion_tier(&w), 1);
    s.peers_spared = 8; // 64 >= drift_t2 (60) → tier 2
    assert_eq!(s.expansion_tier(&w), 2);
    s.peers_spared = 13; // 104 >= drift_t3 (100) → tier 3
    assert_eq!(s.expansion_tier(&w), 3);
}

// ── helper: build a GameState that is above the watched_threshold ──────────
fn above_threshold_state(w: &World) -> GameState {
    let mut s = GameState::new(w, 1);
    // 16 peers × 8.0 = 128.0 > 120.0 default threshold.
    s.peers_killed = 16;
    s
}

/// A Transit arrival below threshold fires NO endgame.
#[test]
fn watched_not_triggered_below_threshold() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    // No peers → expansion_score == 0 < 120.
    assert!(s.expansion_score(&w) < t.watched_threshold);
    let sys1 = crate::galaxy::SystemId(1);
    // Pre-resolve to avoid contact suppression masking the threshold check.
    s.resolved_contacts.insert(sys1.0);
    s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys1, ticks: 1 });
    s.advance(&w, 1);
    assert!(
        s.endgame.is_none(),
        "below threshold: no endgame should be set"
    );
}

/// Crossing the threshold on a Transit arrival to a non-home system sets
/// WatchedArrived and fires all three turn notes.
#[test]
fn watched_triggered_on_arrival_above_threshold() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = above_threshold_state(&w);
    assert!(s.expansion_score(&w) >= t.watched_threshold);
    let sys1 = crate::galaxy::SystemId(1);
    // Pre-resolve sys1 so maybe_start_contact skips (no contact will suppress trigger).
    s.resolved_contacts.insert(sys1.0);
    s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys1, ticks: 1 });
    s.advance(&w, 1);
    assert_eq!(
        s.endgame,
        Some(Endgame::WatchedArrived),
        "above threshold: endgame must be WatchedArrived"
    );
    let has_note = |id: crate::codex::NoteId| s.codex.has(crate::codex::DiscoveryId::Note(id));
    assert!(
        has_note(NoteId::TheWatched),
        "TheWatched note must be fired"
    );
    assert!(
        has_note(NoteId::TheRuinsWereHome),
        "TheRuinsWereHome note must be fired"
    );
    assert!(
        has_note(NoteId::ThePeersWereYou),
        "ThePeersWereYou note must be fired"
    );
}

/// Suppression: destination is HOME → no trigger.
#[test]
fn watched_suppressed_when_destination_is_home() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = above_threshold_state(&w);
    // Transit back to HOME.
    s.expedition.location = crate::galaxy::SystemId(1);
    s.expedition.activity = Some(crate::expedition::Activity::Transit {
        to: crate::galaxy::HOME,
        ticks: 1,
    });
    s.advance(&w, 1);
    assert!(
        s.endgame.is_none(),
        "arrival at HOME must not trigger the watched"
    );
}

/// Suppression: a Contact is open → no trigger.
#[test]
fn watched_suppressed_when_contact_open() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = above_threshold_state(&w);
    // Pre-set a contact so it is open when transit completes.
    // We do this by pre-populating resolved_contacts for sys1 so maybe_start_contact
    // will skip, then manually set a contact before advance.
    let sys1 = crate::galaxy::SystemId(1);
    s.resolved_contacts.insert(sys1.0);
    s.expedition.contact = Some(crate::contact::ShipContact {
        kind: crate::contact::ShipKind::Peer,
        disposition: 0.5,
        combat: None,
    });
    s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys1, ticks: 1 });
    s.advance(&w, 1);
    assert!(
        s.endgame.is_none(),
        "open contact must suppress the watched trigger"
    );
}

/// Suppression: player is landed → no trigger.
#[test]
fn watched_suppressed_when_landed() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = above_threshold_state(&w);
    let sys1 = crate::galaxy::SystemId(1);
    // Pre-resolve so contact doesn't open (keeps test focused on landed suppression).
    s.resolved_contacts.insert(sys1.0);
    // Simulate being on a surface.
    s.expedition.landing = Some(crate::landing::Landing {
        system: sys1,
        site_idx: 0,
        node: 0,
        biome: crate::galaxy::Biome::Dead,
        suit: 1.0,
        satchel: Default::default(),
        pending_choice: false,
        traverse: 0,
        combat: None,
        alien: None,
    });
    s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys1, ticks: 1 });
    s.advance(&w, 1);
    assert!(
        s.endgame.is_none(),
        "landed player must not trigger the watched"
    );
}

/// Idempotence: trigger fires only once even if expansion keeps growing.
#[test]
fn watched_fires_only_once() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = above_threshold_state(&w);
    let sys1 = crate::galaxy::SystemId(1);
    // Pre-resolve to avoid contact suppression on first arrival.
    s.resolved_contacts.insert(sys1.0);
    s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys1, ticks: 1 });
    s.advance(&w, 1);
    assert_eq!(s.endgame, Some(Endgame::WatchedArrived));
    let note_count_after_first = s
        .codex
        .entries
        .iter()
        .filter(|d| {
            matches!(
                d.id,
                crate::codex::DiscoveryId::Note(NoteId::TheWatched)
                    | crate::codex::DiscoveryId::Note(NoteId::TheRuinsWereHome)
                    | crate::codex::DiscoveryId::Note(NoteId::ThePeersWereYou)
            )
        })
        .count();
    // Transit to another non-home system — endgame.is_some() guard blocks re-fire.
    let sys2 = crate::galaxy::SystemId(2);
    s.resolved_contacts.insert(sys2.0);
    s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys2, ticks: 1 });
    s.advance(&w, 1);
    let note_count_after_second = s
        .codex
        .entries
        .iter()
        .filter(|d| {
            matches!(
                d.id,
                crate::codex::DiscoveryId::Note(NoteId::TheWatched)
                    | crate::codex::DiscoveryId::Note(NoteId::TheRuinsWereHome)
                    | crate::codex::DiscoveryId::Note(NoteId::ThePeersWereYou)
            )
        })
        .count();
    assert_eq!(
        note_count_after_first, note_count_after_second,
        "turn notes must not be added a second time"
    );
}

// ── Helper: a state at WatchedArrived with Cold always available ────────────
fn watched_arrived_state(w: &World) -> GameState {
    let mut s = above_threshold_state(w);
    // Cold + Ascendant are always available. Force WatchedArrived directly.
    s.endgame = Some(Endgame::WatchedArrived);
    s
}

/// Unearned ending is rejected at WatchedArrived.
#[test]
fn choose_ending_rejects_unearned_path() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = watched_arrived_state(&w);
    // Communion requires peers_spared; default state has 0.
    assert!(!s.available_endings(&t).contains(&Ending::Communion));
    assert_eq!(
        s.apply(&w, Action::ChooseEnding(Ending::Communion)),
        Err(Rejected("you have not earned that path")),
    );
}

/// Earned ending (Cold) sets Ended(Cold) and fires the EndingCold note.
#[test]
fn choose_ending_cold_sets_ended_and_fires_note() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = watched_arrived_state(&w);
    assert!(s.available_endings(&t).contains(&Ending::Cold));
    s.apply(&w, Action::ChooseEnding(Ending::Cold)).unwrap();
    assert_eq!(s.endgame, Some(Endgame::Ended(Ending::Cold)));
    assert!(
        s.codex
            .has(crate::codex::DiscoveryId::Note(NoteId::EndingCold)),
        "EndingCold note must be fired"
    );
}

/// After an ending, all actions are rejected (terminal state).
#[test]
fn terminal_state_rejects_all_actions_after_ending() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = watched_arrived_state(&w);
    s.apply(&w, Action::ChooseEnding(Ending::Cold)).unwrap();
    assert_eq!(s.endgame, Some(Endgame::Ended(Ending::Cold)));
    // Jump (and any non-ChooseEnding action) is blocked.
    assert_eq!(
        s.apply(&w, Action::Jump(crate::galaxy::SystemId(1))),
        Err(Rejected("the watched is waiting")),
    );
    // ChooseEnding also fails because WatchedArrived is gone.
    assert_eq!(
        s.apply(&w, Action::ChooseEnding(Ending::Ascendant)),
        Err(Rejected("the watched is not here")),
    );
}

/// advance after Ended leaves the endgame unchanged.
#[test]
fn advance_after_ending_leaves_endgame_unchanged() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = watched_arrived_state(&w);
    s.apply(&w, Action::ChooseEnding(Ending::Cold)).unwrap();
    assert_eq!(s.endgame, Some(Endgame::Ended(Ending::Cold)));
    s.advance(&w, 10);
    assert_eq!(
        s.endgame,
        Some(Endgame::Ended(Ending::Cold)),
        "endgame must be unchanged after advance"
    );
}

/// ChooseEnding without WatchedArrived is rejected.
#[test]
fn choose_ending_rejected_when_watched_not_here() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);
    let mut s = GameState::new(&w, 1);
    assert!(s.endgame.is_none());
    assert_eq!(
        s.apply(&w, Action::ChooseEnding(Ending::Cold)),
        Err(Rejected("the watched is not here")),
    );
}

/// Drifter and Seed get different note bodies for the same ending.
#[test]
fn different_origins_get_different_ending_text() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);

    let mut drifter = watched_arrived_state(&w);
    // drifter origin is already Drifter (default_origin_def)
    drifter
        .apply(&w, Action::ChooseEnding(Ending::Cold))
        .unwrap();

    let mut seed = watched_arrived_state(&w);
    seed.origin.id = crate::origin::Origin::Seed;
    seed.apply(&w, Action::ChooseEnding(Ending::Cold)).unwrap();

    let drifter_entry = drifter
        .codex
        .entries
        .iter()
        .find(|d| d.id == crate::codex::DiscoveryId::Note(NoteId::EndingCold))
        .expect("drifter must have EndingCold note");
    let seed_entry = seed
        .codex
        .entries
        .iter()
        .find(|d| d.id == crate::codex::DiscoveryId::Note(NoteId::EndingCold))
        .expect("seed must have EndingCold note");

    assert_ne!(
        drifter_entry.text, seed_entry.text,
        "Drifter and Seed must have different ending text"
    );
}

// ── M6a Task 6: determinism through the watched arrival ───────────────────

/// Bulk advance(N) == N × advance(1) across a Transit arrival that fires
/// the watched.  The terminal state (WatchedArrived + all three turn notes)
/// must be bit-for-bit identical between both execution paths.
#[test]
fn advance_n_equals_n_times_advance_1_through_the_watched() {
    let (t, j, m, tc) = (Tuning::default(), jobs(), modules(), techs());
    let w = world(&t, &j, &m, &tc);

    let setup = |s: &mut GameState| {
        // 16 peers × 8.0 = 128.0 ≥ 120.0 threshold — above from tick 0.
        s.peers_killed = 16;
        let sys1 = crate::galaxy::SystemId(1);
        // Pre-resolve so maybe_start_contact skips; contact suppression
        // would prevent the watched from firing.
        s.resolved_contacts.insert(sys1.0);
        // Transit arrives in exactly 1 tick.
        s.expedition.activity = Some(crate::expedition::Activity::Transit { to: sys1, ticks: 1 });
    };

    let mut bulk = GameState::new(&w, 1);
    setup(&mut bulk);
    let mut step = GameState::new(&w, 1);
    setup(&mut step);

    // Sanity: watched NOT present before advancing.
    assert!(
        bulk.endgame.is_none(),
        "endgame must be None before the advance window"
    );

    // Drive N ticks — the arrival (and watched trigger) occurs on tick 1.
    const N: u64 = 5;
    bulk.advance(&w, N);
    for _ in 0..N {
        step.advance(&w, 1);
    }

    // Sanity: watched IS present in the terminal state.
    assert_eq!(
        bulk.endgame,
        Some(Endgame::WatchedArrived),
        "bulk: endgame must be WatchedArrived after the arrival"
    );

    // Core determinism assertion.
    assert_eq!(
        bulk, step,
        "bulk advance({N}) and {N}×advance(1) must produce identical state"
    );
}
