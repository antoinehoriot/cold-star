//! Browser persistence: localStorage autosave (+offline catch-up) and the
//! shareable URL-fragment link. Wall-clock and DOM access live here, never in `sim`.

use gloo_storage::{LocalStorage, Storage};
use sim::Tuning;
use sim::save;
use sim::state::GameState;
use sim::time::ticks_for_elapsed;

const KEY_SAVE: &str = "coldstar.save";
const KEY_SAVE_AT: &str = "coldstar.save_at";
const TICK_MS: f64 = 1000.0;
const OFFLINE_CAP: u64 = 8 * 60 * 60;

/// Choose the starting state: shared link > local save (+catch-up) > new game.
pub fn load_initial(tuning: &Tuning) -> GameState {
    if let Some(payload) = link_payload() {
        match save::decode(&payload) {
            Ok(state) => {
                if confirm(
                    "Load the shared game from this link? Your local save is kept until you next save.",
                ) {
                    clear_fragment();
                    return state;
                }
            }
            Err(_) => alert("This save link is invalid or corrupted."),
        }
    }

    if let Ok(encoded) = LocalStorage::get::<String>(KEY_SAVE) {
        if let Ok(mut state) = save::decode(&encoded) {
            if let Ok(saved_at) = LocalStorage::get::<f64>(KEY_SAVE_AT) {
                let n = ticks_for_elapsed(js_sys::Date::now() - saved_at, TICK_MS, OFFLINE_CAP);
                state.advance(tuning, n);
            }
            return state;
        }
    }

    GameState::new(tuning, seed_from_clock())
}

/// Persist the current state plus a timestamp for offline catch-up.
pub fn save(state: &GameState) {
    if let Ok(encoded) = save::encode(state) {
        let _ = LocalStorage::set(KEY_SAVE, encoded);
        let _ = LocalStorage::set(KEY_SAVE_AT, js_sys::Date::now());
    }
}

/// Put a shareable link in the address bar (fragment) and the clipboard.
pub fn copy_link(state: &GameState) {
    if let Ok(encoded) = save::encode(state) {
        if let Some(win) = web_sys::window() {
            let _ = win.location().set_hash(&format!("s={encoded}"));
            if let Ok(href) = win.location().href() {
                let _ = win.navigator().clipboard().write_text(&href);
            }
            alert(
                "Save link is in the address bar (and copied to your clipboard if your browser allowed it).",
            );
        }
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

fn seed_from_clock() -> u64 {
    js_sys::Date::now().to_bits()
}
