//! The wall-clock tick driver. This is the ONLY place real time is read.

use dioxus::prelude::*;
use sim::Tuning;
use sim::job::{Job, JobRegistry};
use sim::module::{Module, ModuleRegistry};
use sim::research::{Tech, TechRegistry};
use sim::state::{GameState, World};
use sim::time::ticks_for_elapsed;

use crate::persist;

/// One sim tick per wall-clock second.
pub const TICK_MS: f64 = 1000.0;
/// Max ticks granted for time spent away (offline or suspended): 8 hours.
pub const OFFLINE_CAP: u64 = 8 * 60 * 60;
/// How often we poll the wall clock.
const POLL_MS: u32 = 250;

pub fn use_tick_driver(
    mut state: Signal<Option<GameState>>,
    tuning: Signal<Tuning>,
    jobs: Signal<Vec<Job>>,
    modules: Signal<Vec<Module>>,
    techs: Signal<Vec<Tech>>,
) {
    use_future(move || async move {
        // Content never changes after startup: build the registries once.
        let t = tuning.read().clone();
        let j = JobRegistry(jobs.read().clone());
        let m = ModuleRegistry(modules.read().clone());
        let tc = TechRegistry(techs.read().clone());
        let mut anchor = js_sys::Date::now();
        loop {
            gloo_timers::future::TimeoutFuture::new(POLL_MS).await;
            let now = js_sys::Date::now();
            let n = ticks_for_elapsed(now - anchor, TICK_MS, OFFLINE_CAP);
            if n > 0 {
                {
                    let world = World {
                        tuning: &t,
                        jobs: &j,
                        modules: &m,
                        techs: &tc,
                    };
                    if let Some(gs) = state.write().as_mut() {
                        gs.advance(&world, n);
                    }
                }
                if n >= OFFLINE_CAP {
                    // The grant hit the cap: discard the remaining backlog, or the
                    // next polls would re-grant 8h chunks until the full absence
                    // is replayed, defeating the cap (and freezing the UI).
                    anchor = now;
                } else {
                    anchor += n as f64 * TICK_MS;
                }
                if let Some(gs) = state.read().as_ref() {
                    persist::save(gs);
                }
            }
        }
    });
}
