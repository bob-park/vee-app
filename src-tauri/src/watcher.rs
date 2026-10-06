//! Watches the system clipboard and records every change in the store.

use crate::{AppState, now_ms, source_app, store::NewClip};
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, ContentFormat,
    common::RustImage,
};
use tauri::{AppHandle, Emitter, Manager};

/// Pasteboard markers set by password managers and other privacy-aware apps.
const CONCEALED_FORMATS: [&str; 3] = [
    "org.nspasteboard.ConcealedType",
    "org.nspasteboard.TransientType",
    "ExcludeClipboardContentFromMonitorProcessing",
];
const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024;
const THUMB_SIZE: u32 = 320;

pub fn text_clip(text: String) -> Option<NewClip> {
    (!text.trim().is_empty()).then_some(NewClip::Text(text))
}

/// Files win over images, images over text: copying a file in Finder also
/// puts its icon and name on the pasteboard.
fn read(ctx: &ClipboardContext) -> clipboard_rs::Result<Option<NewClip>> {
    if CONCEALED_FORMATS.iter().any(|f| ctx.has(ContentFormat::Other(f.to_string()))) {
        return Ok(None);
    }
    if ctx.has(ContentFormat::Files) {
        let files = ctx.get_files()?;
        if !files.is_empty() {
            return Ok(Some(NewClip::Files(files)));
        }
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
    if ctx.has(ContentFormat::Text) {
        return Ok(text_clip(ctx.get_text()?));
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
        let front = source_app::frontmost();
        let store = state.store.lock().unwrap();
        let app_id = match front {
            Some(front) => {
                let icon = if store.app_known(&front.bundle_id)? { None } else { source_app::icon_png(&front) };
                Some(store.upsert_app(&front.bundle_id, &front.name, icon.as_deref())?)
            }
            None => None,
        };
        store.upsert(clip, app_id, now_ms())?;
        drop(store);
        self.app.emit("clips://changed", ())?;
        Ok(())
    }
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) {
        if let Err(e) = self.capture() {
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
