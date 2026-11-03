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
use geometry::{MonitorInfo, Point, RectF, Selection};
use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WebviewWindowBuilder};
use window_list::WinInfo;

pub const CAPTURE: &str = "capture";

/// Lets the windows under the overlay repaint without it before the grab.
const HIDE_SETTLE_MS: u64 = 120;
/// How long the page gets to claim a new session before it is dropped and the overlay stays hidden.
const CLAIM_TIMEOUT_MS: u64 = 1000;
/// How often the open overlays check which monitor the mouse is on.
const FOLLOW_MS: u64 = 30;

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
    /// Where the mouse is, if on this monitor: the window under it is tinted before it moves.
    cursor: Option<Point>,
}

/// One monitor's overlay in a session.
struct Overlay {
    label: String,
    /// Its monitor's physical origin, to find it under the mouse.
    origin: (i32, i32),
    payload: OpenPayload,
    /// Its page took the payload, so it is shown.
    claimed: bool,
}

/// One open of the overlays, one per monitor. An overlay is shown only once its page has claimed it.
struct Session {
    generation: u64,
    overlays: Vec<Overlay>,
    /// The overlay on the cursor's monitor: it takes the keyboard.
    focus: String,
    /// The app to hand focus back to.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    prev_app: Option<i32>,
}

impl Session {
    fn overlay(&self, label: &str) -> Option<&Overlay> {
        self.overlays.iter().find(|o| o.label == label)
    }
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The overlay label for monitor `index`: the prebuilt `capture` first, then `capture-1`, `capture-2`…
fn overlay_label(index: usize) -> String {
    if index == 0 { CAPTURE.to_string() } else { format!("{CAPTURE}-{index}") }
}

/// Page `label` took its overlay: mark it claimed; its payload, and whether it takes the keyboard.
fn claim(slot: &mut Option<Session>, label: &str) -> Option<(OpenPayload, bool)> {
    let session = slot.as_mut()?;
    let focus = session.focus == label;
    let overlay = session.overlays.iter_mut().find(|o| o.label == label)?;
    overlay.claimed = true;
    Some((overlay.payload.clone(), focus))
}

/// The mouse is on the monitor at `origin`: its overlay takes the keyboard. The label to focus, if that changed
/// and its page is up (an unclaimed one is focused when it claims).
fn refocus(session: &mut Session, origin: (i32, i32)) -> Option<String> {
    let overlay = session.overlays.iter().find(|o| o.origin == origin && o.label != session.focus)?;
    let (label, claimed) = (overlay.label.clone(), overlay.claimed);
    session.focus = label.clone();
    claimed.then_some(label)
}

/// Keeps the keyboard on the overlay under the mouse while session `generation` is open. macOS sends no mouse
/// moves to a window that isn't key, so another monitor's overlay would show no tint or cursor until clicked.
fn follow_cursor(app: AppHandle, generation: u64) {
    loop {
        std::thread::sleep(Duration::from_millis(FOLLOW_MS));
        let origin = window::cursor_monitor(&app).map(|m| (m.position().x, m.position().y));
        let label = {
            let mut slot = SESSION.lock().unwrap();
            let Some(session) = slot.as_mut().filter(|s| s.generation == generation) else { return };
            origin.and_then(|origin| refocus(session, origin))
        };
        if let Some(w) = label.and_then(|label| app.get_webview_window(&label)) {
            let _ = w.set_focus();
        }
    }
}

/// Drops session `generation` if no page claimed it; true when dropped.
fn expire(slot: &mut Option<Session>, generation: u64) -> bool {
    let stale = matches!(slot, Some(s) if s.generation == generation && !s.overlays.iter().any(|o| o.claimed));
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

/// The recorded area in monitor-local logical px: the drawn rect, or the picked window's frame.
fn selection_rect(selection: &Selection, windows: &[WinInfo]) -> Option<RectF> {
    match *selection {
        Selection::Rect { x, y, width, height } => Some((x, y, width, height)),
        Selection::Window { id } => windows.iter().find(|w| w.id == id).map(|w| (w.x, w.y, w.width, w.height)),
    }
}

/// The overlay window for monitor `index`, built from the `capture` window's config the first time.
/// Kept afterwards, so the next open is instant.
fn overlay_window(app: &AppHandle, index: usize) -> Option<WebviewWindow> {
    let label = overlay_label(index);
    if let Some(w) = app.get_webview_window(&label) {
        return Some(w);
    }
    let mut config = app.config().app.windows.iter().find(|w| w.label == CAPTURE)?.clone();
    config.label = label;
    let built = WebviewWindowBuilder::from_config(app, &config).and_then(|b| b.build());
    match built {
        Ok(w) => {
            recorder::exclude_window(&w);
            Some(w)
        }
        Err(e) => {
            log::warn!("couldn't build overlay {index}: {e}");
            None
        }
    }
}

/// Opens an overlay on every monitor; ignored while a session is open. False when not opened.
/// Each overlay is placed now but shown only when its page claims it (`get_capture_session`).
pub fn open(app: &AppHandle, mode: Mode) -> bool {
    if !screen_access::granted() {
        window::show_settings(app, "permissions");
        return false;
    }
    if SESSION.lock().unwrap().is_some() {
        return false;
    }
    let Ok(monitors) = app.available_monitors() else { return false };
    let cursor = window::cursor_monitor(app).map(|m| *m.position());
    let pointer = window::cursor_at(app);
    let mut overlays = Vec::new();
    let mut focus = None;
    for (index, m) in monitors.iter().enumerate() {
        let Some(overlay) = overlay_window(app, index) else { continue };
        let scale = m.scale_factor();
        let (pos, size) = (m.position(), m.size());
        let monitor = MonitorInfo {
            x: pos.x as f64 / scale,
            y: pos.y as f64 / scale,
            width: size.width as f64 / scale,
            height: size.height as f64 / scale,
            scale,
        };
        let _ = window::place(&overlay, window::rect_from((pos.x, pos.y), (size.width, size.height), scale, window::USES_POINTS));
        raise_above_menu_bar(&overlay);
        let label = overlay.label().to_string();
        if focus.is_none() || cursor == Some(*pos) {
            focus = Some(label.clone());
        }
        let at = pointer.and_then(|p| geometry::local_cursor(p, &monitor, window::USES_POINTS));
        let payload = OpenPayload { mode, monitor, windows: window_list::list(&monitor), cursor: at };
        overlays.push(Overlay { label, origin: (pos.x, pos.y), payload, claimed: false });
    }
    // A monitor that went away leaves its overlay behind: drop it.
    for index in monitors.len().max(1).. {
        let Some(w) = app.get_webview_window(&overlay_label(index)) else { break };
        let _ = w.destroy();
    }
    let Some(focus) = focus else { return false };
    let prev_app = front_app::frontmost().map(|a| a.pid).filter(|&pid| pid != std::process::id() as i32);
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let labels: Vec<String> = overlays.iter().map(|o| o.label.clone()).collect();
    *SESSION.lock().unwrap() = Some(Session { generation, overlays, focus, prev_app });
    for label in &labels {
        let _ = app.emit_to(label.as_str(), "capture://open", ());
    }
    if labels.len() > 1 {
        let app = app.clone();
        std::thread::spawn(move || follow_cursor(app, generation));
    }
    // Safety net: pages that never answer (not loaded, JS error) must not leave the next shortcut ignored.
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
fn raise_above_menu_bar(overlay: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    match overlay.ns_window() {
        // SAFETY: Tauri hands out the live NSWindow; `open` runs on the main thread (shortcut handler).
        Ok(ptr) => unsafe { (*(ptr as *const objc2_app_kit::NSWindow)).setLevel(objc2_app_kit::NSScreenSaverWindowLevel) },
        Err(e) => log::warn!("couldn't raise the capture overlay: {e}"),
    }
    #[cfg(not(target_os = "macos"))]
    let _ = overlay;
}

/// Hides every overlay, clears their pages and gives focus back to the app that had it; returns the closed session.
fn close(app: &AppHandle) -> Option<Session> {
    let session = SESSION.lock().unwrap().take();
    for overlay in session.iter().flat_map(|s| &s.overlays) {
        if let Some(w) = app.get_webview_window(&overlay.label) {
            let _ = w.hide();
        }
        // So an overlay never shows this session's drawing when it is shown again.
        let _ = app.emit_to(overlay.label.as_str(), "capture://closed", ());
    }
    // Windows hands focus back on hide by itself; `activate` would pick the app's first window, maybe not the right one.
    #[cfg(target_os = "macos")]
    if let Some(pid) = session.as_ref().and_then(|s| s.prev_app) {
        front_app::activate(pid);
    }
    session
}

/// Hides the overlays: the recorder, once its frame is up.
pub(crate) fn close_overlays(app: &AppHandle) {
    close(app);
}

/// The monitor of record overlay `label` and the area `selection` covers on it. The overlay stays up: the
/// recorder closes it (`close_overlays`) once its own frame is on screen, so nothing blinks in between.
pub(crate) fn record_target(label: &str, selection: &Selection) -> Option<(MonitorInfo, Option<RectF>)> {
    let slot = SESSION.lock().unwrap();
    let overlay = slot.as_ref()?.overlay(label).filter(|o| o.claimed && o.payload.mode == Mode::Record)?;
    Some((overlay.payload.monitor, selection_rect(selection, &overlay.payload.windows)))
}

fn capture(app: &AppHandle, label: &str, selection: Selection) -> AppResult<()> {
    let session = close(app).ok_or_else(|| AppError::InvalidInput("capture overlay is not open".into()))?;
    let overlay =
        session.overlay(label).filter(|o| o.claimed).ok_or_else(|| AppError::InvalidInput("capture overlay is not open".into()))?;
    let sound = settings::capture_sounds(&app.state::<AppState>().db.lock().unwrap());
    // The shutter sounds as the overlay goes, like the system's; AppKit, so on the main thread.
    if sound {
        let _ = app.run_on_main_thread(crate::app::sound::play_shutter);
    }
    let source = source_pid(&selection, &overlay.payload.windows).and_then(|pid| front_app::by_pid(pid as i32));
    // A window grab takes the window alone, so the overlay going away doesn't matter.
    if matches!(selection, Selection::Rect { .. }) {
        std::thread::sleep(Duration::from_millis(HIDE_SETTLE_MS));
    }
    let img = shot::grab(&overlay.payload.monitor, &selection)?;
    let (_, content) = crate::app::editor::copy_to_history(app, &img, source.as_ref())?;
    drop(img);
    let main = app.clone();
    app.run_on_main_thread(move || {
        let _ = main.emit("clips://changed", ());
        window::show_toast(&main, window::ToastPayload::copied(&content, false));
    })
    .map_err(|e| AppError::Other(e.to_string()))
}

pub mod commands {
    use super::{OpenPayload, geometry::Selection};
    use crate::shared::error::{AppError, AppResult};
    use tauri::{AppHandle, WebviewWindow};

    /// The page of overlay `window` claims its part of the open session (on mount and on every
    /// `capture://open`); only then is it shown. The one on the cursor's monitor also takes focus.
    /// `None` when nothing is open.
    #[tauri::command]
    pub fn get_capture_session(window: WebviewWindow) -> Option<OpenPayload> {
        let (payload, focus) = super::claim(&mut super::SESSION.lock().unwrap(), window.label())?;
        let _ = window.show();
        if focus {
            let _ = window.set_focus();
        }
        Some(payload)
    }

    /// Hides the overlay, grabs `selection`, and copies it into the clipboard and history.
    #[tauri::command]
    pub async fn take_capture(app: AppHandle, window: WebviewWindow, selection: Selection) -> AppResult<()> {
        let label = window.label().to_string();
        let result = tauri::async_runtime::spawn_blocking(move || super::capture(&app, &label, selection))
            .await
            .map_err(|e| AppError::Other(e.to_string()))?;
        if let Err(e) = &result {
            log::warn!("capture failed: {e}");
        }
        result
    }

    #[tauri::command]
    pub fn cancel_capture(app: AppHandle) {
        if super::close(&app).is_some_and(|s| s.overlays.first().is_some_and(|o| o.payload.mode == super::Mode::Record)) {
            crate::app::recorder::overlay_closed(&app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(generation: u64) -> Option<Session> {
        let monitor = MonitorInfo { x: 0.0, y: 0.0, width: 100.0, height: 100.0, scale: 1.0 };
        let overlay = |label: &str| Overlay {
            label: label.into(),
            origin: if label == "capture" { (0, 0) } else { (1920, 0) },
            payload: OpenPayload { mode: Mode::Capture, monitor, windows: Vec::new(), cursor: None },
            claimed: false,
        };
        Some(Session { generation, overlays: vec![overlay("capture"), overlay("capture-1")], focus: "capture-1".into(), prev_app: None })
    }

    #[test]
    fn unclaimed_session_expires_claimed_one_stays() {
        let mut slot = session(1);
        assert!(!expire(&mut slot, 0), "an older timer leaves a newer session alone");
        assert!(expire(&mut slot, 1));
        assert!(slot.is_none());
        assert!(claim(&mut slot, "capture").is_none());

        let mut slot = session(2);
        assert!(claim(&mut slot, "capture-9").is_none(), "an overlay outside the session gets nothing");
        assert_eq!(claim(&mut slot, "capture").map(|(p, focus)| (p.mode, focus)), Some((Mode::Capture, false)));
        assert!(!expire(&mut slot, 2), "one claimed overlay keeps the session");
        assert_eq!(claim(&mut slot, "capture-1").map(|(_, focus)| focus), Some(true), "the cursor's overlay takes focus");
        assert!(slot.is_some_and(|s| s.overlays.iter().all(|o| o.claimed)));
    }

    #[test]
    fn the_overlay_under_the_mouse_takes_the_keyboard() {
        let mut session = session(1).unwrap();
        assert_eq!(refocus(&mut session, (1920, 0)), None, "already focused");
        assert_eq!(refocus(&mut session, (0, 0)), None, "not claimed yet: focused when it claims");
        assert_eq!(session.focus, "capture");
        session.overlays[1].claimed = true;
        assert_eq!(refocus(&mut session, (1920, 0)).as_deref(), Some("capture-1"));
        assert_eq!(refocus(&mut session, (5000, 0)), None, "no overlay there");
        assert_eq!(session.focus, "capture-1");
    }

    #[test]
    fn overlays_are_labelled_by_monitor() {
        assert_eq!(overlay_label(0), "capture");
        assert_eq!(overlay_label(2), "capture-2");
    }

    #[test]
    fn window_capture_credits_the_window_owner() {
        let win = WinInfo { id: 7, pid: 42, app: "Safari".into(), x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        let windows = [win];
        assert_eq!(source_pid(&Selection::Window { id: 7 }, &windows), Some(42));
        assert_eq!(source_pid(&Selection::Window { id: 8 }, &windows), None);
        assert_eq!(
            selection_rect(&Selection::Window { id: 7 }, &windows),
            Some((windows[0].x, windows[0].y, windows[0].width, windows[0].height))
        );
        assert_eq!(selection_rect(&Selection::Window { id: 8 }, &windows), None);
        let drawn = Selection::Rect { x: 1.0, y: 2.0, width: 3.0, height: 4.0 };
        assert_eq!(selection_rect(&drawn, &windows), Some((1.0, 2.0, 3.0, 4.0)));
        assert_eq!(source_pid(&Selection::Rect { x: 0.0, y: 0.0, width: 5.0, height: 5.0 }, &windows), None);
    }
}
