//! Global (OS-wide) shortcuts: toggle the panel, capture the screen, record the screen.

use crate::{
    app::state::AppState,
    domain::settings,
    shared::error::{AppError, AppResult},
};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Global {
    Panel,
    Capture,
    Record,
}

impl Global {
    pub const ALL: [Global; 3] = [Global::Panel, Global::Capture, Global::Record];

    /// The setting key that stores this shortcut.
    pub fn key(self) -> &'static str {
        match self {
            Global::Panel => "shortcut",
            Global::Capture => "captureShortcut",
            Global::Record => "recordShortcut",
        }
    }

    pub fn from_key(key: &str) -> Option<Global> {
        Self::ALL.into_iter().find(|g| g.key() == key)
    }
}

/// Setting keys whose shortcut failed to register (taken by the OS or another app).
static FAILED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

pub fn failures() -> Vec<&'static str> {
    FAILED.lock().unwrap().clone()
}

pub fn set_failed(global: Global, failed: bool) {
    let mut list = FAILED.lock().unwrap();
    list.retain(|k| *k != global.key());
    if failed {
        list.push(global.key());
    }
}

/// Whether two accelerators name the same keys ("Shift+V" == "shift+KeyV"); unparsable ones compare as text.
pub fn same_accel(a: &str, b: &str) -> bool {
    match (a.parse::<Shortcut>(), b.parse::<Shortcut>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

pub fn register(app: &AppHandle, global: Global, accel: &str) -> AppResult<()> {
    app.global_shortcut()
        .on_shortcut(accel, move |app, _shortcut, event| {
            if !matches!(event.state, ShortcutState::Pressed) {
                return;
            }
            match global {
                Global::Panel => crate::app::window::toggle_panel(app),
                Global::Capture => crate::app::capture::open(app, crate::app::capture::Mode::Capture),
                // Task 7 swaps this for `crate::app::recorder::on_shortcut(app)`.
                Global::Record => log::info!("record shortcut pressed"),
            }
        })
        .map_err(|e| AppError::Other(e.to_string()))
}

/// Registers every saved global at startup; one failing doesn't stop the others and is kept for `failures()`.
/// Only the panel falls back to its default.
pub fn register_stored(app: &AppHandle) {
    let stored = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        Global::ALL.map(|g| (g, settings::get(&conn, g.key())))
    };
    for (global, accel) in stored {
        let Err(e) = register(app, global, &accel) else { continue };
        log::error!("failed to register {} shortcut {accel}: {e}", global.key());
        // Flagged even when the fallback works: the settings row still shows the stored accelerator.
        set_failed(global, true);
        if global == Global::Panel
            && accel != settings::DEFAULT_SHORTCUT
            && let Err(e) = register(app, global, settings::DEFAULT_SHORTCUT)
        {
            log::error!("failed to register default shortcut: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globals_map_to_their_setting_keys() {
        for g in Global::ALL {
            assert_eq!(Global::from_key(g.key()), Some(g));
        }
        assert_eq!(Global::Capture.key(), "captureShortcut");
        assert_eq!(Global::from_key("panelPinShortcut"), None);
    }

    #[test]
    fn same_accel_ignores_spelling() {
        assert!(same_accel("CommandOrControl+Shift+V", "CommandOrControl+Shift+KeyV"));
        assert!(!same_accel("CommandOrControl+Shift+KeyV", "CommandOrControl+Alt+Shift+KeyV"));
        assert!(same_accel("???", "???"));
    }

    #[test]
    fn failures_are_set_and_cleared_per_key() {
        set_failed(Global::Record, true);
        set_failed(Global::Record, true);
        assert_eq!(failures().iter().filter(|k| **k == "recordShortcut").count(), 1);
        set_failed(Global::Record, false);
        assert!(!failures().contains(&"recordShortcut"));
    }
}
