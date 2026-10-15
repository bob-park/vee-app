//! Thin Tauri adapters over the settings domain API.

use super::{SILENT, SettingsDto, get};
use crate::app::{
    retention::prune_now,
    shortcut::register_shortcut,
    sound,
    state::{AppState, now_ms},
    tray,
};
use crate::shared::error::{AppError, AppResult};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// Tells every window to reload its settings.
pub fn changed(app: &AppHandle) {
    let _ = app.emit("settings://changed", ());
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<AppState>) -> SettingsDto {
    let conn = state.db.lock().unwrap();
    SettingsDto {
        theme: get(&conn, "theme"),
        locale: get(&conn, "locale"),
        shortcut: get(&conn, "shortcut"),
        panel_pin_shortcut: get(&conn, "panelPinShortcut"),
        reset_search_shortcut: get(&conn, "resetSearchShortcut"),
        sound: get(&conn, "sound"),
        sound_name: get(&conn, "soundName"),
        confirm_delete: get(&conn, "confirmDelete"),
        retention: get(&conn, "retention"),
        panel_resize: get(&conn, "panelResize"),
        panel_height_ratio: get(&conn, "panelHeightRatio"),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn set_setting(app: AppHandle, state: State<AppState>, key: String, value: String) -> AppResult<()> {
    super::set(&state.db.lock().unwrap(), &key, &value)?;
    if key == "locale" {
        tray::refresh(&app);
    }
    if key == "retention" {
        prune_now(&app)?;
    }
    changed(&app);
    Ok(())
}

/// Swaps the global shortcut; on failure the previous one is restored.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, state: State<AppState>, accel: String) -> AppResult<()> {
    let old = get(&state.db.lock().unwrap(), "shortcut");
    if accel == old {
        return Ok(());
    }
    let _ = app.global_shortcut().unregister(old.as_str());
    if let Err(e) = register_shortcut(&app, &accel) {
        let _ = register_shortcut(&app, &old);
        return Err(e);
    }
    super::set_raw(&state.db.lock().unwrap(), "shortcut", &accel)?;
    changed(&app);
    Ok(())
}

/// How many clips a not-yet-saved retention value would delete right away.
#[tauri::command]
pub fn count_prunable(state: State<AppState>, value: String) -> AppResult<i64> {
    super::count_prunable(&state.db.lock().unwrap(), &value, now_ms())
}

/// Plays a built-in copy sound for the settings preview, regardless of the on/off setting.
#[tauri::command]
pub fn preview_sound(name: String) -> AppResult<()> {
    if name == SILENT {
        return Ok(());
    }
    if sound::wav(&name).is_none() {
        return Err(AppError::InvalidInput(format!("unknown sound: {name}")));
    }
    sound::play(&name);
    Ok(())
}
