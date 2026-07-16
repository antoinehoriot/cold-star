//! Browser persistence: localStorage autosave (+offline catch-up) and the
//! shareable URL-fragment link. Wall-clock and DOM access live here, never in `sim`.

use gloo_storage::{LocalStorage, Storage};
use sim::Tuning;
use sim::job::{Job, JobRegistry};
use sim::module::{Module, ModuleRegistry};
use sim::research::{Tech, TechRegistry};
use sim::save;
use sim::state::{GameState, World};
use sim::time::ticks_for_elapsed;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::tick::{OFFLINE_CAP, TICK_MS};

const KEY_SAVE: &str = "coldstar.save";
const KEY_SAVE_AT: &str = "coldstar.save_at";
/// Where an unreadable local save is parked instead of being overwritten.
const KEY_SAVE_CORRUPT: &str = "coldstar.save.corrupt";

/// Alert about failing autosaves only once per session.
static SAVE_FAILURE_ALERTED: AtomicBool = AtomicBool::new(false);

/// Choose the starting state: shared link > local save (+catch-up) > none.
/// `None` means there is no game yet — the UI shows origin select.
pub fn load_initial(
    tuning: &Tuning,
    jobs: &[Job],
    modules: &[Module],
    techs: &[Tech],
) -> Option<GameState> {
    if let Some(payload) = link_payload() {
        // Always clear the fragment once the link has been handled, whatever the
        // outcome: a lingering `#s=` would re-prompt on every reload and offer to
        // roll the player back to their own stale link.
        match save::decode(&payload) {
            Ok(state) => {
                let load = confirm(
                    "Load the shared game from this link? Your local save is kept until you next save.",
                );
                clear_fragment();
                if load {
                    return Some(state);
                }
            }
            Err(_) => {
                alert("This save link is invalid or corrupted.");
                clear_fragment();
            }
        }
    }

    if let Ok(encoded) = LocalStorage::get::<String>(KEY_SAVE) {
        match save::decode(&encoded) {
            Ok(mut state) => {
                if let Ok(saved_at) = LocalStorage::get::<f64>(KEY_SAVE_AT) {
                    let n = ticks_for_elapsed(js_sys::Date::now() - saved_at, TICK_MS, OFFLINE_CAP);
                    let j = JobRegistry(jobs.to_vec());
                    let m = ModuleRegistry(modules.to_vec());
                    let tc = TechRegistry(techs.to_vec());
                    let world = World {
                        tuning,
                        jobs: &j,
                        modules: &m,
                        techs: &tc,
                    };
                    state.advance(&world, n);
                }
                return Some(state);
            }
            Err(_) => {
                // Don't let the first autosave of a fresh run overwrite a blob
                // that might be recoverable (e.g. after a format bug): park it.
                let _ = LocalStorage::set(KEY_SAVE_CORRUPT, encoded);
                alert(
                    "Your local save could not be read, so a new game will start. The old data was kept in browser storage under 'coldstar.save.corrupt'.",
                );
            }
        }
    }

    None
}

/// Persist the current state plus a timestamp for offline catch-up.
/// A failing autosave (quota, blocked storage) alerts once per session instead
/// of letting the player lose hours of progress silently.
pub fn save(state: &GameState) {
    let saved = match save::encode(state) {
        // NOTE: offline catch-up is driven by this LOCAL timestamp only. Shareable links
        // deliberately carry no timestamp, so a recipient never gets free offline progress.
        // The timestamp is only written after the save itself succeeded, so a stale
        // timestamp can never pair with a newer blob.
        Ok(encoded) => {
            LocalStorage::set(KEY_SAVE, encoded).is_ok()
                && LocalStorage::set(KEY_SAVE_AT, js_sys::Date::now()).is_ok()
        }
        Err(_) => false,
    };
    if !saved && !SAVE_FAILURE_ALERTED.swap(true, Ordering::Relaxed) {
        alert(
            "Autosave is failing (browser storage may be full or blocked). Your progress will be lost when this tab closes — use 'Copy save link' to keep it.",
        );
    }
}

/// Put a shareable link in the address bar (fragment) and the clipboard.
pub fn copy_link(state: &GameState) {
    if let Ok(encoded) = save::encode(state)
        && let Some(win) = web_sys::window()
    {
        let _ = win.location().set_hash(&format!("s={encoded}"));
        if let Ok(href) = win.location().href() {
            let _ = win.navigator().clipboard().write_text(&href);
        }
        alert(
            "Save link is in the address bar (and copied to your clipboard if your browser allowed it).",
        );
    }
}

fn link_payload() -> Option<String> {
    let hash = web_sys::window()?.location().hash().ok()?;
    let hash = hash.strip_prefix('#').unwrap_or(&hash);
    hash.split('&')
        .find_map(|kv| kv.strip_prefix("s="))
        .map(|s| s.to_string())
}

fn clear_fragment() {
    if let Some(win) = web_sys::window() {
        let _ = win.location().set_hash("");
    }
}

fn confirm(msg: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(msg).ok())
        .unwrap_or(false)
}

fn alert(msg: &str) {
    if let Some(w) = web_sys::window() {
        let _ = w.alert_with_message(msg);
    }
}

pub fn seed_from_clock() -> u64 {
    js_sys::Date::now().to_bits()
}
