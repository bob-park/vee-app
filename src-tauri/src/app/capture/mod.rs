//! Screen capture and recording: the selection overlay, selection geometry, the on-screen window list
//! and the screen grab.

pub mod geometry;
pub mod shot;
pub mod window_list;

use crate::{
    app::{front_app, screen_access, state::AppState, window},
    domain::settings,
    shared::error::{AppError, AppResult},
};
use geometry::{MonitorInfo, Selection};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use window_list::WinInfo;

pub const CAPTURE: &str = "capture";

/// Lets the windows under the overlay repaint without it before the grab.
const HIDE_SETTLE_MS: u64 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Capture,
    #[allow(dead_code)] // opened by `app::recorder` (Task 7)
    Record,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenPayload {
    mode: Mode,
    monitor: MonitorInfo,
    windows: Vec<WinInfo>,
}

/// The monitor the overlay is open on, and the app to hand focus back to.
struct Session {
    monitor: MonitorInfo,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    prev_app: Option<i32>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

/// Opens the overlay on the monitor under the cursor; ignored while it is already up.
pub fn open(app: &AppHandle, mode: Mode) {
    if !screen_access::granted() {
        window::show_settings(app, "permissions");
        return;
    }
    let Some(overlay) = app.get_webview_window(CAPTURE) else { return };
    if overlay.is_visible().unwrap_or(false) {
        return;
    }
    let Some(m) = window::cursor_monitor(app) else { return };
    let scale = m.scale_factor();
    let (pos, size) = (m.position(), m.size());
    let monitor = MonitorInfo {
        x: pos.x as f64 / scale,
        y: pos.y as f64 / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
        scale,
    };
    let windows = window_list::list(&monitor);
    let prev_app = front_app::frontmost().map(|a| a.pid).filter(|&pid| pid != std::process::id() as i32);
    *SESSION.lock().unwrap() = Some(Session { monitor, prev_app });

    let _ = window::place(&overlay, window::rect_from((pos.x, pos.y), (size.width, size.height), scale, window::USES_POINTS));
    raise_above_menu_bar(&overlay);
    let _ = app.emit_to(CAPTURE, "capture://open", OpenPayload { mode, monitor, windows });
    let _ = overlay.show();
    let _ = overlay.set_focus();
}

/// macOS: a normal always-on-top window sits under the menu bar; the screen-saver level covers it.
fn raise_above_menu_bar(overlay: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    match overlay.ns_window() {
        // SAFETY: Tauri hands out the live NSWindow; `open` runs on the main thread (shortcut handler).
        Ok(ptr) => unsafe { (*(ptr as *const objc2_app_kit::NSWindow)).setLevel(objc2_app_kit::NSScreenSaverWindowLevel) },
        Err(e) => log::warn!("couldn't raise the capture overlay: {e}"),
    }
    #[cfg(not(target_os = "macos"))]
    let _ = overlay;
}

/// Hides the overlay and gives focus back to the app that had it; returns the closed session.
fn close(app: &AppHandle) -> Option<Session> {
    if let Some(overlay) = app.get_webview_window(CAPTURE) {
        let _ = overlay.hide();
    }
    let session = SESSION.lock().unwrap().take();
    // Windows hands focus back on hide by itself; `activate` would pick the app's first window, maybe not the right one.
    #[cfg(target_os = "macos")]
    if let Some(pid) = session.as_ref().and_then(|s| s.prev_app) {
        front_app::activate(pid);
    }
    session
}

fn capture(app: &AppHandle, selection: Selection) -> AppResult<()> {
    let session = close(app).ok_or_else(|| AppError::InvalidInput("capture overlay is not open".into()))?;
    std::thread::sleep(Duration::from_millis(HIDE_SETTLE_MS));
    let img = shot::grab(&session.monitor, &selection)?;
    let (_, content) = crate::app::editor::copy_to_history(app, &img)?;
    drop(img);
    let sound = settings::capture_sounds(&app.state::<AppState>().db.lock().unwrap());
    let main = app.clone();
    // The shutter is AppKit on macOS, which belongs on the main thread.
    app.run_on_main_thread(move || {
        if sound {
            crate::app::sound::play_shutter();
        }
        let _ = main.emit("clips://changed", ());
        window::show_toast(&main, window::ToastPayload::copied(&content, false));
    })
    .map_err(|e| AppError::Other(e.to_string()))
}

pub mod commands {
    use super::geometry::Selection;
    use crate::shared::error::{AppError, AppResult};
    use tauri::AppHandle;

    /// Hides the overlay, grabs `selection`, and copies it into the clipboard and history.
    #[tauri::command]
    pub async fn take_capture(app: AppHandle, selection: Selection) -> AppResult<()> {
        let result = tauri::async_runtime::spawn_blocking(move || super::capture(&app, selection))
            .await
            .map_err(|e| AppError::Other(e.to_string()))?;
        if let Err(e) = &result {
            log::warn!("capture failed: {e}");
        }
        result
    }

    #[tauri::command]
    pub fn cancel_capture(app: AppHandle) {
        super::close(&app);
    }
}
