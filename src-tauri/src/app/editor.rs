//! The image editor window: holds the image being edited, renders op lists and copies the result to history.

use crate::{
    app::{
        ai,
        state::{AppState, now_ms},
        watcher::{MAX_IMAGE_BYTES, THUMB_SIZE},
        window,
    },
    domain::{
        clip::{self, NewClip},
        edit::{self, Op, PREVIEW_SIDE},
        source_app,
    },
    shared::{
        error::{AppError, AppResult},
        png,
    },
};
use ::image::DynamicImage;
use serde::Serialize;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

pub const EDITOR: &str = "editor";

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// The image open in the editor, decoded into memory so deleting its clip doesn't break it.
pub struct Session {
    id: u64,
    source: Arc<DynamicImage>,
    /// AI results keyed by `ai_key`.
    // ponytail: cache only grows within one session; cleared on open/close.
    cache: Mutex<HashMap<u64, Arc<DynamicImage>>>,
}

impl Session {
    fn new(source: DynamicImage) -> Self {
        Self { id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed), source: Arc::new(source), cache: Mutex::default() }
    }

    fn load(path: &Path) -> AppResult<Self> {
        let img = ::image::open(path).map_err(|e| AppError::InvalidInput(format!("can't open the image: {e}")))?;
        Ok(Self::new(img))
    }
}

/// The open session, if any. A render works on its own `Arc`, so closing never waits for one.
#[derive(Default)]
pub struct EditorState(Mutex<Option<Arc<Session>>>);

impl EditorState {
    fn set(&self, session: Option<Session>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = session.map(Arc::new);
    }

    fn current(&self) -> AppResult<Arc<Session>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or_else(|| AppError::InvalidInput("no image is open".into()))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditImageDto {
    /// Full-resolution size; the data URL may be smaller.
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

fn ai_key(session: &Session, prefix: &[Op]) -> u64 {
    let mut h = DefaultHasher::new();
    (session.id, prefix).hash(&mut h);
    h.finish()
}

/// Applies `ops` to the session's source in order. AI ops go through `ai` and are cached by the ops before them.
fn render(
    session: &Session,
    ops: &[Op],
    ai: impl Fn(&DynamicImage, &Op) -> AppResult<DynamicImage>,
) -> AppResult<DynamicImage> {
    let mut img = session.source.clone();
    for (i, op) in ops.iter().enumerate() {
        if img.width() == 0 || img.height() == 0 {
            return Err(AppError::InvalidInput("empty image".into()));
        }
        img = match op {
            Op::RemoveBg | Op::Upscale { .. } => {
                if let Op::Upscale { scale } = op
                    && !matches!(scale, 2 | 4)
                {
                    return Err(AppError::InvalidInput(format!("bad upscale {scale}")));
                }
                let key = ai_key(session, &ops[..=i]);
                let hit = session.cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key).cloned();
                match hit {
                    Some(done) => done,
                    None => {
                        let done = Arc::new(ai(&img, op)?);
                        session.cache.lock().unwrap_or_else(|e| e.into_inner()).insert(key, done.clone());
                        done
                    }
                }
            }
            _ => Arc::new(edit::apply_geometric(Arc::unwrap_or_clone(img), op)?),
        };
    }
    Ok(Arc::unwrap_or_clone(img))
}

/// `img` as a PNG fitted to `PREVIEW_SIDE`, with its full-resolution size.
fn preview(img: &DynamicImage) -> AppResult<EditImageDto> {
    let (w, h) = (img.width(), img.height());
    let (pw, ph) = edit::fit(w, h, PREVIEW_SIDE);
    let png = if (pw, ph) == (w, h) { edit::to_png(img)? } else { edit::to_png(&img.thumbnail_exact(pw, ph))? };
    Ok(EditImageDto { width: w, height: h, data_url: png::data_url(&png) })
}

/// Renders `session` on a blocking thread with the real AI ops.
async fn render_off_main(app: &AppHandle, session: Arc<Session>, ops: Vec<Op>) -> AppResult<DynamicImage> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        render(&session, &ops, |img, op| match op {
            Op::RemoveBg => ai::remove_background(&app, img),
            Op::Upscale { scale } => ai::upscale(&app, img, *scale),
            _ => Err(AppError::InvalidInput("not an AI op".into())),
        })
    })
    .await
    .map_err(join_err)?
}

fn join_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(e.to_string())
}

/// Opens `source` (any image the `image` crate reads here) in the editor, replacing any open image.
pub fn open(app: &AppHandle, source: PathBuf) -> AppResult<()> {
    app.state::<EditorState>().set(Some(Session::load(&source)?));
    let _ = app.emit_to(EDITOR, "editor://opened", ());
    if let Some(window) = app.get_webview_window(EDITOR) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    Ok(())
}

/// Drops the session and hides the window.
pub fn close(app: &AppHandle) {
    app.state::<EditorState>().set(None);
    if let Some(window) = app.get_webview_window(EDITOR) {
        let _ = window.hide();
    }
}

/// Whether `id` is still the open session, so a late copy doesn't close a newer image.
fn is_current(state: &EditorState, id: u64) -> bool {
    state.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| s.id == id)
}

/// Saves `img` as a new history clip and puts it on the clipboard.
/// If the clipboard write fails the clip stays in history: lists refresh and a failed toast shows.
/// The watcher's cap on one image, so an edit can't save what a copy couldn't.
fn check_png_size(len: usize) -> AppResult<()> {
    if len > MAX_IMAGE_BYTES {
        return Err(AppError::InvalidInput(format!("edited image is {len} bytes, over the {MAX_IMAGE_BYTES} byte limit")));
    }
    Ok(())
}

fn copy_to_history(app: &AppHandle, img: &DynamicImage) -> AppResult<(i64, clip::ClipContent)> {
    let png = edit::to_png(img)?;
    check_png_size(png.len())?;
    let thumb_png = edit::to_png(&img.thumbnail(THUMB_SIZE, THUMB_SIZE))?;
    let state = app.state::<AppState>();
    let (id, content) = {
        let conn = state.db.lock().unwrap();
        let clip = NewClip::Image { png, thumb_png, width: img.width(), height: img.height() };
        // An edit is Vee's own clip, so its card shows Vee as the source app.
        let vee = source_app::upsert_app(&conn, &app.config().identifier, &app.package_info().name, Some(window::DRAG_ICON))?;
        let id = clip::upsert(&conn, &state.images_dir, clip, Some(vee), now_ms())?;
        (id, clip::content(&conn, id)?.ok_or_else(|| AppError::Other("clip no longer exists".into()))?)
    };
    if let Err(e) = window::write_clipboard(&state, &content, Vec::new()) {
        log::warn!("editor copy failed: {e}");
        let _ = app.emit("clips://changed", ());
        window::show_toast(app, window::ToastPayload::failed());
        return Err(e);
    }
    Ok((id, content))
}

pub mod commands {
    use super::{EditImageDto, EditorState, join_err};
    use crate::{domain::edit::Op, shared::error::AppResult};
    use tauri::{AppHandle, Manager, State};

    /// Async so encoding the preview stays off the main thread.
    #[tauri::command(async)]
    pub fn get_edit_source(state: State<EditorState>) -> AppResult<EditImageDto> {
        super::preview(&state.current()?.source)
    }

    /// Full-resolution render; the preview is fitted to `PREVIEW_SIDE`.
    #[tauri::command]
    pub async fn render_edit(app: AppHandle, ops: Vec<Op>) -> AppResult<EditImageDto> {
        let session = app.state::<EditorState>().current()?;
        let img = super::render_off_main(&app, session, ops).await?;
        tauri::async_runtime::spawn_blocking(move || super::preview(&img)).await.map_err(join_err)?
    }

    /// Renders, saves a new clip, copies it and closes the editor (unless another image opened meanwhile).
    #[tauri::command]
    pub async fn copy_edit(app: AppHandle, ops: Vec<Op>) -> AppResult<()> {
        let session = app.state::<EditorState>().current()?;
        let id = session.id;
        let img = super::render_off_main(&app, session, ops).await?;
        let handle = app.clone();
        let (clip_id, content) =
            tauri::async_runtime::spawn_blocking(move || super::copy_to_history(&handle, &img)).await.map_err(join_err)??;
        let main = app.clone();
        // The copy sound is AppKit, which belongs on the main thread.
        app.run_on_main_thread(move || {
            crate::app::window::confirm_copy(&main, clip_id, &content, false);
            if super::is_current(&main.state::<EditorState>(), id) {
                super::close(&main);
            }
        })
        .map_err(join_err)
    }

    #[tauri::command]
    pub fn close_editor(app: AppHandle) {
        super::close(&app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::RgbaImage;
    use std::cell::Cell;

    fn blank(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn rejects_an_edit_over_the_image_cap() {
        assert!(check_png_size(MAX_IMAGE_BYTES).is_ok());
        assert!(matches!(check_png_size(MAX_IMAGE_BYTES + 1), Err(AppError::InvalidInput(_))));
    }

    /// A fake AI op that counts its calls and returns the input unchanged.
    fn counting(calls: &Cell<u32>) -> impl Fn(&DynamicImage, &Op) -> AppResult<DynamicImage> + '_ {
        move |img, _| {
            calls.set(calls.get() + 1);
            Ok(img.clone())
        }
    }

    #[test]
    fn geometric_ops_apply_in_order() {
        let s = Session::new(blank(100, 50));
        let ops = [Op::Crop { x: 0, y: 0, w: 50, h: 50 }, Op::Rotate { quarter: 1 }, Op::Resize { w: 20, h: 20 }];
        let out = render(&s, &ops, |_, _| panic!("no AI op")).unwrap();
        assert_eq!((out.width(), out.height()), (20, 20));
        // Cropping before the rotate would give 50×50.
        let out = render(&s, &[Op::Rotate { quarter: 1 }, Op::Crop { x: 0, y: 0, w: 50, h: 80 }], |_, _| panic!()).unwrap();
        assert_eq!((out.width(), out.height()), (50, 80));
    }

    #[test]
    fn ai_result_is_cached_for_the_same_prefix() {
        let s = Session::new(blank(10, 10));
        let calls = Cell::new(0);
        let ops = vec![Op::Crop { x: 0, y: 0, w: 8, h: 8 }, Op::RemoveBg];
        render(&s, &ops, counting(&calls)).unwrap();
        render(&s, &ops, counting(&calls)).unwrap();
        assert_eq!(calls.get(), 1);
        let mut more = ops.clone();
        more.push(Op::Rotate { quarter: 1 });
        render(&s, &more, counting(&calls)).unwrap();
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn changed_prefix_reruns_ai() {
        let s = Session::new(blank(10, 10));
        let calls = Cell::new(0);
        render(&s, &[Op::Crop { x: 0, y: 0, w: 8, h: 8 }, Op::RemoveBg], counting(&calls)).unwrap();
        render(&s, &[Op::Crop { x: 1, y: 0, w: 8, h: 8 }, Op::RemoveBg], counting(&calls)).unwrap();
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn bad_upscale_and_empty_images_are_invalid_input() {
        let s = Session::new(blank(10, 10));
        let calls = Cell::new(0);
        assert!(matches!(render(&s, &[Op::Upscale { scale: 3 }], counting(&calls)), Err(AppError::InvalidInput(_))));
        assert_eq!(calls.get(), 0);
        let empty = Session::new(blank(0, 5));
        assert!(matches!(render(&empty, &[Op::RemoveBg], counting(&calls)), Err(AppError::InvalidInput(_))));
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn preview_is_downsized() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let dto = preview(&blank(6016, 3384)).unwrap();
        assert_eq!((dto.width, dto.height), (6016, 3384));
        let png = STANDARD.decode(dto.data_url.strip_prefix("data:image/png;base64,").unwrap()).unwrap();
        let shown = ::image::load_from_memory(&png).unwrap();
        assert_eq!((shown.width(), shown.height()), (2048, 1152));
        let wire = serde_json::to_value(preview(&blank(4, 3)).unwrap()).unwrap();
        assert_eq!((wire["width"].as_u64(), wire["height"].as_u64()), (Some(4), Some(3)));
        assert!(wire["dataUrl"].as_str().unwrap().starts_with("data:image/png;base64,"));
    }

    fn write_png(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
        let path = dir.join(name);
        blank(w, h).save(&path).unwrap();
        path
    }

    #[test]
    fn open_replaces_session() {
        let dir = tempfile::tempdir().unwrap();
        let state = EditorState::default();
        state.set(Some(Session::load(&write_png(dir.path(), "a.png", 30, 20)).unwrap()));
        let first = state.current().unwrap();
        render(&first, &[Op::RemoveBg], |img, _| Ok(img.clone())).unwrap();
        assert_eq!(first.cache.lock().unwrap().len(), 1);

        state.set(Some(Session::load(&write_png(dir.path(), "b.png", 7, 9)).unwrap()));
        let second = state.current().unwrap();
        assert_ne!(first.id, second.id);
        assert!(second.cache.lock().unwrap().is_empty());
        assert_eq!((second.source.width(), second.source.height()), (7, 9));

        assert!(!is_current(&state, first.id) && is_current(&state, second.id));

        state.set(None);
        assert!(matches!(state.current(), Err(AppError::InvalidInput(_))));
        assert!(!is_current(&state, second.id));
    }

    #[test]
    fn session_keeps_source_after_clip_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_png(dir.path(), "clip.png", 12, 6);
        let s = Session::load(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let out = render(&s, &[Op::Rotate { quarter: -1 }], |_, _| panic!()).unwrap();
        assert_eq!((out.width(), out.height()), (6, 12));
    }
}
