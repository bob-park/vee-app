//! Panel, toast and settings window behaviour, plus copying a clip back.

use crate::{
    app::{
        convert::{self, ConvertNote},
        front_app,
        state::{AppState, now_ms},
    },
    domain::{
        clip::{self, ClipContent, Kind, classify_text},
        settings,
    },
    shared::error::{AppError, AppResult},
};
use drag::{DragItem, DragMode, DragResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const PANEL: &str = "panel";
pub const TOAST: &str = "toast";
pub const SETTINGS: &str = "settings";
pub const WHATS_NEW: &str = "whats-new";

const PANEL_RATIO: f64 = 0.30;
/// Default side-panel width as a share of the work-area width.
const PANEL_WIDTH_RATIO: f64 = 0.25;
/// Narrowest a side panel may be, in design points, so the toolbar still fits.
const PANEL_MIN_WIDTH: f64 = 340.0;
/// The least of the work area a resized panel may take.
const PANEL_MIN_SHARE: f64 = 0.15;
/// The most of the work area a resized panel may take.
const PANEL_MAX_SHARE: f64 = 0.5;
const PANEL_MARGIN: f64 = 8.0;
/// Window size; the card inside is smaller and the rest is room for its shadow.
const TOAST_WIDTH: f64 = 420.0;
const TOAST_HEIGHT: f64 = 132.0;
const TOAST_BOTTOM: f64 = 14.0;
const TOAST_MS: u64 = 1500;
/// Time the toast's exit animation gets before the window hides.
const TOAST_EXIT_MS: u64 = 180;
/// How long the panel slides down before its window hides; matches `.panel.closing` in panel.css.
const PANEL_CLOSE_MS: u64 = 200;
/// Reveal the panel anyway if the webview hasn't confirmed its parked frame by then.
const PANEL_REVEAL_FALLBACK_MS: u64 = 200;
const TOAST_PREVIEW_CHARS: usize = 40;

static TOAST_GENERATION: AtomicU64 = AtomicU64::new(0);
/// Bumped to cancel a pending panel hide.
static PANEL_GENERATION: AtomicU64 = AtomicU64::new(0);
/// The panel is still visible but sliding down to hide.
static PANEL_CLOSING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToastPayload {
    pub ok: bool,
    /// What was copied, for the toast's icon.
    pub kind: Kind,
    pub text: Option<String>,
    pub files: usize,
    /// Copied as plain text, without the source app's formatting.
    pub plain: bool,
    pub convert: Option<ConvertNote>,
    /// Set when warning that pinned cards hide a side panel's history: how many fit with history below.
    pub pins_fit: Option<usize>,
}

impl ToastPayload {
    fn copied(content: &ClipContent, plain: bool) -> Self {
        match content {
            ClipContent::Text(t) => {
                Self { ok: true, kind: classify_text(t), text: Some(toast_preview(t)), files: 0, plain, convert: None, pins_fit: None }
            }
            ClipContent::Image(_) => Self { ok: true, kind: Kind::Image, text: None, files: 0, plain: false, convert: None, pins_fit: None },
            ClipContent::Files(f) => {
                Self { ok: true, kind: Kind::Files, text: None, files: f.len(), plain: false, convert: None, pins_fit: None }
            }
        }
    }

    fn failed() -> Self {
        Self { ok: false, kind: Kind::Text, text: None, files: 0, plain: false, convert: None, pins_fit: None }
    }

    /// A convert note; `ok` only for a saved file.
    pub(crate) fn convert(note: ConvertNote, kind: Kind) -> Self {
        let ok = matches!(note, ConvertNote::Saved { .. });
        Self { ok, kind, text: None, files: 0, plain: false, convert: Some(note), pins_fit: None }
    }

    /// The side-panel warning: more cards are pinned than fit above the history.
    pub(crate) fn pins_fit(n: usize) -> Self {
        Self { ok: false, kind: Kind::Text, text: None, files: 0, plain: false, convert: None, pins_fit: Some(n) }
    }
}

fn toast_preview(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= TOAST_PREVIEW_CHARS {
        return flat;
    }
    format!("{}…", flat.chars().take(TOAST_PREVIEW_CHARS).collect::<String>())
}

/// macOS positions windows in points in one global space across displays;
/// Windows positions them in physical pixels of the virtual desktop.
pub(crate) const USES_POINTS: bool = cfg!(target_os = "macos");

/// A rectangle in the space windows are positioned in (see `USES_POINTS`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub(crate) fn contains(&self, (x, y): (f64, f64)) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// tao reports macOS monitor geometry as points × that monitor's own scale.
pub(crate) fn rect_from((x, y): (i32, i32), (w, h): (u32, u32), scale: f64, points: bool) -> Rect {
    let k = if points { scale } else { 1.0 };
    Rect { x: x as f64 / k, y: y as f64 / k, w: w as f64 / k, h: h as f64 / k }
}

/// tao's macOS cursor is points × the *primary* display's scale, whichever display it is on,
/// so `monitor_from_point` misses displays with a different scale.
pub(crate) fn cursor_point((x, y): (f64, f64), primary_scale: f64, points: bool) -> (f64, f64) {
    if points { (x / primary_scale, y / primary_scale) } else { (x, y) }
}

fn pick(point: (f64, f64), rects: &[Rect]) -> Option<usize> {
    rects.iter().position(|r| r.contains(point))
}

/// Screen edge the panel docks to, from the `panelPosition` setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PanelPosition {
    Bottom,
    Top,
    Left,
    Right,
}

impl PanelPosition {
    /// Unknown values fall back to the default, `Bottom`.
    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "top" => Self::Top,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => Self::Bottom,
        }
    }

    /// Left and right: sized by width, cards stacked vertically.
    pub(crate) fn side(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Top/bottom share the saved height, left/right the saved width.
    pub(crate) fn ratio_key(self) -> &'static str {
        if self.side() { "panelWidthRatio" } else { "panelHeightRatio" }
    }
}

/// Default panel height in design points: a share of the work area.
fn panel_height(work_h_points: f64) -> f64 {
    (work_h_points * PANEL_RATIO).round()
}

/// Shortest a resized panel may be.
fn panel_min(work_h: f64) -> f64 {
    (work_h * PANEL_MIN_SHARE).ceil()
}

/// Tallest a resized panel may be.
fn panel_max(work_h: f64) -> f64 {
    (work_h * PANEL_MAX_SHARE).floor()
}

/// A resized size along `pos`'s axis, rounded and kept between its floor and `panel_max`.
/// `base` is the work-area height (top/bottom) or width (sides) it is measured against.
fn clamp_panel_extent(pos: PanelPosition, base: f64, v: f64) -> f64 {
    let max = panel_max(base);
    let min = if pos.side() { panel_min(base).max(PANEL_MIN_WIDTH).min(max) } else { panel_min(base) };
    v.round().clamp(min, max)
}

/// Panel size along `pos`'s axis from the saved ratio of `base`, the primary display's work area,
/// or the default without one. Capped at half of `work`, the work area the panel opens on.
fn panel_extent_for(pos: PanelPosition, base: f64, work: f64, ratio: Option<f64>) -> f64 {
    let default = if pos.side() { (base * PANEL_WIDTH_RATIO).round() } else { panel_height(base) };
    let v = clamp_panel_extent(pos, base, ratio.map_or(default, |r| r * base));
    v.min(panel_max(work))
}

/// Work-area length along `pos`'s axis, in design points: height for top/bottom, width for the sides.
fn axis(work: Rect, unit: f64, pos: PanelPosition) -> f64 {
    if pos.side() { work.w / unit } else { work.h / unit }
}

/// Work-area length along `pos`'s axis that panel sizes are measured against: the primary display's,
/// so the panel is the same size on every monitor. `fallback` when there is no primary display.
fn base_extent(app: &AppHandle, pos: PanelPosition, fallback: f64) -> f64 {
    app.primary_monitor().ok().flatten().map_or(fallback, |m| {
        let (work, unit) = work_area(&m);
        axis(work, unit, pos)
    })
}

/// The saved `panelPosition`.
fn stored_position(app: &AppHandle) -> PanelPosition {
    PanelPosition::parse(&settings::get(&app.state::<AppState>().db.lock().unwrap(), "panelPosition"))
}

/// The saved ratio for `pos`'s axis; `None` when unset or unreadable.
fn stored_ratio(app: &AppHandle, pos: PanelPosition) -> Option<f64> {
    settings::get(&app.state::<AppState>().db.lock().unwrap(), pos.ratio_key()).parse().ok().filter(|r: &f64| r.is_finite())
}

/// Where the panel goes on a monitor's work area, at its saved position and size (`ratio`, or the default with `None`).
fn panel_frame(app: &AppHandle, work: Rect, unit: f64, pos: PanelPosition, ratio: Option<f64>) -> Rect {
    let here = axis(work, unit, pos);
    panel_rect(work, unit, pos, panel_extent_for(pos, base_extent(app, pos, here), here, ratio))
}

/// `unit` is how many positioning units one design point takes: 1 on macOS, the scale on Windows.
/// `extent` is the panel's height (top/bottom) or width (sides) in design points.
fn panel_rect(work: Rect, unit: f64, pos: PanelPosition, extent: f64) -> Rect {
    let (m, e) = (PANEL_MARGIN * unit, extent * unit);
    let (full_w, full_h) = (work.w - 2.0 * m, work.h - 2.0 * m);
    match pos {
        PanelPosition::Bottom => Rect { x: work.x + m, y: work.y + work.h - e - m, w: full_w, h: e },
        PanelPosition::Top => Rect { x: work.x + m, y: work.y + m, w: full_w, h: e },
        PanelPosition::Left => Rect { x: work.x + m, y: work.y + m, w: e, h: full_h },
        PanelPosition::Right => Rect { x: work.x + work.w - e - m, y: work.y + m, w: e, h: full_h },
    }
}

/// `pinned` is a pinned panel's position and live size in design points, which the toast keeps clear of:
/// above a bottom panel, beside a side panel, unmoved under a top one.
fn toast_rect(work: Rect, unit: f64, pinned: Option<(PanelPosition, f64)>) -> Rect {
    let (w, h) = (TOAST_WIDTH * unit, TOAST_HEIGHT * unit);
    let taken = pinned.map_or(0.0, |(_, e)| (e + PANEL_MARGIN) * unit);
    let (lift, left, free_w) = match pinned.map(|(p, _)| p) {
        Some(PanelPosition::Bottom) => (taken, 0.0, work.w),
        Some(PanelPosition::Left) => (0.0, taken, work.w - taken),
        Some(PanelPosition::Right) => (0.0, 0.0, work.w - taken),
        Some(PanelPosition::Top) | None => (0.0, 0.0, work.w),
    };
    // Centred in the free part; when that is narrower than the toast, kept on screen against the far edge.
    let x = (work.x + left + (free_w - w) / 2.0).min(work.x + work.w - w).max(work.x);
    Rect { x, y: work.y + work.h - h - TOAST_BOTTOM * unit - lift, w, h }
}

/// The live cursor in positioning space.
pub(crate) fn cursor_at(app: &AppHandle) -> Option<(f64, f64)> {
    let cursor = app.cursor_position().ok()?;
    let primary_scale = app.primary_monitor().ok().flatten().map_or(1.0, |m| m.scale_factor());
    Some(cursor_point((cursor.x, cursor.y), primary_scale, USES_POINTS))
}

pub(crate) fn cursor_monitor(app: &AppHandle) -> Option<Monitor> {
    let primary = app.primary_monitor().ok().flatten();
    let found = (|| {
        let monitors = app.available_monitors().ok()?;
        let rects: Vec<Rect> = monitors
            .iter()
            .map(|m| rect_from((m.position().x, m.position().y), (m.size().width, m.size().height), m.scale_factor(), USES_POINTS))
            .collect();
        let index = pick(cursor_at(app)?, &rects)?;
        monitors.into_iter().nth(index)
    })();
    found.or(primary)
}

/// Work area of `monitor` in positioning space, plus the size of one design point in it.
pub(crate) fn work_area(monitor: &Monitor) -> (Rect, f64) {
    let a = monitor.work_area();
    let rect = rect_from((a.position.x, a.position.y), (a.size.width, a.size.height), monitor.scale_factor(), USES_POINTS);
    (rect, if USES_POINTS { 1.0 } else { monitor.scale_factor() })
}

pub(crate) fn place(window: &WebviewWindow, r: Rect) -> tauri::Result<()> {
    if USES_POINTS {
        window.set_position(LogicalPosition::new(r.x, r.y))?;
        window.set_size(LogicalSize::new(r.w, r.h))?;
    } else {
        window.set_position(PhysicalPosition::new(r.x.round() as i32, r.y.round() as i32))?;
        window.set_size(PhysicalSize::new(r.w.round() as u32, r.h.round() as u32))?;
    }
    Ok(())
}

pub(crate) fn pinned(app: &AppHandle) -> bool {
    app.state::<AppState>().panel_pinned.load(Ordering::SeqCst)
}

/// Records the frontmost app to hand focus back to, unless it is Vee itself (e.g. settings).
pub fn remember_front_app(app: &AppHandle) {
    if let Some(front) = front_app::frontmost().filter(|a| a.pid != std::process::id() as i32) {
        *app.state::<AppState>().prev_app_pid.lock().unwrap() = Some(front.pid);
    }
}

/// A pinned panel never toggles closed; `show_panel` brings it to the front instead.
pub fn toggle_panel(app: &AppHandle) {
    let visible = app.get_webview_window(PANEL).and_then(|p| p.is_visible().ok()).unwrap_or(false);
    if visible && !PANEL_CLOSING.load(Ordering::SeqCst) && !pinned(app) { hide_panel(app, true) } else { show_panel(app) }
}

pub fn show_panel(app: &AppHandle) {
    let Some(panel) = app.get_webview_window(PANEL) else { return };
    if panel.is_visible().unwrap_or(false) {
        if PANEL_CLOSING.swap(false, Ordering::SeqCst) {
            // Reopened mid-close: cancel the pending hide and slide back up.
            PANEL_GENERATION.fetch_add(1, Ordering::SeqCst);
            *app.state::<AppState>().prev_app_pid.lock().unwrap() = front_app::frontmost().map(|a| a.pid);
            let _ = app.emit_to(PANEL, "panel://opened", ());
        } else if pinned(app) {
            // Treated like a fresh open, so the panel resets its search and selection.
            if panel.is_focused().unwrap_or(false) {
                return;
            }
            remember_front_app(app);
            let _ = panel.set_focus();
            let _ = app.emit_to(PANEL, "panel://refocused", ());
            return;
        }
        let _ = panel.set_focus();
        return;
    }
    *app.state::<AppState>().prev_app_pid.lock().unwrap() = front_app::frontmost().map(|a| a.pid);
    if let Err(e) = place_and_show(app, &panel) {
        log::error!("failed to show panel: {e}");
    }
}

fn place_and_show(app: &AppHandle, panel: &WebviewWindow) -> tauri::Result<()> {
    let _ = end_drag(app);
    if let Some(monitor) = cursor_monitor(app) {
        let (work, unit) = work_area(&monitor);
        let pos = stored_position(app);
        place(panel, panel_frame(app, work, unit, pos, stored_ratio(app, pos)))?;
    }
    set_panel_alpha(panel, 0.0);
    panel.show()?;
    // Never leave a shown, focused panel invisible; revealing twice is harmless.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(PANEL_REVEAL_FALLBACK_MS));
        let main = handle.clone();
        let _ = handle.run_on_main_thread(move || reveal_panel(&main));
    });
    panel.set_focus()?;
    app.emit_to(PANEL, "panel://opened", ())?;
    Ok(())
}

/// macOS only: window opacity, used to hide the stale frame a hidden webview shows on `show()`.
fn set_panel_alpha(panel: &WebviewWindow, alpha: f64) {
    #[cfg(target_os = "macos")]
    match panel.ns_window() {
        // SAFETY: Tauri hands out the live NSWindow; callers run on the main thread.
        Ok(ptr) => unsafe { (*(ptr as *const objc2_app_kit::NSWindow)).setAlphaValue(alpha) },
        Err(e) => log::warn!("couldn't set panel alpha: {e}"),
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (panel, alpha);
}

/// Called by the panel once its parked frame has been painted.
pub fn reveal_panel(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(PANEL) {
        set_panel_alpha(&panel, 1.0);
    }
}

/// Slides the panel down, then hides its window.
/// `restore_focus` re-activates the app that was in front before the panel opened.
/// Pass `false` when the user already clicked into another app.
/// A pinned panel stays up and only hands focus back, keeping `prev_app_pid` for the next copy.
pub fn hide_panel(app: &AppHandle, restore_focus: bool) {
    let pinned = pinned(app);
    let state = app.state::<AppState>();
    let previous = {
        let mut slot = state.prev_app_pid.lock().unwrap();
        if pinned { *slot } else { slot.take() }
    };
    // Pinned, the user may have switched apps since the panel last lost focus; ask who they were in.
    let previous = if pinned && restore_focus { front_app::menu_bar_owner_pid().or(previous) } else { previous };
    if let (true, Some(pid)) = (restore_focus, previous) {
        front_app::activate(pid);
    }
    if pinned {
        return;
    }
    let Some(panel) = app.get_webview_window(PANEL) else { return };
    if !panel.is_visible().unwrap_or(false) || PANEL_CLOSING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = app.emit_to(PANEL, "panel://closing", ());
    let generation = PANEL_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(PANEL_CLOSE_MS));
        let main = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            // A reopen in the meantime cancelled this hide.
            if PANEL_GENERATION.load(Ordering::SeqCst) != generation || !PANEL_CLOSING.swap(false, Ordering::SeqCst) {
                return;
            }
            if let Some(panel) = main.get_webview_window(PANEL) {
                let _ = panel.hide();
                let _ = main.emit_to(PANEL, "panel://closed", ());
            }
        });
    });
}

/// Resizes the panel on its own monitor, keeping its screen-side edge put. `size` is its height (top/bottom)
/// or width (sides) in design points. Drags call this on every move with `save` off; `save` stores the size
/// as a share of the primary display's work area.
pub fn resize_panel(app: &AppHandle, size: f64, save: bool) -> AppResult<()> {
    if settings::get(&app.state::<AppState>().db.lock().unwrap(), "panelResize") == "off" {
        return Err(AppError::ResizeOff);
    }
    let panel = app.get_webview_window(PANEL).ok_or_else(|| AppError::Other("panel window is missing".into()))?;
    let monitor = panel
        .current_monitor()
        .map_err(|e| AppError::Other(e.to_string()))?
        .ok_or_else(|| AppError::Other("panel has no monitor".into()))?;
    let (work, unit) = work_area(&monitor);
    let pos = stored_position(app);
    let here = axis(work, unit, pos);
    let base = base_extent(app, pos, here);
    let e = clamp_panel_extent(pos, base, size).min(panel_max(here));
    place(&panel, panel_rect(work, unit, pos, e)).map_err(|e| AppError::Other(e.to_string()))?;
    if save {
        let ratio = format!("{:.4}", e / base);
        settings::set_raw(&app.state::<AppState>().db.lock().unwrap(), pos.ratio_key(), &ratio)?;
        settings::commands::changed(app);
    }
    Ok(())
}

/// Forgets the saved size of the current position's axis and puts a visible panel back at the default.
pub fn reset_panel_size(app: &AppHandle) -> AppResult<()> {
    settings::set_raw(&app.state::<AppState>().db.lock().unwrap(), stored_position(app).ratio_key(), "")?;
    place_panel(app);
    settings::commands::changed(app);
    Ok(())
}

/// Re-places a visible panel on its monitor from the saved position and size; a hidden one waits for the next open.
pub fn place_panel(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(PANEL).filter(|p| p.is_visible().unwrap_or(false))
        && let Some(monitor) = panel.current_monitor().ok().flatten()
    {
        let (work, unit) = work_area(&monitor);
        let pos = stored_position(app);
        if let Err(e) = place(&panel, panel_frame(app, work, unit, pos, stored_ratio(app, pos))) {
            log::error!("failed to place panel: {e}");
        }
    }
}

/// macOS only: settings and what's-new open on the Space the user is looking at,
/// instead of pulling them back to the one they were last shown on. Call once on the main thread.
pub fn follow_active_space(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    for label in [SETTINGS, WHATS_NEW] {
        let Some(window) = app.get_webview_window(label) else { continue };
        match window.ns_window() {
            // SAFETY: Tauri hands out the live NSWindow; setup runs on the main thread.
            Ok(ptr) => unsafe {
                use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
                let ns = &*(ptr as *const NSWindow);
                ns.setCollectionBehavior(ns.collectionBehavior() | NSWindowCollectionBehavior::MoveToActiveSpace);
            },
            Err(e) => log::warn!("couldn't set {label} collection behavior: {e}"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        // Hidden, not destroyed, on close; tell the page to start over on the general tab.
        let _ = app.emit_to(SETTINGS, "settings://opened", ());
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub(crate) fn show_toast(app: &AppHandle, payload: ToastPayload) {
    let Some(toast) = app.get_webview_window(TOAST) else { return };
    // A pinned panel stays up, so the toast keeps clear of it instead of covering its cards.
    let pinned_panel = app.get_webview_window(PANEL).filter(|_| pinned(app));
    let monitor = match &pinned_panel {
        Some(panel) => panel.current_monitor().ok().flatten(),
        None => cursor_monitor(app),
    };
    if let Some(monitor) = monitor {
        let (work, unit) = work_area(&monitor);
        // Live size, so a mid-drag or unsaved size still clears the panel.
        let pos = stored_position(app);
        let pinned = pinned_panel.as_ref().and_then(|p| p.outer_size().ok()).map(|s| {
            let px = if pos.side() { s.width } else { s.height };
            (pos, px as f64 / monitor.scale_factor())
        });
        let _ = place(&toast, toast_rect(work, unit, pinned));
    }
    let _ = app.emit_to(TOAST, "toast://show", payload);
    let _ = toast.set_ignore_cursor_events(true);
    let _ = toast.show();

    let generation = TOAST_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        // A newer toast restarted the timer; let it run the exit and hide the window.
        let current = || TOAST_GENERATION.load(Ordering::SeqCst) == generation;
        std::thread::sleep(Duration::from_millis(TOAST_MS));
        if !current() {
            return;
        }
        let _ = app.emit_to(TOAST, "toast://hide", ());
        std::thread::sleep(Duration::from_millis(TOAST_EXIT_MS));
        if current()
            && let Some(toast) = app.get_webview_window(TOAST)
        {
            let _ = toast.hide();
        }
    });
}

/// Writes the clip's raw pasteboard formats when it has them, else its card content.
fn write_clipboard(state: &AppState, content: &ClipContent, formats: Vec<(String, Vec<u8>)>) -> AppResult<()> {
    use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext, RustImageData, common::RustImage};
    let ctx = ClipboardContext::new().map_err(|e| AppError::Other(e.to_string()))?;
    state.suppress_watcher();
    match content {
        _ if !formats.is_empty() => ctx.set(formats.into_iter().map(|(f, d)| ClipboardContent::Other(f, d)).collect()),
        ClipContent::Text(text) => ctx.set_text(text.clone()),
        ClipContent::Image(path) => {
            let image = RustImageData::from_path(&path.to_string_lossy()).map_err(|e| AppError::Other(e.to_string()))?;
            ctx.set_image(image)
        }
        ClipContent::Files(files) => {
            if files.iter().any(|f| !std::path::Path::new(f).exists()) {
                return Err(AppError::Other("a copied file no longer exists".into()));
            }
            ctx.set_files(files.clone())
        }
    }
    .map_err(|e| AppError::Other(e.to_string()))?;
    // Re-arm after the write: decoding a large image can take a few hundred ms.
    state.suppress_watcher();
    Ok(())
}

/// Copies a clip back to the clipboard, closes the panel and confirms with a toast.
/// `plain` skips the saved raw formats so only the text goes back.
pub fn copy_clip(app: &AppHandle, id: i64, plain: bool) -> AppResult<()> {
    let state = app.state::<AppState>();
    let (content, formats) = {
        let conn = state.db.lock().unwrap();
        let formats = if plain { Vec::new() } else { clip::formats(&conn, id)? };
        (clip::content(&conn, id)?, formats)
    };
    let result = match content {
        Some(content) => write_clipboard(&state, &content, formats).map(|()| content),
        None => Err(AppError::Other("clip no longer exists".into())),
    };
    hide_panel(app, true);
    match result {
        Ok(content) => {
            confirm_copy(app, id, &content, plain);
            Ok(())
        }
        Err(e) => {
            log::warn!("copy failed: {e}");
            show_toast(app, ToastPayload::failed());
            Err(e)
        }
    }
}

/// Bumps the clip, plays the copy sound and confirms with a toast.
fn confirm_copy(app: &AppHandle, id: i64, content: &ClipContent, plain: bool) {
    let state = app.state::<AppState>();
    let sound = {
        let conn = state.db.lock().unwrap();
        let _ = clip::touch(&conn, id, now_ms());
        settings::copy_sound(&conn)
    };
    if let Some(name) = sound {
        crate::app::sound::play(&name);
    }
    let _ = app.emit("clips://changed", ());
    show_toast(app, ToastPayload::copied(content, plain));
}

const DRAG_ICON: &[u8] = include_bytes!("../../icons/128x128.png");
const DRAG_IMAGE_SIZE: u32 = 96;

/// A small PNG to show under the cursor while dragging.
fn drag_image(preview: Option<Vec<u8>>) -> Vec<u8> {
    use clipboard_rs::{RustImageData, common::RustImage};
    preview
        .and_then(|png| {
            let image = RustImageData::from_bytes(&png).ok()?;
            Some(image.thumbnail(DRAG_IMAGE_SIZE, DRAG_IMAGE_SIZE).ok()?.to_png().ok()?.get_bytes().to_vec())
        })
        .unwrap_or_else(|| DRAG_ICON.to_vec())
}

fn card_image(base64_png: Option<&str>) -> Option<Vec<u8>> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    png_at_retina(&STANDARD.decode(base64_png?).ok()?)
}

/// Marks a PNG as 144 dpi (a `pHYs` chunk right after `IHDR`, replacing any other), so macOS
/// sizes a 2x-rendered drag image in points and it stays sharp on Retina. Windows ignores the
/// chunk and draws pixels 1:1, which is why the card is drawn at device pixels there. None if
/// not a PNG.
fn png_at_retina(png: &[u8]) -> Option<Vec<u8>> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }
    let mut rest = png.strip_prefix(SIGNATURE)?;
    let mut out = SIGNATURE.to_vec();
    while !rest.is_empty() {
        let len = u32::from_be_bytes(rest.get(..4)?.try_into().ok()?) as usize;
        let chunk = rest.get(..12 + len)?;
        let kind = &chunk[4..8];
        if kind != b"pHYs" {
            out.extend_from_slice(chunk);
        }
        if kind == b"IHDR" {
            let ppm = 5669u32.to_be_bytes(); // 144 dpi in pixels per metre
            let body = [b"pHYs".as_slice(), &ppm, &ppm, &[1]].concat();
            out.extend_from_slice(&9u32.to_be_bytes());
            out.extend_from_slice(&body);
            out.extend_from_slice(&crc32(&body).to_be_bytes());
        }
        rest = &rest[12 + len..];
    }
    Some(out)
}

/// A safe file name for a dragged image: no path parts, always `.png`.
fn drag_file_name(name: &str) -> String {
    let clean: String = name.chars().map(|c| if c.is_alphanumeric() || " -._".contains(c) { c } else { '_' }).collect();
    let clean = clean.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let stem = clean.strip_suffix(".png").unwrap_or(clean);
    format!("{}.png", if stem.is_empty() { "Vee" } else { stem })
}

/// Stored images are named by hash; drag a copy under a readable name instead.
fn stage_image(src: &Path, dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(drag_file_name(name));
    std::fs::copy(src, &dest)?;
    Ok(dest)
}

/// Starts dragging any clip out of the panel. Drops always copy.
/// `name` names the dropped file for image clips. `image` is the card itself as a base64 PNG
/// laid out for this platform (see `dragImageLayout` in motion.ts); without it the clip's
/// thumbnail is dragged.
pub fn start_drag(app: &AppHandle, id: i64, name: &str, image: Option<&str>) -> AppResult<()> {
    let state = app.state::<AppState>();
    let (content, preview) = {
        let conn = state.db.lock().unwrap();
        let content = clip::content(&conn, id)?.ok_or_else(|| AppError::Other("clip no longer exists".into()))?;
        (content, clip::preview_png(&conn, id)?)
    };
    let item = match &content {
        ClipContent::Files(paths) => {
            if paths.iter().any(|p| !Path::new(p).exists()) {
                return Err(AppError::Other("a copied file no longer exists".into()));
            }
            DragItem::Files(paths.iter().map(PathBuf::from).collect())
        }
        ClipContent::Image(path) => DragItem::Files(vec![
            stage_image(path, &std::env::temp_dir().join("vee-drag"), name)?,
        ]),
        ClipContent::Text(text) => DragItem::Text(text.clone()),
    };
    // Image and files clips can be converted; their staged or original paths are the sources.
    let sources = match &item {
        DragItem::Files(files) => Some(files.clone()),
        _ => None,
    };
    let panel = app.get_webview_window(PANEL).ok_or_else(|| AppError::Other("panel window is missing".into()))?;
    state.dragging.store(true, Ordering::SeqCst);
    // Let drops land on whatever is behind the (slid-away) panel.
    let _ = panel.set_ignore_cursor_events(true);
    // Before `drag::start_drag`, which blocks until the drop on Windows.
    if let Some(files) = sources {
        convert::begin(app, id, files);
    }
    let handle = app.clone();
    let started = drag::start_drag(
        &panel,
        item,
        drag::Image::Raw(card_image(image).unwrap_or_else(|| drag_image(preview))),
        move |result, _cursor| finish_drag(&handle, id, &content, result),
        drag::Options { mode: DragMode::Copy, ..Default::default() },
    );
    if let Err(e) = started {
        end_drag(app);
        return Err(AppError::Other(e.to_string()));
    }
    Ok(())
}

fn end_drag(app: &AppHandle) -> Option<WebviewWindow> {
    app.state::<AppState>().dragging.store(false, Ordering::SeqCst);
    let panel = app.get_webview_window(PANEL)?;
    let _ = panel.set_ignore_cursor_events(false);
    Some(panel)
}

fn finish_drag(app: &AppHandle, id: i64, content: &ClipContent, result: DragResult) {
    let panel = end_drag(app);
    // Decided by geometry: the overlay's own drop event only arrives after this.
    let on_overlay = convert::dropped_on_overlay(app);
    match result {
        DragResult::Dropped => {
            if pinned(app) {
                // The panel stays; let it bring the dragged card back.
                let _ = app.emit_to(PANEL, "panel://drag-cancelled", ());
            } else {
                hide_panel(app, true);
            }
            // A drop on the convert overlay is a convert, not a copy.
            if !on_overlay {
                confirm_copy(app, id, content, false);
            }
        }
        DragResult::Cancel => {
            if let Some(panel) = panel {
                let _ = panel.set_focus();
            }
            let _ = app.emit_to(PANEL, "panel://drag-cancelled", ());
        }
    }
}

/// Panel and settings-window commands. A submodule so they can share names with the functions above.
pub mod commands {
    use crate::{app::state::AppState, shared::error::AppResult};
    use std::sync::atomic::Ordering;
    use tauri::{AppHandle, State};

    #[tauri::command]
    pub fn hide_panel(app: AppHandle) {
        super::hide_panel(&app, true);
    }

    /// Unpinning leaves the panel open; it closes the next time something hides it.
    #[tauri::command]
    pub fn set_panel_pinned(state: State<AppState>, pinned: bool) {
        state.panel_pinned.store(pinned, Ordering::SeqCst);
    }

    #[tauri::command]
    pub fn resize_panel(app: AppHandle, size: f64, save: bool) -> AppResult<()> {
        super::resize_panel(&app, size, save)
    }

    #[tauri::command]
    pub fn reset_panel_size(app: AppHandle) -> AppResult<()> {
        super::reset_panel_size(&app)
    }

    /// The panel measured that `fits` pinned cards leave room for history and more are pinned.
    #[tauri::command]
    pub fn warn_side_pins(app: AppHandle, fits: usize) {
        super::show_toast(&app, super::ToastPayload::pins_fit(fits));
    }

    #[tauri::command]
    pub fn reveal_panel(app: AppHandle) {
        super::reveal_panel(&app);
    }

    #[tauri::command]
    pub fn open_settings(app: AppHandle) {
        super::hide_panel(&app, false);
        super::show_settings(&app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragged_image_names_are_path_free_pngs() {
        assert_eq!(drag_file_name("Vee 2026-10-07 14.32.05"), "Vee 2026-10-07 14.32.05.png");
        assert_eq!(drag_file_name("shot.png"), "shot.png");
        assert_eq!(drag_file_name("a/b\\c"), "a_b_c.png");
        assert_eq!(drag_file_name("../x"), "_x.png");
        assert_eq!(drag_file_name(".."), "Vee.png");
        assert_eq!(drag_file_name(""), "Vee.png");
    }

    #[test]
    fn staged_image_is_a_named_copy_of_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("3f9a.png");
        std::fs::write(&src, b"png bytes").unwrap();
        let staged = stage_image(&src, &dir.path().join("drag"), "Vee 2026-10-07 14.32.05").unwrap();
        assert_eq!(staged.file_name().unwrap(), "Vee 2026-10-07 14.32.05.png");
        assert_eq!(std::fs::read(&staged).unwrap(), b"png bytes");
        assert!(src.exists());
    }

    #[test]
    fn drag_image_shrinks_previews_and_falls_back_to_the_app_icon() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let pixel = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
            .unwrap();
        assert!(drag_image(Some(pixel)).starts_with(b"\x89PNG"));
        assert_eq!(drag_image(None), DRAG_ICON);
        assert_eq!(drag_image(Some(vec![1, 2, 3])), DRAG_ICON);
    }

    #[test]
    fn card_image_is_marked_as_retina_once_and_still_decodes() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use clipboard_rs::{RustImageData, common::RustImage};
        let pixel = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
            .unwrap();
        let marked = png_at_retina(&pixel).unwrap();
        // 144 dpi = 5669 pixels per metre, in both directions, unit = metre.
        let phys = [b"pHYs".as_slice(), &5669u32.to_be_bytes(), &5669u32.to_be_bytes(), &[1]].concat();
        assert!(marked.windows(phys.len()).any(|w| w == phys));
        // The decoder checks every chunk's CRC.
        assert_eq!(RustImageData::from_bytes(&marked).unwrap().get_size(), (1, 1));
        // Marking twice replaces the chunk instead of adding a second one.
        let twice = png_at_retina(&marked).unwrap();
        assert_eq!(twice.windows(4).filter(|w| *w == b"pHYs").count(), 1);
        assert_eq!(png_at_retina(&[1, 2, 3]), None);
    }

    // Geometry captured from a MacBook (Retina, scale 2) with a 1x 2560x1440
    // display placed to its left — the setup where the panel never reached the
    // external display.
    const MAC_PRIMARY: Rect = Rect { x: 0.0, y: 0.0, w: 1800.0, h: 1169.0 };
    const MAC_EXTERNAL: Rect = Rect { x: -2560.0, y: -271.0, w: 2560.0, h: 1440.0 };

    #[test]
    fn monitor_rects_are_points_on_macos_and_pixels_on_windows() {
        assert_eq!(rect_from((0, 0), (3600, 2338), 2.0, true), MAC_PRIMARY);
        assert_eq!(rect_from((-2560, -271), (2560, 1440), 1.0, true), MAC_EXTERNAL);
        assert_eq!(rect_from((-1920, 0), (1920, 1080), 1.5, false), Rect { x: -1920.0, y: 0.0, w: 1920.0, h: 1080.0 });
    }

    #[test]
    fn cursor_on_the_external_display_picks_it() {
        let rects = [MAC_PRIMARY, MAC_EXTERNAL];
        // Raw tao cursor values logged while the pointer was on the external display.
        for raw in [(-2638.21875, 647.3125), (-3108.2734375, -166.953125), (-3980.0546875, 391.65625)] {
            assert_eq!(pick(cursor_point(raw, 2.0, true), &rects), Some(1), "{raw:?}");
        }
        assert_eq!(pick(cursor_point((1000.0, 1000.0), 2.0, true), &rects), Some(0));
        assert_eq!(pick(cursor_point((-1000.0, 500.0), 1.5, false), &[Rect { x: -1920.0, y: 0.0, w: 1920.0, h: 1080.0 }]), Some(0));
    }

    #[test]
    fn panel_height_is_thirty_percent_of_the_work_area() {
        assert_eq!(panel_height(956.0), 287.0); // MacBook Air 13"
        assert_eq!(panel_height(1117.0), 335.0); // MacBook Pro 16"
        assert_eq!(panel_height(1440.0), 432.0); // 27" QHD
    }

    #[test]
    fn panel_and_toast_sit_at_the_bottom_of_the_work_area() {
        let work = Rect { x: -2560.0, y: -241.0, w: 2560.0, h: 1410.0 };
        assert_eq!(panel_rect(work, 1.0, PanelPosition::Bottom, 360.0), Rect { x: -2552.0, y: 801.0, w: 2544.0, h: 360.0 });
        assert_eq!(toast_rect(work, 1.0, None), Rect { x: -1490.0, y: 1023.0, w: 420.0, h: 132.0 });
        // Windows: same layout in pixels at 150%.
        let px = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1040.0 };
        assert_eq!(panel_rect(px, 1.5, PanelPosition::Bottom, 300.0), Rect { x: 12.0, y: 578.0, w: 1896.0, h: 450.0 });
    }

    #[test]
    fn panel_can_grow_to_half_the_work_area() {
        assert_eq!(panel_max(1117.0), 558.0);
        assert_eq!(clamp_panel_extent(PanelPosition::Bottom, 1117.0, 447.2), 447.0);
        assert_eq!(clamp_panel_extent(PanelPosition::Bottom, 1117.0, 900.0), 558.0);
        assert_eq!(clamp_panel_extent(PanelPosition::Bottom, 1117.0, 100.0), 168.0); // never below 15%
        assert_eq!(clamp_panel_extent(PanelPosition::Bottom, 1117.0, 200.0), 200.0); // below the default is fine
    }

    #[test]
    fn stored_ratio_is_clamped_to_the_primary_display() {
        assert_eq!(panel_extent_for(PanelPosition::Bottom, 1117.0, 1117.0, None), 335.0);
        assert_eq!(panel_extent_for(PanelPosition::Bottom, 1117.0, 1117.0, Some(0.4)), 447.0);
        assert_eq!(panel_extent_for(PanelPosition::Bottom, 956.0, 956.0, Some(0.6)), 478.0); // over 50%
        assert_eq!(panel_extent_for(PanelPosition::Bottom, 1440.0, 1440.0, Some(0.1)), 216.0); // under 15%
    }

    #[test]
    fn panel_keeps_the_primary_display_height_on_other_monitors() {
        // MacBook Pro 16" primary, 27" QHD external: same height as on the MacBook.
        assert_eq!(panel_extent_for(PanelPosition::Bottom, 1117.0, 1410.0, None), 335.0);
        assert_eq!(panel_extent_for(PanelPosition::Bottom, 1117.0, 1410.0, Some(0.4)), 447.0);
        // A smaller external display still caps the panel at half its own work area.
        assert_eq!(panel_extent_for(PanelPosition::Bottom, 1117.0, 800.0, Some(0.5)), 400.0);
    }

    #[test]
    fn pinned_toast_sits_above_the_panel() {
        let work = Rect { x: -2560.0, y: -241.0, w: 2560.0, h: 1410.0 };
        assert_eq!(toast_rect(work, 1.0, Some((PanelPosition::Bottom, 360.0))), Rect { x: -1490.0, y: 655.0, w: 420.0, h: 132.0 });
    }

    #[test]
    fn panel_docks_to_each_edge() {
        let work = Rect { x: -2560.0, y: -241.0, w: 2560.0, h: 1410.0 };
        use PanelPosition::*;
        assert_eq!(panel_rect(work, 1.0, Bottom, 360.0), Rect { x: -2552.0, y: 801.0, w: 2544.0, h: 360.0 });
        assert_eq!(panel_rect(work, 1.0, Top, 360.0), Rect { x: -2552.0, y: -233.0, w: 2544.0, h: 360.0 });
        assert_eq!(panel_rect(work, 1.0, Left, 400.0), Rect { x: -2552.0, y: -233.0, w: 400.0, h: 1394.0 });
        assert_eq!(panel_rect(work, 1.0, Right, 400.0), Rect { x: -408.0, y: -233.0, w: 400.0, h: 1394.0 });
    }

    #[test]
    fn panel_rects_scale_on_windows() {
        let px = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1040.0 };
        assert_eq!(panel_rect(px, 1.5, PanelPosition::Top, 300.0), Rect { x: 12.0, y: 12.0, w: 1896.0, h: 450.0 });
        assert_eq!(panel_rect(px, 1.5, PanelPosition::Right, 340.0), Rect { x: 1398.0, y: 12.0, w: 510.0, h: 1016.0 });
    }

    #[test]
    fn side_width_defaults_to_a_quarter_with_a_340pt_floor() {
        use PanelPosition::Left;
        assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, None), 432.0);
        assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, Some(0.1)), 340.0); // floor beats 15%
        assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, Some(0.7)), 864.0); // 50% cap
        assert_eq!(panel_extent_for(PanelPosition::Bottom, 1117.0, 1117.0, None), 335.0); // unchanged
    }

    #[test]
    fn side_width_floor_never_exceeds_the_cap() {
        use PanelPosition::Right;
        assert_eq!(clamp_panel_extent(Right, 600.0, 100.0), 300.0); // 50% of 600 < 340
        assert_eq!(panel_extent_for(Right, 1728.0, 600.0, None), 300.0); // small external display
    }

    #[test]
    fn pinned_toast_avoids_the_panel_on_every_edge() {
        let work = Rect { x: -2560.0, y: -241.0, w: 2560.0, h: 1410.0 };
        use PanelPosition::*;
        assert_eq!(toast_rect(work, 1.0, Some((Top, 360.0))), toast_rect(work, 1.0, None));
        assert_eq!(toast_rect(work, 1.0, Some((Left, 400.0))), Rect { x: -1286.0, y: 1023.0, w: 420.0, h: 132.0 });
        assert_eq!(toast_rect(work, 1.0, Some((Right, 400.0))), Rect { x: -1694.0, y: 1023.0, w: 420.0, h: 132.0 });
    }

    #[test]
    fn toast_beside_a_wide_side_panel_stays_on_screen_and_hugs_the_far_edge() {
        let small = Rect { x: 0.0, y: 0.0, w: 600.0, h: 800.0 };
        use PanelPosition::*;
        assert_eq!(toast_rect(small, 1.0, Some((Left, 300.0))).x, 180.0);
        assert_eq!(toast_rect(small, 1.0, Some((Right, 300.0))).x, 0.0);
    }

    #[test]
    fn unknown_position_falls_back_to_bottom() {
        assert_eq!(PanelPosition::parse("left"), PanelPosition::Left);
        assert_eq!(PanelPosition::parse(""), PanelPosition::Bottom);
        assert_eq!(PanelPosition::Right.ratio_key(), "panelWidthRatio");
        assert_eq!(PanelPosition::Top.ratio_key(), "panelHeightRatio");
    }

    #[test]
    fn toast_names_what_was_copied() {
        assert_eq!(ToastPayload::copied(&ClipContent::Text("hi".into()), false).kind, Kind::Text);
        assert_eq!(ToastPayload::copied(&ClipContent::Text("https://a.com".into()), false).kind, Kind::Link);
        assert_eq!(ToastPayload::copied(&ClipContent::Image(PathBuf::from("/x.png")), false).kind, Kind::Image);
        let files = ToastPayload::copied(&ClipContent::Files(vec!["/a".into(), "/b".into()]), false);
        assert_eq!((files.kind, files.files), (Kind::Files, 2));
    }

    #[test]
    fn side_pins_toast_carries_the_count_that_fits() {
        let v = serde_json::to_value(ToastPayload::pins_fit(1)).unwrap();
        assert_eq!((v["ok"].as_bool(), v["pinsFit"].as_u64()), (Some(false), Some(1)));
        assert!(serde_json::to_value(ToastPayload::failed()).unwrap()["pinsFit"].is_null());
    }

    #[test]
    fn toast_says_plain_only_for_text() {
        assert!(ToastPayload::copied(&ClipContent::Text("hi".into()), true).plain);
        assert!(!ToastPayload::copied(&ClipContent::Text("hi".into()), false).plain);
        // Images and files have no formatting to strip, so they copy as usual.
        assert!(!ToastPayload::copied(&ClipContent::Image(PathBuf::from("/x.png")), true).plain);
        assert!(!ToastPayload::copied(&ClipContent::Files(vec!["/a".into()]), true).plain);
    }

    #[test]
    fn toast_preview_collapses_whitespace_and_caps_length() {
        assert_eq!(toast_preview("a\n   b\tc"), "a b c");
        let long = "x".repeat(41);
        assert_eq!(toast_preview(&long), format!("{}…", "x".repeat(40)));
        assert_eq!(toast_preview(&"가".repeat(40)), "가".repeat(40));
    }
}
