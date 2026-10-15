//! The global shortcut that toggles the panel.

use crate::{app::state::AppState, settings};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn register_shortcut(app: &AppHandle, accel: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(accel, |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                crate::app::window::toggle_panel(app);
            }
        })
        .map_err(|e| e.to_string())
}

/// Registers the saved shortcut at startup, falling back to the default.
pub fn register_stored_shortcut(app: &AppHandle) {
    let stored = settings::get(&app.state::<AppState>().store.lock().unwrap(), "shortcut");
    if let Err(e) = register_shortcut(app, &stored) {
        log::error!("failed to register shortcut {stored}: {e}");
        if stored != settings::DEFAULT_SHORTCUT {
            if let Err(e) = register_shortcut(app, settings::DEFAULT_SHORTCUT) {
                log::error!("failed to register default shortcut: {e}");
            }
        }
    }
}
