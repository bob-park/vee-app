//! Keeps the notes of a downloaded update and shows them once after it's installed.

use crate::{AppState, store::Store, windows};
use serde::Serialize;
use tauri::{AppHandle, Manager};

// Not in `settings::validate`: only the backend writes these.
const VERSION_KEY: &str = "whatsNewVersion";
const NOTES_KEY: &str = "whatsNewNotes";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsNew {
    version: String,
    notes: String,
}

/// Remembers the notes of a downloaded update; blank or missing notes are skipped.
pub fn save(store: &Store, version: &str, notes: Option<&str>) {
    let Some(notes) = notes.filter(|n| !n.trim().is_empty()) else { return };
    if let Err(e) = store.set_setting(VERSION_KEY, version).and_then(|_| store.set_setting(NOTES_KEY, notes)) {
        log::warn!("failed to save release notes: {e}");
    }
}

/// The saved notes, once the update they belong to is the running version.
pub fn due(store: &Store, current: &str) -> Option<WhatsNew> {
    let get = |key| store.get_setting(key).ok().flatten().unwrap_or_default();
    let (version, notes) = (get(VERSION_KEY), get(NOTES_KEY));
    (version == current && !notes.trim().is_empty()).then_some(WhatsNew { version, notes })
}

fn current_due(app: &AppHandle) -> Option<WhatsNew> {
    due(&app.state::<AppState>().store.lock().unwrap(), &app.package_info().version.to_string())
}

/// Called at startup, after the windows exist.
pub fn show_if_due(app: &AppHandle) {
    if current_due(app).is_none() {
        return;
    }
    if let Some(window) = app.get_webview_window(windows::WHATS_NEW) {
        let _ = window.show();
        // An Accessory app isn't active at launch; without focus the window opens behind others.
        let _ = window.set_focus();
    }
}

/// Forgets the notes so they're shown only once, and hides the window.
pub fn dismiss(app: &AppHandle) {
    {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        for key in [VERSION_KEY, NOTES_KEY] {
            if let Err(e) = store.set_setting(key, "") {
                log::warn!("failed to clear {key}: {e}");
            }
        }
    }
    if let Some(window) = app.get_webview_window(windows::WHATS_NEW) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    #[test]
    fn shows_notes_saved_for_the_running_version_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert!(due(&store, "0.9.0").is_none()); // fresh install
        save(&store, "0.9.0", Some("## 새 기능\n- a"));
        assert!(due(&store, "0.8.0").is_none()); // not installed yet
        let w = due(&store, "0.9.0").unwrap();
        assert_eq!((w.version.as_str(), w.notes.as_str()), ("0.9.0", "## 새 기능\n- a"));
    }

    #[test]
    fn ignores_updates_without_notes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        save(&store, "0.9.0", None);
        save(&store, "0.9.0", Some("   "));
        assert!(due(&store, "0.9.0").is_none());
    }
}
