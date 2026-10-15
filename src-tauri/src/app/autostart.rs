//! Launch at login.

use crate::{
    domain::settings,
    shared::error::{AppError, AppResult},
};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> AppResult<()> {
    let autolaunch = app.autolaunch();
    if enabled { autolaunch.enable() } else { autolaunch.disable() }.map_err(|e| AppError::Other(e.to_string()))?;
    settings::commands::changed(&app);
    Ok(())
}
