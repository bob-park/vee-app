//! Panel, toast and settings window behaviour, plus copying a clip back.

use crate::{AppState, now_ms, source_app, store::ClipContent};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const PANEL: &str = "panel";
pub const TOAST: &str = "toast";
pub const SETTINGS: &str = "settings";

const PANEL_HEIGHT: f64 = 300.0;
const PANEL_MARGIN: f64 = 8.0;
const TOAST_WIDTH: f64 = 360.0;
const TOAST_HEIGHT: f64 = 52.0;
const TOAST_BOTTOM: f64 = 32.0;
const TOAST_MS: u64 = 1500;
const TOAST_PREVIEW_CHARS: usize = 40;

static TOAST_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToastPayload {
    pub ok: bool,
    pub text: Option<String>,
    pub files: usize,
    pub image: bool,
}

impl ToastPayload {
    fn copied(content: &ClipContent) -> Self {
        match content {
            ClipContent::Text(t) => Self { ok: true, text: Some(toast_preview(t)), files: 0, image: false },
            ClipContent::Image(_) => Self { ok: true, text: None, files: 0, image: true },
            ClipContent::Files(f) => Self { ok: true, text: None, files: f.len(), image: false },
        }
    }

    fn failed() -> Self {
        Self { ok: false, text: None, files: 0, image: false }
    }
}

fn toast_preview(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= TOAST_PREVIEW_CHARS {
        return flat;
    }
    format!("{}…", flat.chars().take(TOAST_PREVIEW_CHARS).collect::<String>())
}

fn cursor_monitor(app: &AppHandle) -> Option<Monitor> {
    let pos = app.cursor_position().ok()?;
    app.monitor_from_point(pos.x, pos.y).ok().flatten().or_else(|| app.primary_monitor().ok().flatten())
}

pub fn toggle_panel(app: &AppHandle) {
    let visible = app.get_webview_window(PANEL).and_then(|p| p.is_visible().ok()).unwrap_or(false);
    if visible { hide_panel(app, true) } else { show_panel(app) }
}

pub fn show_panel(app: &AppHandle) {
    let Some(panel) = app.get_webview_window(PANEL) else { return };
    if panel.is_visible().unwrap_or(false) {
        let _ = panel.set_focus();
        return;
    }
    *app.state::<AppState>().prev_app_pid.lock().unwrap() = source_app::frontmost().map(|a| a.pid);
    if let Err(e) = place_and_show(app, &panel) {
        log::error!("failed to show panel: {e}");
    }
}

fn place_and_show(app: &AppHandle, panel: &WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = cursor_monitor(app) {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let margin = (PANEL_MARGIN * scale) as i32;
        let height = (PANEL_HEIGHT * scale) as u32;
        let width = area.size.width.saturating_sub(2 * margin as u32);
        panel.set_size(PhysicalSize::new(width, height))?;
        panel.set_position(PhysicalPosition::new(
            area.position.x + margin,
            area.position.y + area.size.height as i32 - height as i32 - margin,
        ))?;
    }
    panel.show()?;
    panel.set_focus()?;
    app.emit_to(PANEL, "panel://opened", ())?;
    Ok(())
}

/// `restore_focus` re-activates the app that was in front before the panel opened.
/// Pass `false` when the user already clicked into another app.
pub fn hide_panel(app: &AppHandle, restore_focus: bool) {
    if let Some(panel) = app.get_webview_window(PANEL) {
        let _ = panel.hide();
        let _ = app.emit_to(PANEL, "panel://closed", ());
    }
    let previous = app.state::<AppState>().prev_app_pid.lock().unwrap().take();
    if let (true, Some(pid)) = (restore_focus, previous) {
        source_app::activate(pid);
    }
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
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let (w, h) = ((TOAST_WIDTH * scale) as i32, (TOAST_HEIGHT * scale) as i32);
        let _ = toast.set_size(PhysicalSize::new(w as u32, h as u32));
        let _ = toast.set_position(PhysicalPosition::new(
            area.position.x + (area.size.width as i32 - w) / 2,
            area.position.y + area.size.height as i32 - h - (TOAST_BOTTOM * scale) as i32,
        ));
    }
    let _ = app.emit_to(TOAST, "toast://show", payload);
    let _ = toast.set_ignore_cursor_events(true);
    let _ = toast.show();

    let generation = TOAST_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(TOAST_MS));
        // A newer toast restarted the timer; let it hide the window.
        if TOAST_GENERATION.load(Ordering::SeqCst) == generation {
            if let Some(toast) = app.get_webview_window(TOAST) {
                let _ = toast.hide();
            }
        }
    });
}

fn write_clipboard(state: &AppState, content: &ClipContent) -> Result<(), String> {
    use clipboard_rs::{Clipboard, ClipboardContext, RustImageData, common::RustImage};
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
    state.suppress_watcher();
    match content {
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
    .map_err(|e| e.to_string())
}

/// Copies a clip back to the clipboard, closes the panel and confirms with a toast.
pub fn copy_clip(app: &AppHandle, id: i64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let content = state.store.lock().unwrap().content(id).map_err(|e| e.to_string())?;
    let result = match content {
        Some(content) => write_clipboard(&state, &content).map(|()| content),
        None => Err("clip no longer exists".into()),
    };
    hide_panel(app, true);
    match result {
        Ok(content) => {
            let sound_on = {
                let store = state.store.lock().unwrap();
                let _ = store.touch(id, now_ms());
                crate::settings::copy_sound_enabled(&store)
            };
            if sound_on {
                source_app::play_copy_sound();
            }
            let _ = app.emit("clips://changed", ());
            show_toast(app, ToastPayload::copied(&content));
            Ok(())
        }
        Err(e) => {
            log::warn!("copy failed: {e}");
            show_toast(app, ToastPayload::failed());
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_preview_collapses_whitespace_and_caps_length() {
        assert_eq!(toast_preview("a\n   b\tc"), "a b c");
        let long = "x".repeat(41);
        assert_eq!(toast_preview(&long), format!("{}…", "x".repeat(40)));
        assert_eq!(toast_preview(&"가".repeat(40)), "가".repeat(40));
    }
}
