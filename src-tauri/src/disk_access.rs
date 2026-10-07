//! macOS Full Disk Access: dragging original files out of protected folders
//! (Documents, Downloads, …) silently fails without it.

use tauri::AppHandle;

#[cfg(any(target_os = "macos", test))]
use std::path::{Path, PathBuf};

/// Data only an app with Full Disk Access can read. Opening any of them tells
/// us access is on; a denied attempt is what lists an app (switched off) in the
/// Full Disk Access pane, as for other apps. Paths missing on this Mac are skipped
/// (macOS 27 has no per-user TCC.db).
#[cfg(target_os = "macos")]
const PROBES: [&str; 5] = [
    "Library/Safari",
    "Library/Safari/Bookmarks.plist",
    "Library/Mail",
    "Library/Messages/chat.db",
    "Library/Application Support/com.apple.TCC/TCC.db",
];

#[cfg(target_os = "macos")]
fn granted() -> bool {
    let Some(home) = std::env::var_os("HOME") else { return false };
    let home = Path::new(&home);
    // Try every probe (no short-circuit) so a denied run registers all of them.
    PROBES
        .iter()
        .map(|rel| {
            let path = home.join(rel);
            if path.is_dir() { std::fs::read_dir(&path).is_ok() } else { std::fs::File::open(&path).is_ok() }
        })
        .filter(|&opened| opened)
        .count()
        > 0
}

/// The `.app` bundle that contains `exe`, if any.
#[cfg(any(target_os = "macos", test))]
fn app_bundle(exe: &Path) -> Option<PathBuf> {
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

/// Opens the Full Disk Access pane and selects Vee.app in Finder, ready to drag
/// into the list if macOS hasn't listed it.
#[cfg(target_os = "macos")]
fn open_pane() -> Result<(), String> {
    use std::process::Command;
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(bundle) = std::env::current_exe().ok().as_deref().and_then(app_bundle) {
        let _ = Command::new("open").arg("-R").arg(bundle).spawn();
    }
    Ok(())
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
            ("파일을 드래그해서 복사하려면 Vee에 전체 디스크 접근 권한이 필요해요.\n\n시스템 설정의 목록에서 Vee를 켜 주세요. 목록에 없으면 Finder에 보이는 Vee를 목록으로 끌어다 놓으세요.", "시스템 설정 열기", "나중에")
        } else {
            ("Vee needs Full Disk Access to copy files by dragging them.\n\nTurn on Vee in the System Settings list. If it isn't there, drag Vee from Finder into the list.", "Open System Settings", "Later")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_app_bundle_around_the_executable() {
        assert_eq!(app_bundle(Path::new("/Applications/Vee.app/Contents/MacOS/vee-app")), Some(PathBuf::from("/Applications/Vee.app")));
        assert_eq!(app_bundle(Path::new("/Users/me/vee-app/src-tauri/target/debug/vee-app")), None);
    }
}
