//! The wall-clock tick driver. This is the ONLY place real time is read.

use dioxus::prelude::*;
use sim::Tuning;
use sim::job::{Job, JobRegistry};
use sim::module::{Module, ModuleRegistry};
use sim::state::{GameState, World};
use sim::time::ticks_for_elapsed;

use crate::persist;

pub fn use_tick_driver(
    mut state: Signal<Option<GameState>>,
    tuning: Signal<Tuning>,
    jobs: Signal<Vec<Job>>,
    modules: Signal<Vec<Module>>,
    tick_ms: f64,
    offline_cap: u64,
) {
    use_future(move || async move {
        let mut anchor = js_sys::Date::now();
        loop {
            gloo_timers::future::TimeoutFuture::new(250).await;
            let now = js_sys::Date::now();
            let n = ticks_for_elapsed(now - anchor, tick_ms, offline_cap);
            if n > 0 {
                {
                    let t = tuning.read().clone();
                    let j = JobRegistry(jobs.read().clone());
                    let m = ModuleRegistry(modules.read().clone());
                    let world = World {
                        tuning: &t,
                        jobs: &j,
                        modules: &m,
                    };
                    if let Some(gs) = state.write().as_mut() {
                        gs.advance(&world, n);
                    }
                }
                anchor += n as f64 * tick_ms;
                if let Some(gs) = state.read().as_ref() {
                    persist::save(gs);
                }
            }
        }
    });
}
