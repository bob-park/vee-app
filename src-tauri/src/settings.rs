//! User settings and the global shortcut.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

pub fn register_shortcut(app: &AppHandle, accel: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(accel, |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                crate::windows::toggle_panel(app);
            }
        })
        .map_err(|e| e.to_string())
}
