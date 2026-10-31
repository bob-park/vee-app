//! Watches the system clipboard and records every change in the clip history.

use crate::{
    app::{
        front_app,
        state::{AppState, now_ms},
    },
    domain::{
        clip::{self, NewClip, STACK_LAYERS},
        settings, source_app,
    },
};
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, ContentFormat, RustImageData,
    common::RustImage,
};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Pasteboard markers set by password managers and other privacy-aware apps.
const CONCEALED_FORMATS: [&str; 3] =
    ["org.nspasteboard.ConcealedType", "org.nspasteboard.TransientType", "ExcludeClipboardContentFromMonitorProcessing"];
pub(crate) const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024;
const MAX_FORMATS_BYTES: usize = 50 * 1024 * 1024;
pub(crate) const THUMB_SIZE: u32 = 320;

pub fn text_clip(text: String) -> Option<NewClip> {
    (!text.trim().is_empty()).then_some(NewClip::Text(text))
}

const IMAGE_EXTS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

pub fn is_image_path(path: &str) -> bool {
    std::path::Path::new(path).extension().and_then(|e| e.to_str()).is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

fn thumb_png(path: &str) -> Option<Vec<u8>> {
    let image = RustImageData::from_path(path).ok()?;
    Some(image.thumbnail(THUMB_SIZE, THUMB_SIZE).ok()?.to_png().ok()?.get_bytes().to_vec())
}

/// Thumbnails of image files among the first `STACK_LAYERS`; undecodable files are skipped.
fn file_thumbs(paths: &[String]) -> Vec<(usize, Vec<u8>)> {
    paths
        .iter()
        .take(STACK_LAYERS)
        .enumerate()
        .filter(|(_, p)| is_image_path(p))
        .filter(|(_, p)| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= MAX_IMAGE_BYTES as u64))
        .filter_map(|(i, p)| thumb_png(p).map(|png| (i, png)))
        .collect()
}

/// Every raw representation on the pasteboard, so a copy back pastes like the original.
/// Empty when the total exceeds `MAX_FORMATS_BYTES`; the clip then falls back to its card content.
#[cfg(target_os = "macos")]
fn snapshot(ctx: &ClipboardContext) -> Vec<(String, Vec<u8>)> {
    let mut formats = Vec::new();
    let mut total = 0;
    for format in ctx.available_formats().unwrap_or_default() {
        let Ok(data) = ctx.get_buffer(&format) else { continue };
        total += data.len();
        if total > MAX_FORMATS_BYTES {
            log::info!("not keeping raw formats larger than 50MB");
            return Vec::new();
        }
        formats.push((format, data));
    }
    formats
}

// ponytail: Windows keeps only the card content; standard CF_* formats and GDI handles
// don't round-trip through raw buffers, so add a per-format allowlist if it's needed there.
#[cfg(not(target_os = "macos"))]
fn snapshot(_ctx: &ClipboardContext) -> Vec<(String, Vec<u8>)> {
    Vec::new()
}

/// Picks what the card shows. Files win over text, text over images: copying a file
/// in Finder also puts its icon and name on the pasteboard, and Office apps add a
/// picture of the copied cells or text.
fn read(ctx: &ClipboardContext) -> clipboard_rs::Result<Option<NewClip>> {
    if CONCEALED_FORMATS.iter().any(|f| ctx.has(ContentFormat::Other(f.to_string()))) {
        return Ok(None);
    }
    if ctx.has(ContentFormat::Files) {
        let files = ctx.get_files()?;
        if !files.is_empty() {
            let thumbs = file_thumbs(&files);
            return Ok(Some(NewClip::Files { paths: files, thumbs }));
        }
    }
    if ctx.has(ContentFormat::Text)
        && let Some(clip) = text_clip(ctx.get_text()?)
    {
        return Ok(Some(clip));
    }
    if ctx.has(ContentFormat::Image) {
        let image = ctx.get_image()?;
        let png = image.to_png()?.get_bytes().to_vec();
        if png.len() > MAX_IMAGE_BYTES {
            log::info!("skipping image larger than 50MB");
            return Ok(None);
        }
        let (width, height) = image.get_size();
        let thumb_png = image.thumbnail(THUMB_SIZE, THUMB_SIZE)?.to_png()?.get_bytes().to_vec();
        return Ok(Some(NewClip::Image { png, thumb_png, width, height }));
    }
    Ok(None)
}

/// How long the clipboard must stay unchanged before it's recorded. Terminals with
/// copy-on-select rewrite it continuously while a selection is dragged.
const QUIET: Duration = Duration::from_millis(300);
/// How often clipboard-rs checks the macOS change count (ignored on Windows).
const POLL: Duration = Duration::from_millis(200);

/// Blocks until no change arrives for `quiet`. False when the watcher is gone.
fn wait_quiet(rx: &Receiver<()>, quiet: Duration) -> bool {
    loop {
        match rx.recv_timeout(quiet) {
            Ok(()) => continue,
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

fn capture(app: &AppHandle, ctx: &ClipboardContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = app.state::<AppState>();
    if state.watcher_suppressed() {
        return Ok(());
    }
    let Some(clip) = read(ctx)? else { return Ok(()) };
    // File clips are restored from their paths, which handles multiple files.
    let formats = if matches!(clip, NewClip::Files { .. }) { Vec::new() } else { snapshot(ctx) };
    let front = front_app::frontmost();
    let conn = state.db.lock().unwrap();
    let app_id = match front {
        Some(front) => {
            if source_app::is_excluded(&conn, &front.bundle_id)? {
                return Ok(());
            }
            let icon = if source_app::app_known(&conn, &front.bundle_id)? { None } else { front_app::icon_png(&front) };
            Some(source_app::upsert_app(&conn, &front.bundle_id, &front.name, icon.as_deref())?)
        }
        None => None,
    };
    let id = clip::upsert(&conn, &state.images_dir, clip, app_id, now_ms())?;
    clip::set_formats(&conn, id, &formats)?;
    let sound = settings::copy_sound(&conn);
    drop(conn);
    if let Some(name) = sound {
        // AppKit sound playback belongs on the main thread.
        app.run_on_main_thread(move || crate::app::sound::play(&name))?;
    }
    app.emit("clips://added", ())?;
    crate::app::ocr::wake();
    Ok(())
}

/// Records the clipboard once changes have settled for `QUIET`.
fn record(app: AppHandle, rx: Receiver<()>) {
    let ctx = match ClipboardContext::new() {
        Ok(ctx) => ctx,
        Err(e) => return log::error!("clipboard unavailable: {e}"),
    };
    while rx.recv().is_ok() && wait_quiet(&rx, QUIET) {
        // This thread has no run loop, so nothing drains autoreleased Cocoa objects for it:
        // without a pool every pasteboard buffer read here (an image is ~2MB) is kept forever.
        #[cfg(target_os = "macos")]
        let result = objc2::rc::autoreleasepool(|_| capture(&app, &ctx));
        #[cfg(not(target_os = "macos"))]
        let result = capture(&app, &ctx);
        if let Err(e) = result {
            log::warn!("clipboard capture failed: {e}");
        }
    }
}

struct Handler {
    app: AppHandle,
    tx: Sender<()>,
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) {
        // Our own writes are dropped here as well as at record time, so they don't
        // hold back recording of a real copy made right after.
        if !self.app.state::<AppState>().watcher_suppressed() {
            let _ = self.tx.send(());
        }
    }
}

pub fn spawn(app: AppHandle) {
    let (tx, rx) = channel();
    let recorder = app.clone();
    std::thread::spawn(move || record(recorder, rx));
    std::thread::spawn(move || {
        let mut watcher = match ClipboardWatcherContext::new_with_interval(POLL) {
            Ok(watcher) => watcher,
            Err(e) => return log::error!("clipboard watcher unavailable: {e}"),
        };
        watcher.add_handler(Handler { app, tx });
        watcher.start_watch();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid 1×1 PNG.
    const PIXEL_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

    #[test]
    fn recognises_image_extensions_case_insensitively() {
        assert!(is_image_path("/a/Shot.PNG"));
        assert!(is_image_path("C:\\pics\\a.jpeg"));
        assert!(is_image_path("/a/b.webp"));
        assert!(!is_image_path("/a/report.pdf"));
        assert!(!is_image_path("/a/README"));
    }

    #[test]
    fn thumbnails_only_decodable_images_among_the_first_three() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.PNG");
        std::fs::write(&png, STANDARD.decode(PIXEL_PNG).unwrap()).unwrap();
        let txt = dir.path().join("b.txt");
        std::fs::write(&txt, "x").unwrap();
        let broken = dir.path().join("c.jpg");
        std::fs::write(&broken, "not an image").unwrap();
        let fourth = dir.path().join("d.png");
        std::fs::copy(&png, &fourth).unwrap();
        let p = |p: &std::path::Path| p.to_string_lossy().into_owned();
        let thumbs = file_thumbs(&[p(&png), p(&txt), p(&broken), p(&fourth)]);
        assert_eq!(thumbs.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![0]);
        assert!(thumbs[0].1.starts_with(b"\x89PNG"));
    }

    #[test]
    fn waits_until_changes_stop_for_the_quiet_period() {
        let (tx, rx) = std::sync::mpsc::channel();
        let start = std::time::Instant::now();
        std::thread::spawn(move || {
            for _ in 0..4 {
                std::thread::sleep(Duration::from_millis(150));
                tx.send(()).unwrap();
            }
            std::thread::sleep(Duration::from_secs(2)); // keep the sender alive
        });
        assert!(wait_quiet(&rx, Duration::from_millis(300)));
        let waited = start.elapsed();
        assert!(waited >= Duration::from_millis(900), "{waited:?}"); // 4×150 + 300
        assert!(waited < Duration::from_millis(1500), "{waited:?}");
    }

    #[test]
    fn stops_waiting_when_the_watcher_is_gone() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        drop(tx);
        assert!(!wait_quiet(&rx, Duration::from_millis(300)));
    }

    #[test]
    fn whitespace_only_text_is_ignored() {
        assert!(text_clip("   \n\t ".into()).is_none());
        assert!(text_clip(String::new()).is_none());
    }

    #[test]
    fn text_is_kept_verbatim() {
        match text_clip("  hi \n".into()) {
            Some(NewClip::Text(t)) => assert_eq!(t, "  hi \n"),
            _ => panic!("expected text clip"),
        }
    }
}
