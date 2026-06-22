//! The wall-clock tick driver. This is the ONLY place real time is read.
//! It converts elapsed time into whole ticks (via the pure `sim` helper) and
//! advances the simulation, then triggers a save.

use dioxus::prelude::*;
use sim::Tuning;
use sim::state::GameState;
use sim::time::ticks_for_elapsed;

use crate::persist;

pub fn use_tick_driver(
    mut state: Signal<GameState>,
    tuning: Signal<Tuning>,
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
                    let t = tuning.read();
                    state.write().advance(&t, n);
                }
                anchor += n as f64 * tick_ms;
                persist::save(&state.read());
            }
        }
    });
}
