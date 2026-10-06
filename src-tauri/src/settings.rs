//! User settings and the global shortcut.

use crate::{AppState, store::Store};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    theme: String,
    locale: String,
    shortcut: String,
    autostart: bool,
    version: String,
}

/// Stored value, or the default for a known key.
pub fn get(store: &Store, key: &str) -> String {
    let default = match key {
        "theme" | "locale" => "system",
        "shortcut" => DEFAULT_SHORTCUT,
        _ => "",
    };
    store.get_setting(key).ok().flatten().unwrap_or_else(|| default.to_string())
}

fn validate(key: &str, value: &str) -> Result<(), String> {
    let ok = match key {
        "theme" => matches!(value, "system" | "light" | "dark"),
        "locale" => matches!(value, "system" | "ko" | "en"),
        _ => false,
    };
    if ok { Ok(()) } else { Err(format!("invalid setting {key}={value}")) }
}

pub fn register_shortcut(app: &AppHandle, accel: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(accel, |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                crate::windows::toggle_panel(app);
            }
        })
        .map_err(|e| e.to_string())
}

/// Registers the saved shortcut at startup, falling back to the default.
pub fn register_stored_shortcut(app: &AppHandle) {
    let stored = get(&app.state::<AppState>().store.lock().unwrap(), "shortcut");
    if let Err(e) = register_shortcut(app, &stored) {
        log::error!("failed to register shortcut {stored}: {e}");
        if stored != DEFAULT_SHORTCUT {
            if let Err(e) = register_shortcut(app, DEFAULT_SHORTCUT) {
                log::error!("failed to register default shortcut: {e}");
            }
        }
    }
}

fn changed(app: &AppHandle) {
    let _ = app.emit("settings://changed", ());
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<AppState>) -> SettingsDto {
    let store = state.store.lock().unwrap();
    SettingsDto {
        theme: get(&store, "theme"),
        locale: get(&store, "locale"),
        shortcut: get(&store, "shortcut"),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn set_setting(app: AppHandle, state: State<AppState>, key: String, value: String) -> Result<(), String> {
    validate(&key, &value)?;
    state.store.lock().unwrap().set_setting(&key, &value).map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled { autolaunch.enable() } else { autolaunch.disable() }.map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}

/// Swaps the global shortcut; on failure the previous one is restored.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, state: State<AppState>, accel: String) -> Result<(), String> {
    let old = get(&state.store.lock().unwrap(), "shortcut");
    if accel == old {
        return Ok(());
    }
    let _ = app.global_shortcut().unregister(old.as_str());
    if let Err(e) = register_shortcut(&app, &accel) {
        let _ = register_shortcut(&app, &old);
        return Err(e);
    }
    state.store.lock().unwrap().set_setting("shortcut", &accel).map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_known_settings_only() {
        assert!(validate("theme", "dark").is_ok());
        assert!(validate("locale", "ko").is_ok());
        assert!(validate("theme", "purple").is_err());
        assert!(validate("shortcut", "Alt+X").is_err()); // shortcuts go through set_shortcut
        assert!(validate("unknown", "x").is_err());
    }

    #[test]
    fn missing_settings_fall_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert_eq!(get(&store, "theme"), "system");
        assert_eq!(get(&store, "shortcut"), DEFAULT_SHORTCUT);
        store.set_setting("theme", "dark").unwrap();
        assert_eq!(get(&store, "theme"), "dark");
    }
}
