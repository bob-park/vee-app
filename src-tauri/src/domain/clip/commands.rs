//! Thin Tauri adapters over the clip domain API.

use super::{ClipDto, StatsDto};
use crate::app::{
    state::{AppState, now_ms},
    window,
};
use crate::shared::error::AppResult;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn list_clips(
    state: State<AppState>,
    query: String,
    kind: String,
    app_id: Option<i64>,
    offset: i64,
    limit: i64,
) -> AppResult<Vec<ClipDto>> {
    super::list(&state.db.lock().unwrap(), &query, &kind, app_id, offset, limit)
}

#[tauri::command]
pub fn list_pinned(state: State<AppState>) -> AppResult<Vec<ClipDto>> {
    super::list_pinned(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn delete_clip(app: AppHandle, state: State<AppState>, id: i64) -> AppResult<()> {
    super::delete(&state.db.lock().unwrap(), id)?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle, state: State<AppState>) -> AppResult<()> {
    super::clear(&state.db.lock().unwrap())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
pub fn copy_clip(app: AppHandle, id: i64, plain: bool) -> AppResult<()> {
    window::copy_clip(&app, id, plain)
}

#[tauri::command]
pub fn set_pinned(app: AppHandle, state: State<AppState>, id: i64, pinned: bool) -> AppResult<()> {
    super::set_pinned(&state.db.lock().unwrap(), id, pinned, now_ms())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
pub fn get_stats(state: State<AppState>) -> AppResult<StatsDto> {
    super::stats(&state.db.lock().unwrap(), &state.images_dir)
}

#[tauri::command]
pub fn start_drag(app: AppHandle, id: i64, name: String, image: Option<String>) -> AppResult<()> {
    window::start_drag(&app, id, &name, image.as_deref())
}
