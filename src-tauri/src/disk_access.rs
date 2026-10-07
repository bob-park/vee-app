//! macOS Full Disk Access: dragging original files out of protected folders
//! (Documents, Downloads, …) silently fails without it.

use tauri::AppHandle;

/// Only an app with Full Disk Access can open the TCC database, and every Mac has one.
#[cfg(target_os = "macos")]
fn granted() -> bool {
    let Some(home) = std::env::var_os("HOME") else { return false };
    let db = std::path::Path::new(&home).join("Library/Application Support/com.apple.TCC/TCC.db");
    std::fs::File::open(db).is_ok()
}

#[cfg(target_os = "macos")]
fn open_pane() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `None` where the permission doesn't exist (Windows).
#[tauri::command]
pub fn get_disk_access() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return Some(granted());
    #[cfg(not(target_os = "macos"))]
    None
}

#[tauri::command]
pub fn open_disk_access_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return open_pane();
    #[cfg(not(target_os = "macos"))]
    Err("Full Disk Access is macOS only".into())
}

/// Asks once, ever, when Full Disk Access is missing; the settings row covers later.
pub fn prompt_once(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use crate::{AppState, settings, tray};
        use tauri::Manager;
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
        let store = app.state::<AppState>();
        let store = store.store.lock().unwrap();
        if granted() || store.get_setting("diskAccessPrompted").ok().flatten().is_some() {
            return;
        }
        let _ = store.set_setting("diskAccessPrompted", "1");
        let ko = tray::resolve_locale(&settings::get(&store, "locale")) == "ko";
        drop(store);
        let (message, open, later) = if ko {
            ("파일을 드래그해서 복사하려면 Vee에 전체 디스크 접근 권한이 필요해요.\n\n시스템 설정에서 Vee를 켜 주세요.", "시스템 설정 열기", "나중에")
        } else {
            ("Vee needs Full Disk Access to copy files by dragging them.\n\nTurn on Vee in System Settings.", "Open System Settings", "Later")
        };
        app.dialog()
            .message(message)
            .title("Vee")
            .buttons(MessageDialogButtons::OkCancelCustom(open.into(), later.into()))
            .show(|open| {
                if open && let Err(e) = open_pane() {
                    log::warn!("couldn't open Full Disk Access settings: {e}");
                }
            });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}
