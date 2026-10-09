//! Panel, toast and settings window behaviour, plus copying a clip back.

use crate::{AppState, now_ms, source_app, store::{ClipContent, Kind, classify_text}};
use drag::{DragItem, DragMode, DragResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const PANEL: &str = "panel";
pub const TOAST: &str = "toast";
pub const SETTINGS: &str = "settings";

const PANEL_RATIO: f64 = 0.30;
const PANEL_MIN: f64 = 300.0;
const PANEL_MAX: f64 = 360.0;
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
}

impl ToastPayload {
    fn copied(content: &ClipContent, plain: bool) -> Self {
        match content {
            ClipContent::Text(t) => {
                Self { ok: true, kind: classify_text(t), text: Some(toast_preview(t)), files: 0, plain }
            }
            ClipContent::Image(_) => Self { ok: true, kind: Kind::Image, text: None, files: 0, plain: false },
            ClipContent::Files(f) => Self { ok: true, kind: Kind::Files, text: None, files: f.len(), plain: false },
        }
    }

    fn failed() -> Self {
        Self { ok: false, kind: Kind::Text, text: None, files: 0, plain: false }
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
const USES_POINTS: bool = cfg!(target_os = "macos");

/// A rectangle in the space windows are positioned in (see `USES_POINTS`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn contains(&self, (x, y): (f64, f64)) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// tao reports macOS monitor geometry as points × that monitor's own scale.
fn rect_from((x, y): (i32, i32), (w, h): (u32, u32), scale: f64, points: bool) -> Rect {
    let k = if points { scale } else { 1.0 };
    Rect { x: x as f64 / k, y: y as f64 / k, w: w as f64 / k, h: h as f64 / k }
}

/// tao's macOS cursor is points × the *primary* display's scale, whichever display it is on,
/// so `monitor_from_point` misses displays with a different scale.
fn cursor_point((x, y): (f64, f64), primary_scale: f64, points: bool) -> (f64, f64) {
    if points { (x / primary_scale, y / primary_scale) } else { (x, y) }
}

fn pick(point: (f64, f64), rects: &[Rect]) -> Option<usize> {
    rects.iter().position(|r| r.contains(point))
}

/// Panel height in design points: a share of the work area, clamped.
fn panel_height(work_h_points: f64) -> f64 {
    (work_h_points * PANEL_RATIO).round().clamp(PANEL_MIN, PANEL_MAX)
}

/// `unit` is how many positioning units one design point takes: 1 on macOS, the scale on Windows.
fn panel_rect(work: Rect, unit: f64) -> Rect {
    let (margin, h) = (PANEL_MARGIN * unit, panel_height(work.h / unit) * unit);
    Rect { x: work.x + margin, y: work.y + work.h - h - margin, w: work.w - 2.0 * margin, h }
}

fn toast_rect(work: Rect, unit: f64) -> Rect {
    let (w, h) = (TOAST_WIDTH * unit, TOAST_HEIGHT * unit);
    Rect { x: work.x + (work.w - w) / 2.0, y: work.y + work.h - h - TOAST_BOTTOM * unit, w, h }
}

fn cursor_monitor(app: &AppHandle) -> Option<Monitor> {
    let primary = app.primary_monitor().ok().flatten();
    let found = (|| {
        let monitors = app.available_monitors().ok()?;
        let cursor = app.cursor_position().ok()?;
        let primary_scale = primary.as_ref().map_or(1.0, |m| m.scale_factor());
        let rects: Vec<Rect> = monitors
            .iter()
            .map(|m| rect_from((m.position().x, m.position().y), (m.size().width, m.size().height), m.scale_factor(), USES_POINTS))
            .collect();
        let index = pick(cursor_point((cursor.x, cursor.y), primary_scale, USES_POINTS), &rects)?;
        monitors.into_iter().nth(index)
    })();
    found.or(primary)
}

/// Work area of `monitor` in positioning space, plus the size of one design point in it.
fn work_area(monitor: &Monitor) -> (Rect, f64) {
    let a = monitor.work_area();
    let rect = rect_from((a.position.x, a.position.y), (a.size.width, a.size.height), monitor.scale_factor(), USES_POINTS);
    (rect, if USES_POINTS { 1.0 } else { monitor.scale_factor() })
}

fn place(window: &WebviewWindow, r: Rect) -> tauri::Result<()> {
    if USES_POINTS {
        window.set_position(LogicalPosition::new(r.x, r.y))?;
        window.set_size(LogicalSize::new(r.w, r.h))?;
    } else {
        window.set_position(PhysicalPosition::new(r.x.round() as i32, r.y.round() as i32))?;
        window.set_size(PhysicalSize::new(r.w.round() as u32, r.h.round() as u32))?;
    }
    Ok(())
}

pub fn toggle_panel(app: &AppHandle) {
    let visible = app.get_webview_window(PANEL).and_then(|p| p.is_visible().ok()).unwrap_or(false);
    if visible && !PANEL_CLOSING.load(Ordering::SeqCst) { hide_panel(app, true) } else { show_panel(app) }
}

pub fn show_panel(app: &AppHandle) {
    let Some(panel) = app.get_webview_window(PANEL) else { return };
    if panel.is_visible().unwrap_or(false) {
        if PANEL_CLOSING.swap(false, Ordering::SeqCst) {
            // Reopened mid-close: cancel the pending hide and slide back up.
            PANEL_GENERATION.fetch_add(1, Ordering::SeqCst);
            *app.state::<AppState>().prev_app_pid.lock().unwrap() = source_app::frontmost().map(|a| a.pid);
            let _ = app.emit_to(PANEL, "panel://opened", ());
        }
        let _ = panel.set_focus();
        return;
    }
    *app.state::<AppState>().prev_app_pid.lock().unwrap() = source_app::frontmost().map(|a| a.pid);
    if let Err(e) = place_and_show(app, &panel) {
        log::error!("failed to show panel: {e}");
    }
}

fn place_and_show(app: &AppHandle, panel: &WebviewWindow) -> tauri::Result<()> {
    let _ = end_drag(app);
    if let Some(monitor) = cursor_monitor(app) {
        let (work, unit) = work_area(&monitor);
        place(panel, panel_rect(work, unit))?;
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
pub fn hide_panel(app: &AppHandle, restore_focus: bool) {
    let previous = app.state::<AppState>().prev_app_pid.lock().unwrap().take();
    if let (true, Some(pid)) = (restore_focus, previous) {
        source_app::activate(pid);
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

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn show_toast(app: &AppHandle, payload: ToastPayload) {
    let Some(toast) = app.get_webview_window(TOAST) else { return };
    if let Some(monitor) = cursor_monitor(app) {
        let (work, unit) = work_area(&monitor);
        let _ = place(&toast, toast_rect(work, unit));
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
fn write_clipboard(state: &AppState, content: &ClipContent, formats: Vec<(String, Vec<u8>)>) -> Result<(), String> {
    use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext, RustImageData, common::RustImage};
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
    state.suppress_watcher();
    match content {
        _ if !formats.is_empty() => ctx.set(formats.into_iter().map(|(f, d)| ClipboardContent::Other(f, d)).collect()),
        ClipContent::Text(text) => ctx.set_text(text.clone()),
        ClipContent::Image(path) => {
            let image = RustImageData::from_path(&path.to_string_lossy()).map_err(|e| e.to_string())?;
            ctx.set_image(image)
        }
        ClipContent::Files(files) => {
            if files.iter().any(|f| !std::path::Path::new(f).exists()) {
                return Err("a copied file no longer exists".into());
            }
            ctx.set_files(files.clone())
        }
    }
    .map_err(|e| e.to_string())?;
    // Re-arm after the write: decoding a large image can take a few hundred ms.
    state.suppress_watcher();
    Ok(())
}

/// Copies a clip back to the clipboard, closes the panel and confirms with a toast.
/// `plain` skips the saved raw formats so only the text goes back.
pub fn copy_clip(app: &AppHandle, id: i64, plain: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (content, formats) = {
        let store = state.store.lock().unwrap();
        let formats = if plain { Vec::new() } else { store.formats(id).map_err(|e| e.to_string())? };
        (store.content(id).map_err(|e| e.to_string())?, formats)
    };
    let result = match content {
        Some(content) => write_clipboard(&state, &content, formats).map(|()| content),
        None => Err("clip no longer exists".into()),
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
        let store = state.store.lock().unwrap();
        let _ = store.touch(id, now_ms());
        crate::settings::copy_sound(&store)
    };
    if let Some(name) = sound {
        crate::sound::play(&name);
    }
    let _ = app.emit("clips://changed", ());
    show_toast(app, ToastPayload::copied(content, plain));
}

const DRAG_ICON: &[u8] = include_bytes!("../icons/128x128.png");
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

/// Starts dragging a files or image clip out of the panel. Drops always copy.
/// `name` names the dropped file for image clips. `image` is the card itself as a base64 PNG
/// laid out for this platform (see `dragImageLayout` in motion.ts); without it the clip's
/// thumbnail is dragged.
pub fn start_drag(app: &AppHandle, id: i64, name: &str, image: Option<&str>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (content, preview) = {
        let store = state.store.lock().unwrap();
        let content = store.content(id).map_err(|e| e.to_string())?.ok_or("clip no longer exists")?;
        (content, store.preview_png(id).map_err(|e| e.to_string())?)
    };
    let paths = match &content {
        ClipContent::Files(paths) => {
            if paths.iter().any(|p| !Path::new(p).exists()) {
                return Err("a copied file no longer exists".into());
            }
            paths.iter().map(PathBuf::from).collect()
        }
        ClipContent::Image(path) => {
            vec![stage_image(path, &std::env::temp_dir().join("vee-drag"), name).map_err(|e| e.to_string())?]
        }
        ClipContent::Text(_) => return Err("only files and images can be dragged".into()),
    };
    let panel = app.get_webview_window(PANEL).ok_or("panel window is missing")?;
    state.dragging.store(true, Ordering::SeqCst);
    // Let drops land on whatever is behind the (slid-away) panel.
    let _ = panel.set_ignore_cursor_events(true);
    let handle = app.clone();
    let started = drag::start_drag(
        &panel,
        DragItem::Files(paths),
        drag::Image::Raw(card_image(image).unwrap_or_else(|| drag_image(preview))),
        move |result, _cursor| finish_drag(&handle, id, &content, result),
        drag::Options { mode: DragMode::Copy, ..Default::default() },
    );
    if let Err(e) = started {
        end_drag(app);
        return Err(e.to_string());
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
    match result {
        DragResult::Dropped => {
            hide_panel(app, true);
            confirm_copy(app, id, content, false);
        }
        DragResult::Cancel => {
            if let Some(panel) = panel {
                let _ = panel.set_focus();
            }
            let _ = app.emit_to(PANEL, "panel://drag-cancelled", ());
        }
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
    fn panel_height_is_thirty_percent_of_the_work_area_clamped() {
        assert_eq!(panel_height(956.0), 300.0); // MacBook Air 13" → 287, floored to 300
        assert_eq!(panel_height(1117.0), 335.0); // MacBook Pro 16"
        assert_eq!(panel_height(1200.0), 360.0); // exactly at the cap
        assert_eq!(panel_height(1440.0), 360.0); // 27" QHD → 432, capped
        assert_eq!(panel_height(1692.0), 360.0); // 32" 4K scaled → 508, capped
    }

    #[test]
    fn panel_and_toast_sit_at_the_bottom_of_the_work_area() {
        let work = Rect { x: -2560.0, y: -241.0, w: 2560.0, h: 1410.0 };
        assert_eq!(panel_rect(work, 1.0), Rect { x: -2552.0, y: 801.0, w: 2544.0, h: 360.0 });
        assert_eq!(toast_rect(work, 1.0), Rect { x: -1490.0, y: 1023.0, w: 420.0, h: 132.0 });
        // Windows: same layout in pixels at 150%.
        let px = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1040.0 };
        assert_eq!(panel_rect(px, 1.5), Rect { x: 12.0, y: 578.0, w: 1896.0, h: 450.0 });
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
