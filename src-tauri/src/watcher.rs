//! Watches the system clipboard and records every change in the store.

use crate::{AppState, now_ms, source_app, store::{NewClip, STACK_LAYERS}};
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, ContentFormat,
    RustImageData, common::RustImage,
};
use tauri::{AppHandle, Emitter, Manager};

/// Pasteboard markers set by password managers and other privacy-aware apps.
const CONCEALED_FORMATS: [&str; 3] = [
    "org.nspasteboard.ConcealedType",
    "org.nspasteboard.TransientType",
    "ExcludeClipboardContentFromMonitorProcessing",
];
const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024;
const MAX_FORMATS_BYTES: usize = 50 * 1024 * 1024;
const THUMB_SIZE: u32 = 320;

pub fn text_clip(text: String) -> Option<NewClip> {
    (!text.trim().is_empty()).then_some(NewClip::Text(text))
}

const IMAGE_EXTS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

pub fn is_image_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
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

struct Handler {
    app: AppHandle,
    ctx: ClipboardContext,
}

impl Handler {
    fn capture(&self) -> crate::store::Result<()> {
        let state = self.app.state::<AppState>();
        if state.watcher_suppressed() {
            return Ok(());
        }
        let Some(clip) = read(&self.ctx)? else { return Ok(()) };
        // File clips are restored from their paths, which handles multiple files.
        let formats = if matches!(clip, NewClip::Files { .. }) { Vec::new() } else { snapshot(&self.ctx) };
        let front = source_app::frontmost();
        let store = state.store.lock().unwrap();
        let app_id = match front {
            Some(front) => {
                let icon = if store.app_known(&front.bundle_id)? { None } else { source_app::icon_png(&front) };
                Some(store.upsert_app(&front.bundle_id, &front.name, icon.as_deref())?)
            }
            None => None,
        };
        let id = store.upsert(clip, app_id, now_ms())?;
        store.set_formats(id, &formats)?;
        let sound = crate::settings::copy_sound(&store);
        drop(store);
        if let Some(name) = sound {
            // AppKit sound playback belongs on the main thread.
            self.app.run_on_main_thread(move || crate::sound::play(&name))?;
        }
        self.app.emit("clips://changed", ())?;
        Ok(())
    }
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) {
        // This thread has no run loop, so nothing drains autoreleased Cocoa objects for it:
        // without a pool every pasteboard buffer read here (an image is ~2MB) is kept forever.
        #[cfg(target_os = "macos")]
        let result = objc2::rc::autoreleasepool(|_| self.capture());
        #[cfg(not(target_os = "macos"))]
        let result = self.capture();
        if let Err(e) = result {
            log::warn!("clipboard capture failed: {e}");
        }
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let (ctx, mut watcher) = match (ClipboardContext::new(), ClipboardWatcherContext::new()) {
            (Ok(ctx), Ok(watcher)) => (ctx, watcher),
            (Err(e), _) | (_, Err(e)) => return log::error!("clipboard watcher unavailable: {e}"),
        };
        watcher.add_handler(Handler { app, ctx });
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
