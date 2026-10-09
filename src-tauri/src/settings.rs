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
    retention: String,
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
        "retention" => "off",
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

const DAY_MS: i64 = 86_400_000;
const PRUNE_EVERY: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Clips last used before this (ms) are pruned; `None` keeps everything.
pub fn cutoff_for(value: &str, now: i64) -> Option<i64> {
    let days: i64 = value.parse().ok()?;
    Some(now - days * DAY_MS)
}

/// Prunes by the saved retention and tells the panel when anything went.
pub fn prune_now(app: &AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let n = {
        let store = state.store.lock().unwrap();
        match cutoff_for(&get(&store, "retention"), crate::now_ms()) {
            Some(cutoff) => store.prune(cutoff).map_err(|e| e.to_string())?,
            None => 0,
        }
    };
    if n > 0 {
        log::info!("pruned {n} clips");
        let _ = app.emit("clips://changed", ());
    }
    Ok(n)
}

/// Prunes now and then every hour.
pub fn spawn_pruner(app: AppHandle) {
    std::thread::spawn(move || loop {
        if let Err(e) = prune_now(&app) {
            log::warn!("pruning failed: {e}");
        }
        std::thread::sleep(PRUNE_EVERY);
    });
}

/// How many clips a not-yet-saved retention value would delete right away.
#[tauri::command]
pub fn count_prunable(state: State<AppState>, value: String) -> Result<i64, String> {
    validate("retention", &value)?;
    match cutoff_for(&value, crate::now_ms()) {
        Some(cutoff) => state.store.lock().unwrap().prunable_count(cutoff).map_err(|e| e.to_string()),
        None => Ok(0),
    }
}

fn validate(key: &str, value: &str) -> Result<(), String> {
    let ok = match key {
        "theme" => matches!(value, "system" | "light" | "dark"),
        "locale" => matches!(value, "system" | "ko" | "en"),
        "sound" => matches!(value, "on" | "off"),
        "soundName" => value == SILENT || crate::sound::NAMES.contains(&value),
        "confirmDelete" => matches!(value, "on" | "off"),
        "retention" => matches!(value, "off" | "7" | "30" | "90"),
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
        retention: get(&store, "retention"),
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
    if key == "retention" {
        prune_now(&app)?;
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
    fn retention_defaults_off_and_accepts_known_periods() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert_eq!(get(&store, "retention"), "off");
        for v in ["off", "7", "30", "90"] {
            assert!(validate("retention", v).is_ok(), "{v}");
        }
        assert!(validate("retention", "14").is_err());
        assert!(validate("retention", "").is_err());
    }

    #[test]
    fn cutoff_is_none_when_off_and_days_back_otherwise() {
        const DAY: i64 = 86_400_000;
        let now = 100 * DAY;
        assert_eq!(cutoff_for("off", now), None);
        assert_eq!(cutoff_for("7", now), Some(93 * DAY));
        assert_eq!(cutoff_for("90", now), Some(10 * DAY));
        assert_eq!(cutoff_for("bogus", now), None);
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
