//! Screen capture and recording: the selection overlay, selection geometry, the on-screen window list
//! and the screen grab.

pub mod geometry;
pub mod shot;
pub mod window_list;

use crate::{
    app::{front_app, recorder, screen_access, state::AppState, window},
    domain::settings,
    shared::error::{AppError, AppResult},
};
use geometry::{MonitorInfo, Selection};
use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use window_list::WinInfo;

pub const CAPTURE: &str = "capture";

/// Lets the windows under the overlay repaint without it before the grab.
const HIDE_SETTLE_MS: u64 = 120;
/// How long the page gets to claim a new session before it is dropped and the overlay stays hidden.
const CLAIM_TIMEOUT_MS: u64 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Capture,
    Record,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPayload {
    mode: Mode,
    monitor: MonitorInfo,
    windows: Vec<WinInfo>,
}

/// One open of the overlay. It is shown only once the page has claimed it (`ready`).
struct Session {
    generation: u64,
    payload: OpenPayload,
    ready: bool,
    /// The app to hand focus back to.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    prev_app: Option<i32>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The page took the session: mark it ready and hand over its payload.
fn claim(slot: &mut Option<Session>) -> Option<OpenPayload> {
    let session = slot.as_mut()?;
    session.ready = true;
    Some(session.payload.clone())
}

/// Drops session `generation` if the page never claimed it; true when dropped.
fn expire(slot: &mut Option<Session>, generation: u64) -> bool {
    let stale = matches!(slot, Some(s) if s.generation == generation && !s.ready);
    if stale {
        *slot = None;
    }
    stale
}

/// The app credited with a capture: the picked window's owner, or None (Vee) for a region.
fn source_pid(selection: &Selection, windows: &[WinInfo]) -> Option<u32> {
    match selection {
        Selection::Rect { .. } => None,
        Selection::Window { id } => windows.iter().find(|w| w.id == *id).map(|w| w.pid),
    }
}

/// Opens the overlay on the monitor under the cursor; ignored while a session is open. False when not opened.
/// The window is placed now but shown only when the page claims the session (`get_capture_session`).
pub fn open(app: &AppHandle, mode: Mode) -> bool {
    if !screen_access::granted() {
        window::show_settings(app, "permissions");
        return false;
    }
    let Some(overlay) = app.get_webview_window(CAPTURE) else { return false };
    if SESSION.lock().unwrap().is_some() {
        return false;
    }
    let Some(m) = window::cursor_monitor(app) else { return false };
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
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let payload = OpenPayload { mode, monitor, windows };
    *SESSION.lock().unwrap() = Some(Session { generation, payload, ready: false, prev_app });

    let _ = window::place(&overlay, window::rect_from((pos.x, pos.y), (size.width, size.height), scale, window::USES_POINTS));
    raise_above_menu_bar(&overlay);
    let _ = app.emit_to(CAPTURE, "capture://open", ());
    // Safety net: a page that never answers (not loaded, JS error) must not leave the next shortcut ignored.
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(CLAIM_TIMEOUT_MS));
        if expire(&mut SESSION.lock().unwrap(), generation) {
            log::warn!("capture overlay didn't answer; dropped the session");
            if mode == Mode::Record {
                recorder::overlay_closed(&app);
            }
        }
    });
    true
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

/// Closes a claimed record session for the recording to start; its monitor.
pub(crate) fn take_record(app: &AppHandle) -> Option<MonitorInfo> {
    close(app).filter(|s| s.ready && s.payload.mode == Mode::Record).map(|s| s.payload.monitor)
}

fn capture(app: &AppHandle, selection: Selection) -> AppResult<()> {
    let session = close(app).filter(|s| s.ready).ok_or_else(|| AppError::InvalidInput("capture overlay is not open".into()))?;
    let source = source_pid(&selection, &session.payload.windows).and_then(|pid| front_app::by_pid(pid as i32));
    std::thread::sleep(Duration::from_millis(HIDE_SETTLE_MS));
    let img = shot::grab(&session.payload.monitor, &selection)?;
    let (_, content) = crate::app::editor::copy_to_history(app, &img, source.as_ref())?;
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
    use super::{OpenPayload, geometry::Selection};
    use crate::shared::error::{AppError, AppResult};
    use tauri::{AppHandle, Manager};

    /// The page claims the open session (on mount and on every `capture://open`); only then is the overlay
    /// shown and focused. `None` when nothing is open.
    #[tauri::command]
    pub fn get_capture_session(app: AppHandle) -> Option<OpenPayload> {
        let payload = super::claim(&mut super::SESSION.lock().unwrap())?;
        if let Some(overlay) = app.get_webview_window(super::CAPTURE) {
            let _ = overlay.show();
            let _ = overlay.set_focus();
        }
        Some(payload)
    }

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
        if super::close(&app).is_some_and(|s| s.payload.mode == super::Mode::Record) {
            crate::app::recorder::overlay_closed(&app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(generation: u64) -> Option<Session> {
        let monitor = MonitorInfo { x: 0.0, y: 0.0, width: 100.0, height: 100.0, scale: 1.0 };
        let payload = OpenPayload { mode: Mode::Capture, monitor, windows: Vec::new() };
        Some(Session { generation, payload, ready: false, prev_app: None })
    }

    #[test]
    fn unclaimed_session_expires_claimed_one_stays() {
        let mut slot = session(1);
        assert!(!expire(&mut slot, 0), "an older timer leaves a newer session alone");
        assert!(expire(&mut slot, 1));
        assert!(slot.is_none());
        assert!(claim(&mut slot).is_none());

        let mut slot = session(2);
        assert_eq!(claim(&mut slot).map(|p| p.mode), Some(Mode::Capture));
        assert!(!expire(&mut slot, 2), "a claimed session is never dropped by the timer");
        assert!(slot.is_some_and(|s| s.ready));
    }

    #[test]
    fn window_capture_credits_the_window_owner() {
        let win = WinInfo { id: 7, pid: 42, app: "Safari".into(), x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        let windows = [win];
        assert_eq!(source_pid(&Selection::Window { id: 7 }, &windows), Some(42));
        assert_eq!(source_pid(&Selection::Window { id: 8 }, &windows), None);
        assert_eq!(source_pid(&Selection::Rect { x: 0.0, y: 0.0, width: 5.0, height: 5.0 }, &windows), None);
    }
}
