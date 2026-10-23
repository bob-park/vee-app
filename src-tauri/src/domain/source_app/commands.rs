//! Thin Tauri adapters over the source-app domain API.

use super::AppDto;
use crate::app::state::AppState;
use crate::shared::error::AppResult;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn list_apps(state: State<AppState>, query: String) -> AppResult<Vec<AppDto>> {
    super::apps(&state.db.lock().unwrap(), &query)
}

#[tauri::command]
pub fn list_known_apps(state: State<AppState>) -> AppResult<Vec<AppDto>> {
    super::known_apps(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn count_app_clips(state: State<AppState>, app_id: i64) -> AppResult<i64> {
    super::app_clip_count(&state.db.lock().unwrap(), app_id)
}

#[tauri::command]
pub fn set_app_excluded(app: AppHandle, state: State<AppState>, app_id: i64, excluded: bool) -> AppResult<()> {
    let n = super::set_excluded(&state.db.lock().unwrap(), app_id, excluded)?;
    if n > 0 {
        log::info!("deleted {n} clips from an excluded app");
    }
    let _ = app.emit("clips://changed", ());
    let _ = app.emit("settings://changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_excluded_apps(state: State<AppState>) -> AppResult<Vec<AppDto>> {
    super::excluded_apps(&state.db.lock().unwrap())
}
