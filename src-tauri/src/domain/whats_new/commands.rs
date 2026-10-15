//! Tauri adapters for the what's-new window.

use super::WhatsNew;
use crate::app::{state::AppState, window};
use tauri::{AppHandle, Manager};

fn current_due(app: &AppHandle) -> Option<WhatsNew> {
    super::due(&app.state::<AppState>().db.lock().unwrap(), &app.package_info().version.to_string())
}

/// Called at startup, after the windows exist.
pub fn show_if_due(app: &AppHandle) {
    if current_due(app).is_none() {
        return;
    }
    if let Some(window) = app.get_webview_window(window::WHATS_NEW) {
        let _ = window.show();
        // An Accessory app isn't active at launch; without focus the window opens behind others.
        let _ = window.set_focus();
    }
}

/// Forgets the notes so they're shown only once, and hides the window.
pub fn dismiss(app: &AppHandle) {
    super::clear(&app.state::<AppState>().db.lock().unwrap());
    if let Some(window) = app.get_webview_window(window::WHATS_NEW) {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn get_whats_new(app: AppHandle) -> Option<WhatsNew> {
    current_due(&app)
}

#[tauri::command]
pub fn dismiss_whats_new(app: AppHandle) {
    dismiss(&app);
}
