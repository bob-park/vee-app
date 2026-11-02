//! Thin Tauri adapters over the settings domain API.

use super::{SILENT, SettingsDto, get};
use crate::app::{
    ocr,
    retention::prune_now,
    shortcut::{self, Global},
    sound,
    state::{AppState, now_ms},
    tray, window,
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
        capture_shortcut: get(&conn, "captureShortcut"),
        record_shortcut: get(&conn, "recordShortcut"),
        panel_pin_shortcut: get(&conn, "panelPinShortcut"),
        reset_search_shortcut: get(&conn, "resetSearchShortcut"),
        pinned_copy1_shortcut: get(&conn, "pinnedCopy1Shortcut"),
        pinned_copy2_shortcut: get(&conn, "pinnedCopy2Shortcut"),
        pinned_copy3_shortcut: get(&conn, "pinnedCopy3Shortcut"),
        sound: get(&conn, "sound"),
        sound_name: get(&conn, "soundName"),
        confirm_delete: get(&conn, "confirmDelete"),
        retention: get(&conn, "retention"),
        panel_resize: get(&conn, "panelResize"),
        panel_height_ratio: get(&conn, "panelHeightRatio"),
        panel_width_ratio: get(&conn, "panelWidthRatio"),
        panel_position: get(&conn, "panelPosition"),
        ocr: get(&conn, "ocr"),
        drag_convert: get(&conn, "dragConvert"),
        convert_key: get(&conn, "convertKey"),
        video_quality: get(&conn, "videoQuality"),
        video_hw_accel: get(&conn, "videoHwAccel"),
        record_folder: get(&conn, "recordFolder"),
        record_format: get(&conn, "recordFormat"),
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
    if key == "panelPosition" {
        window::move_panel(&app);
    }
    if key == "ocr" {
        ocr::wake();
    }
    changed(&app);
    Ok(())
}

/// Swaps one global shortcut (`key` = its setting key); on failure the previous one is restored.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, state: State<AppState>, key: String, accel: String) -> AppResult<()> {
    let global = Global::from_key(&key).ok_or_else(|| AppError::InvalidInput(format!("unknown shortcut {key}")))?;
    let (old, taken) = {
        let conn = state.db.lock().unwrap();
        let others: Vec<String> = Global::ALL.into_iter().filter(|g| *g != global).map(|g| get(&conn, g.key())).collect();
        (get(&conn, &key), others)
    };
    let failed = shortcut::failures().contains(&global.key());
    if accel == old && !failed {
        return Ok(());
    }
    if taken.iter().any(|t| shortcut::same_accel(t, &accel)) {
        return Err(AppError::InvalidInput(format!("shortcut {accel} is already used")));
    }
    let _ = app.global_shortcut().unregister(old.as_str());
    if let Err(e) = shortcut::register(&app, global, &accel) {
        let _ = shortcut::register(&app, global, &old);
        return Err(e);
    }
    shortcut::set_failed(global, false);
    super::set_raw(&state.db.lock().unwrap(), &key, &accel)?;
    changed(&app);
    Ok(())
}

/// Setting keys of the global shortcuts that failed to register at startup.
#[tauri::command]
pub fn get_shortcut_failures() -> Vec<String> {
    shortcut::failures().into_iter().map(String::from).collect()
}

/// Opens the OS keyboard shortcut settings, where a conflicting system shortcut can be turned off.
#[tauri::command]
pub fn open_keyboard_settings() -> AppResult<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.Keyboard-Settings.extension").spawn()?;
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
