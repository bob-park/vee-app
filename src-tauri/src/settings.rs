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
    sound: String,
    sound_name: String,
    confirm_delete: String,
    autostart: bool,
    version: String,
}

/// Stored value, or the default for a known key.
pub fn get(store: &Store, key: &str) -> String {
    let default = match key {
        "theme" | "locale" => "system",
        "shortcut" => DEFAULT_SHORTCUT,
        "sound" => "on",
        "soundName" => "pop",
        "confirmDelete" => "off",
        _ => "",
    };
    store.get_setting(key).ok().flatten().unwrap_or_else(|| default.to_string())
}

/// `soundName` value that keeps copies silent while the sound switch stays on.
pub const SILENT: &str = "none";

/// The sound to play on copy, or `None` when copy sounds are off or set to silent.
pub fn copy_sound(store: &Store) -> Option<String> {
    let name = get(store, "soundName");
    (get(store, "sound") == "on" && name != SILENT).then_some(name)
}

fn validate(key: &str, value: &str) -> Result<(), String> {
    let ok = match key {
        "theme" => matches!(value, "system" | "light" | "dark"),
        "locale" => matches!(value, "system" | "ko" | "en"),
        "sound" => matches!(value, "on" | "off"),
        "soundName" => value == SILENT || crate::sound::NAMES.contains(&value),
        "confirmDelete" => matches!(value, "on" | "off"),
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
        sound: get(&store, "sound"),
        sound_name: get(&store, "soundName"),
        confirm_delete: get(&store, "confirmDelete"),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn set_setting(app: AppHandle, state: State<AppState>, key: String, value: String) -> Result<(), String> {
    validate(&key, &value)?;
    state.store.lock().unwrap().set_setting(&key, &value).map_err(|e| e.to_string())?;
    if key == "locale" {
        crate::tray::refresh(&app);
    }
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
    fn copy_sound_defaults_on_and_accepts_on_off() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert_eq!(get(&store, "sound"), "on");
        assert!(validate("sound", "off").is_ok());
        assert!(validate("sound", "on").is_ok());
        assert!(validate("sound", "loud").is_err());
    }

    #[test]
    fn confirm_delete_defaults_off_and_accepts_on_off() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert_eq!(get(&store, "confirmDelete"), "off");
        assert!(validate("confirmDelete", "on").is_ok());
        assert!(validate("confirmDelete", "off").is_ok());
        assert!(validate("confirmDelete", "yes").is_err());
    }

    #[test]
    fn copy_sound_is_the_chosen_sound_or_none_when_off() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert_eq!(copy_sound(&store).as_deref(), Some("pop"));
        store.set_setting("soundName", "chime").unwrap();
        assert_eq!(copy_sound(&store).as_deref(), Some("chime"));
        store.set_setting("soundName", "none").unwrap();
        assert_eq!(copy_sound(&store), None);
        store.set_setting("soundName", "chime").unwrap();
        store.set_setting("sound", "off").unwrap();
        assert_eq!(copy_sound(&store), None);
    }

    #[test]
    fn sound_name_accepts_only_built_in_sounds() {
        for name in crate::sound::NAMES {
            assert!(validate("soundName", name).is_ok(), "{name}");
        }
        assert!(validate("soundName", "none").is_ok());
        assert!(validate("soundName", "Pop").is_err());
        assert!(validate("soundName", "../x.wav").is_err());
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
