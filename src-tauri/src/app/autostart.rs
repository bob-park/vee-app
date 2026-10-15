//! Launch at login.

use crate::settings;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled { autolaunch.enable() } else { autolaunch.disable() }.map_err(|e| e.to_string())?;
    settings::changed(&app);
    Ok(())
}
