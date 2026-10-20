//! Drag-to-convert: the convert-key watch during a card drag, the `convert` overlay window, and the job worker.

use crate::{
    app::{
        drop_target::{self, DropFolder},
        engines, office,
        state::{AppState, now_ms},
        window::{self, PANEL, Rect, ToastPayload, USES_POINTS},
    },
    domain::{
        clip::Kind,
        convert::{self, Caps, Format, Job, JobError, JobState, JobView, Quality, Run, Slice},
        settings,
    },
    shared::error::{AppError, AppResult},
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const CONVERT: &str = "convert";
/// Overlay window side in design points.
const SIZE: f64 = 260.0;
const KEY_POLL_MS: u64 = 30;
const TICK_MS: u64 = 500;

/// What a toast says about converting, alongside `ToastPayload`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ConvertNote {
    Saved { format: Format, count: usize, folder: String },
    Failed { name: String, error: JobError },
    NotFolder { finder_blocked: bool },
    Unsupported,
}

/// The card being dragged. Survives `finish_drag` so the overlay's drop (which arrives later) can take it.
struct Session {
    /// The drag it belongs to (`Convert::drag`); a stale key watch leaves a newer session alone.
    generation: u64,
    clip_id: i64,
    files: Vec<PathBuf>,
    slices: Vec<Slice>,
    /// The folder under the last key-down; set when the overlay opens.
    dir: Option<PathBuf>,
    /// A "not a folder" / "unsupported" toast was already shown this drag.
    noted: bool,
    /// `finish_drag` saw the drag end on the open overlay, so `convert_drop` may take it.
    dropped: bool,
}

/// The session of drag `generation`, if it is still the current one.
fn of_drag(slot: &mut Option<Session>, generation: u64) -> Option<&mut Session> {
    slot.as_mut().filter(|s| s.generation == generation)
}

/// After a drag ends: keep the session for the overlay's drop only if it landed there with a folder.
fn mark_drop(slot: &mut Option<Session>, on_overlay: bool) {
    match slot.as_mut() {
        Some(s) if on_overlay && s.dir.is_some() => s.dropped = true,
        _ => *slot = None,
    }
}

/// Takes the session for a drop on the slice with id `format`; anything else leaves it alone.
/// A slice that still needs an engine is rejected, like one that isn't there.
fn take_drop(slot: &mut Option<Session>, format: &str) -> AppResult<(Session, Format, PathBuf)> {
    let Some(s) = slot.as_ref().filter(|s| s.dropped && s.dir.is_some()) else {
        return Err(AppError::InvalidInput("no convert drop pending".into()));
    };
    let Some(format) = s.slices.iter().find(|x| x.format.id() == format && x.needs.is_none()).map(|x| x.format) else {
        return Err(AppError::InvalidInput(format!("can't convert to {format}")));
    };
    let mut s = slot.take().expect("checked above");
    let dir = s.dir.take().expect("checked above");
    Ok((s, format, dir))
}

#[derive(Default)]
struct Jobs {
    list: Vec<Job>,
    next_id: u64,
    /// A thread re-publishes the list every `TICK_MS`.
    ticking: bool,
}

pub struct Convert {
    session: Mutex<Option<Session>>,
    /// Bumped by every drag, so an older key watch stops.
    drag: AtomicU64,
    jobs: Mutex<Jobs>,
    queue: Sender<u64>,
    /// `<temp>/vee-convert-<identifier>`, so a dev build and the installed app don't share it.
    tmp: PathBuf,
}

/// How long app exit waits for a cancelled video job to stop FFmpeg (one poll plus the kill).
const EXIT_WAIT: Duration = Duration::from_secs(2);

/// Manages the convert state and starts the worker. Call once in setup.
/// Clears temp output a previous run left behind.
pub fn spawn(app: &AppHandle) {
    let (queue, rx) = mpsc::channel();
    let tmp = std::env::temp_dir().join(format!("vee-convert-{}", app.config().identifier));
    if tmp.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    app.manage(Convert { session: Mutex::new(None), drag: AtomicU64::new(0), jobs: Mutex::default(), queue, tmp });
    let app = app.clone();
    // ponytail: one worker for all images; a second worker if batches feel slow.
    std::thread::spawn(move || {
        for id in rx {
            work(&app, id);
        }
    });
}

/// Arms the convert key for a drag of `files` (an image clip's staged PNG, or a files clip's paths).
/// Call after `dragging` is set and before `drag::start_drag`, which blocks on Windows.
pub fn begin(app: &AppHandle, clip_id: i64, files: Vec<PathBuf>) {
    let c = app.state::<Convert>();
    let generation = c.drag.fetch_add(1, Ordering::SeqCst) + 1;
    let state = app.state::<AppState>();
    let key = {
        let conn = state.db.lock().unwrap();
        (settings::get(&conn, "dragConvert") != "off").then(|| settings::get(&conn, "convertKey"))
    };
    let Some(key) = key else {
        *c.session.lock().unwrap() = None;
        return;
    };
    let caps = Caps {
        heic: cfg!(target_os = "macos"),
        ffmpeg: engines::ffmpeg(app).is_some(),
        rhwp: engines::rhwp(app).is_some(),
        libreoffice: office::soffice().is_some(),
        textutil: cfg!(target_os = "macos"),
    };
    let slices = convert::formats_for(&files, caps);
    *c.session.lock().unwrap() = Some(Session { generation, clip_id, files, slices, dir: None, noted: false, dropped: false });
    let app = app.clone();
    std::thread::spawn(move || {
        let (c, state) = (app.state::<Convert>(), app.state::<AppState>());
        // A key already held when the drag starts waits for a fresh press.
        let mut held = drop_target::key_down(&key);
        while state.dragging.load(Ordering::SeqCst) && c.drag.load(Ordering::SeqCst) == generation {
            let down = drop_target::key_down(&key);
            if down != held {
                held = down;
                if down { open_at(&app, generation) } else { close(&app) }
            }
            std::thread::sleep(Duration::from_millis(KEY_POLL_MS));
        }
    });
}

/// Looks up the folder under the cursor and opens the overlay there, or says why not (once per drag).
fn open_at(app: &AppHandle, generation: u64) {
    let Some(point) = window::cursor_at(app) else { return };
    let c = app.state::<Convert>();
    let unsupported = of_drag(&mut c.session.lock().unwrap(), generation).is_none_or(|s| s.slices.is_empty());
    let note = if unsupported {
        ConvertNote::Unsupported
    } else {
        match drop_target::folder_at(point, window::pinned(app)) {
            DropFolder::Folder(dir) => {
                let open = of_drag(&mut c.session.lock().unwrap(), generation).map(|s| {
                    s.dir = Some(dir);
                    Open { slices: s.slices.clone(), name: file_name(&s.files[0]) }
                });
                if let Some(open) = open {
                    show(app, generation, point, open);
                }
                return;
            }
            DropFolder::NotFolder => ConvertNote::NotFolder { finder_blocked: false },
            DropFolder::FinderBlocked => ConvertNote::NotFolder { finder_blocked: true },
        }
    };
    let (first, files) = match of_drag(&mut c.session.lock().unwrap(), generation) {
        Some(s) => (!std::mem::replace(&mut s.noted, true), s.files.len()),
        None => (false, 0),
    };
    if first {
        window::show_toast(app, ToastPayload::convert(note, kind_for(files)));
    }
}

#[derive(Clone, Serialize)]
struct Open {
    slices: Vec<Slice>,
    /// File name for the donut's centre (the first file of a files clip).
    name: String,
}

/// Drag `generation` is still going.
fn live(app: &AppHandle, generation: u64) -> bool {
    app.state::<AppState>().dragging.load(Ordering::SeqCst) && app.state::<Convert>().drag.load(Ordering::SeqCst) == generation
}

/// Places and shows the overlay on the main thread, where `finish_drag` also runs, so the
/// liveness check and the show can't interleave with its hide. The folder lookup stays off it.
fn show(app: &AppHandle, generation: u64, (x, y): (f64, f64), open: Open) {
    let main = app.clone();
    let _ = app.run_on_main_thread(move || {
        // A key press that lands after the drop must not bring the overlay back.
        if !live(&main, generation) {
            return;
        }
        let Some(overlay) = main.get_webview_window(CONVERT) else { return };
        // Positioning units per design point: 1 on macOS, the monitor scale on Windows (as for the toast).
        let Some(monitor) = window::cursor_monitor(&main) else { return };
        let (work, unit) = window::work_area(&monitor);
        let _ = window::place(&overlay, overlay_rect((x, y), SIZE * unit, work));
        let _ = main.emit_to(CONVERT, "convert://open", open);
        let _ = overlay.show();
    });
}

/// A `side`-square centred on `point`, shifted as little as needed to stay inside `work`.
fn overlay_rect((x, y): (f64, f64), side: f64, work: Rect) -> Rect {
    let fit = |p: f64, start: f64, len: f64| (p - side / 2.0).min(start + len - side).max(start);
    Rect { x: fit(x, work.x, work.w), y: fit(y, work.y, work.h), w: side, h: side }
}

fn close(app: &AppHandle) {
    let _ = app.emit_to(CONVERT, "convert://close", ());
    if let Some(overlay) = app.get_webview_window(CONVERT) {
        let _ = overlay.hide();
    }
}

/// Whether the drag just ended over the open overlay, by the live cursor. Always hides the overlay.
/// Keeps the session for the overlay's drop on a hit, and forgets it otherwise.
pub fn dropped_on_overlay(app: &AppHandle) -> bool {
    let hit = (|| {
        let overlay = app.get_webview_window(CONVERT)?;
        if !overlay.is_visible().ok()? {
            return None;
        }
        let (p, s, k) = (overlay.outer_position().ok()?, overlay.outer_size().ok()?, overlay.scale_factor().ok()?);
        Some(window::rect_from((p.x, p.y), (s.width, s.height), k, USES_POINTS).contains(window::cursor_at(app)?))
    })()
    .unwrap_or(false);
    close(app);
    mark_drop(&mut app.state::<Convert>().session.lock().unwrap(), hit);
    hit
}

/// Queues the dragged card for `format` into the folder the overlay opened over.
fn drop_on(app: &AppHandle, format: &str) -> AppResult<()> {
    let c = app.state::<Convert>();
    let (s, format, dir) = take_drop(&mut c.session.lock().unwrap(), format)?;
    let id = {
        let mut jobs = c.jobs.lock().unwrap();
        jobs.next_id += 1;
        let id = jobs.next_id;
        jobs.list.push(Job::new(id, s.clip_id, format, s.files, dir, now_ms()));
        id
    };
    let _ = c.queue.send(id);
    publish(app);
    Ok(())
}

/// Runs one queued job file by file, then ends it with a toast.
fn work(app: &AppHandle, id: u64) {
    let c = app.state::<Convert>();
    let picked = c.jobs.lock().unwrap().list.iter_mut().find(|j| j.id == id && j.state == JobState::Queued).map(|j| {
        j.start(now_ms());
        (j.files.clone(), j.format, j.dir.clone(), j.done, j.cancel.clone())
    });
    // Dismissed while queued.
    let Some((files, format, dir, start, cancel)) = picked else { return };
    publish(app);
    let tmp = &c.tmp;
    let (quality, hw_on) = {
        let state = app.state::<AppState>();
        let db = state.db.lock().unwrap();
        (Quality::parse(&settings::get(&db, "videoQuality")), settings::get(&db, "videoHwAccel") == "on")
    };
    let engine = engines::ffmpeg(app);
    let hw = engine.as_ref().filter(|_| hw_on).and_then(|(ffmpeg, _)| convert::hw_encoder(ffmpeg));
    // Stored only; the running tick publishes it, so FFmpeg's chatty output costs no emits.
    let mut progress = |p: Option<f64>| with_job(&c, id, |j| j.progress = p);
    let lo_profile = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|e| {
            log::warn!("convert: no app data dir ({e}); LibreOffice profile goes in {}", tmp.display());
            tmp.clone()
        })
        .join("lo-profile");
    let mut ctx = Run {
        ffmpeg: engine.as_ref().map(|(f, p)| (f.as_path(), p.as_path())),
        quality,
        hw,
        cancel: &cancel,
        rhwp: engines::rhwp(app),
        soffice: office::soffice(),
        lo_profile,
        progress: &mut progress,
    };
    let mut failed: Option<(JobError, &Path)> = None;
    // A retried job resumes at the file that failed; the ones before it are already saved.
    for (i, src) in files.iter().enumerate().skip(start) {
        let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Vee".into());
        let result = std::fs::create_dir_all(tmp)
            .map_err(|_| JobError::Write)
            .and_then(|()| convert::run(src, &stem, format, &dir, tmp, &mut ctx));
        if result == Err(JobError::Cancelled) {
            // Cancelled by the user: drop the job without a toast.
            c.jobs.lock().unwrap().list.retain(|j| j.id != id);
            publish(app);
            return;
        }
        if let Err(e) = result {
            log::warn!("convert job {id} failed on file {}: {e:?}", i + 1);
            failed = Some((e, src));
            break;
        }
        with_job(&c, id, |j| {
            j.done = i + 1;
            // A video's next file starts at 0 so the percent holds until ffprobe answers; images stay `None`.
            j.progress = j.progress.map(|_| 0.0);
        });
        publish(app);
    }
    let mut done = 0;
    with_job(&c, id, |j| {
        j.state = failed.map_or(JobState::Done, |(e, _)| JobState::Failed(e));
        j.ended = Some(now_ms());
        done = j.done;
    });
    publish(app);
    let note = match failed {
        Some((error, src)) => ConvertNote::Failed { name: file_name(src), error },
        None => ConvertNote::Saved { format, count: done, folder: file_name(&dir) },
    };
    window::show_toast(app, ToastPayload::convert(note, kind_for(files.len())));
}

/// Stops a running video job; the worker then drops it. Image jobs can't stop mid-file.
fn cancel(jobs: &Jobs, id: u64) -> AppResult<()> {
    let job = jobs.list.iter().find(|j| j.id == id && j.state == JobState::Running && j.files.iter().all(|f| convert::is_video(f)));
    let job = job.ok_or_else(|| AppError::InvalidInput(format!("no running video convert job {id}")))?;
    job.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// Drops queued jobs and cancels running ones; true while one is still running.
fn stop_all(jobs: &mut Jobs) -> bool {
    jobs.list.retain(|j| j.state != JobState::Queued);
    let running = jobs.list.iter().filter(|j| j.state == JobState::Running);
    running.inspect(|j| j.cancel.store(true, Ordering::SeqCst)).count() > 0
}

/// Stops the running job's FFmpeg and removes the temp output. Call on app exit.
/// An image job can't stop mid-file, so the wait is capped.
pub fn shutdown(app: &AppHandle) {
    let Some(c) = app.try_state::<Convert>() else { return };
    let until = std::time::Instant::now() + EXIT_WAIT;
    // The worker drops a cancelled job once FFmpeg is killed and reaped.
    while stop_all(&mut c.jobs.lock().unwrap()) && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    if c.tmp.exists() {
        let _ = std::fs::remove_dir_all(&c.tmp);
    }
}

fn with_job(c: &Convert, id: u64, f: impl FnOnce(&mut Job)) {
    if let Some(j) = c.jobs.lock().unwrap().list.iter_mut().find(|j| j.id == id) {
        f(j);
    }
}

/// The panel's view of the jobs; also forgets done jobs it no longer shows. Second: whether the
/// view can still change by itself (a job may cross the show mark or a done one expire).
fn snapshot(jobs: &mut Jobs) -> (Vec<JobView>, bool) {
    let now = now_ms();
    let views = convert::visible_jobs(&mut jobs.list, now);
    convert::prune(&mut jobs.list, now);
    // Failed jobs never change by themselves, so they need no ticks.
    let live = jobs.list.iter().any(|j| !matches!(j.state, JobState::Failed(_)));
    (views, live)
}

/// Sends the jobs to the panel, and keeps doing so every `TICK_MS` while they can change.
fn publish(app: &AppHandle) {
    let c = app.state::<Convert>();
    let (views, start_tick) = {
        let mut jobs = c.jobs.lock().unwrap();
        let (views, live) = snapshot(&mut jobs);
        let start = live && !jobs.ticking;
        jobs.ticking |= start;
        (views, start)
    };
    let _ = app.emit_to(PANEL, "convert://jobs", views);
    if !start_tick {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let c = app.state::<Convert>();
        loop {
            std::thread::sleep(Duration::from_millis(TICK_MS));
            let (views, live) = {
                let mut jobs = c.jobs.lock().unwrap();
                let (views, live) = snapshot(&mut jobs);
                // Decided under the lock, so a job added right now restarts the tick through `publish`.
                jobs.ticking = live;
                (views, live)
            };
            let _ = app.emit_to(PANEL, "convert://jobs", views);
            if !live {
                break;
            }
        }
    });
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The toast icon: one file reads as an image, several as files.
fn kind_for(files: usize) -> Kind {
    if files > 1 { Kind::Files } else { Kind::Image }
}

/// Convert commands. A submodule so they can share names with the functions above.
pub mod commands {
    use super::{Convert, cancel, publish, snapshot};
    use crate::domain::convert::{JobState, JobView};
    use crate::shared::error::{AppError, AppResult};
    use crate::app::state::now_ms;
    use tauri::{AppHandle, Manager, State};

    /// The overlay's drop on a slice.
    #[tauri::command]
    pub fn convert_drop(app: AppHandle, format: String) -> AppResult<()> {
        super::drop_on(&app, &format)
    }

    #[tauri::command]
    pub fn list_convert_jobs(state: State<Convert>) -> Vec<JobView> {
        snapshot(&mut state.jobs.lock().unwrap()).0
    }

    /// Queues a failed job again, same folder and format; it resumes at the file that failed.
    #[tauri::command]
    pub fn retry_convert(app: AppHandle, id: u64) -> AppResult<()> {
        let c = app.state::<Convert>();
        let found = c.jobs.lock().unwrap().list.iter_mut().find(|j| j.id == id && matches!(j.state, JobState::Failed(_))).map(|j| {
            j.state = JobState::Queued;
            j.since = now_ms();
            j.ended = None;
        });
        found.ok_or_else(|| AppError::InvalidInput(format!("no failed convert job {id}")))?;
        let _ = c.queue.send(id);
        publish(&app);
        Ok(())
    }

    /// Stops a running video job: FFmpeg is killed, its temp output removed, the job dropped without a toast.
    #[tauri::command]
    pub fn cancel_convert(state: State<Convert>, id: u64) -> AppResult<()> {
        cancel(&state.jobs.lock().unwrap(), id)
    }

    /// Removes a queued or failed job.
    #[tauri::command]
    pub fn dismiss_convert(app: AppHandle, id: u64) -> AppResult<()> {
        let c = app.state::<Convert>();
        let removed = {
            let mut jobs = c.jobs.lock().unwrap();
            let before = jobs.list.len();
            jobs.list.retain(|j| j.id != id || !matches!(j.state, JobState::Queued | JobState::Failed(_)));
            jobs.list.len() != before
        };
        if !removed {
            return Err(AppError::InvalidInput(format!("no queued or failed convert job {id}")));
        }
        publish(&app);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::convert::Engine;

    fn job(id: u64, state: JobState) -> Job {
        let now = now_ms();
        Job { state, ended: Some(now), shown: true, ..Job::new(id, 1, Format::Png, vec![], PathBuf::new(), now) }
    }

    #[test]
    fn only_running_video_jobs_cancel() {
        let mut video = job(1, JobState::Running);
        video.files = vec![PathBuf::from("/a/v.mp4")];
        let mut image = job(2, JobState::Running);
        image.files = vec![PathBuf::from("/a/p.png")];
        let mut queued = job(3, JobState::Queued);
        queued.files = vec![PathBuf::from("/a/v.mp4")];
        let jobs = Jobs { list: vec![video, image, queued], ..Default::default() };
        assert!(cancel(&jobs, 1).is_ok() && jobs.list[0].cancel.load(Ordering::SeqCst));
        assert!(matches!(cancel(&jobs, 2), Err(AppError::InvalidInput(_))));
        assert!(matches!(cancel(&jobs, 3), Err(AppError::InvalidInput(_))));
        assert!(!jobs.list[1].cancel.load(Ordering::SeqCst));
    }

    #[test]
    fn exit_drops_queued_jobs_and_cancels_running_ones() {
        let mut jobs = Jobs { list: vec![job(1, JobState::Running), job(2, JobState::Queued), job(3, JobState::Done)], ..Default::default() };
        assert!(stop_all(&mut jobs));
        assert_eq!(jobs.list.iter().map(|j| j.id).collect::<Vec<_>>(), [1, 3]);
        assert!(jobs.list[0].cancel.load(Ordering::SeqCst) && !jobs.list[1].cancel.load(Ordering::SeqCst));
        jobs.list.remove(0);
        assert!(!stop_all(&mut jobs));
    }

    #[test]
    fn ticks_only_while_a_job_can_still_change() {
        let mut jobs = Jobs { list: vec![job(1, JobState::Failed(JobError::Write))], ..Default::default() };
        assert!(!snapshot(&mut jobs).1);
        jobs.list.push(job(2, JobState::Done));
        let (views, live) = snapshot(&mut jobs);
        assert!(live && views.len() == 2);
        jobs.list[1].ended = Some(now_ms() - 5000);
        assert!(!snapshot(&mut jobs).1);
        assert_eq!(jobs.list.len(), 1);
    }

    fn session(generation: u64, dir: Option<&str>) -> Option<Session> {
        Some(Session {
            generation,
            clip_id: 1,
            files: vec![PathBuf::from("/a/p.png")],
            slices: vec![Slice { format: Format::Jpg, needs: None }],
            dir: dir.map(PathBuf::from),
            noted: false,
            dropped: false,
        })
    }

    #[test]
    fn overlay_stays_centred_unless_it_would_leave_the_work_area() {
        let work = Rect { x: 0.0, y: 25.0, w: 1000.0, h: 800.0 };
        assert_eq!(overlay_rect((500.0, 400.0), 260.0, work), Rect { x: 370.0, y: 270.0, w: 260.0, h: 260.0 });
        assert_eq!(overlay_rect((10.0, 30.0), 260.0, work), Rect { x: 0.0, y: 25.0, w: 260.0, h: 260.0 });
        assert_eq!(overlay_rect((995.0, 820.0), 260.0, work), Rect { x: 740.0, y: 565.0, w: 260.0, h: 260.0 });
        // A second monitor left of the primary.
        let left = Rect { x: -1440.0, y: 0.0, w: 1440.0, h: 900.0 };
        assert_eq!(overlay_rect((-5.0, 450.0), 260.0, left).x, -260.0);
    }

    #[test]
    fn a_stale_watch_leaves_a_newer_session_alone() {
        let mut slot = session(2, None);
        assert!(of_drag(&mut slot, 1).is_none());
        assert!(of_drag(&mut slot, 2).is_some());
    }

    #[test]
    fn only_a_drop_on_the_open_overlay_keeps_the_session() {
        let mut slot = session(1, Some("/out"));
        mark_drop(&mut slot, false);
        assert!(slot.is_none());
        // Never opened (no folder): nothing for convert_drop either.
        let mut slot = session(1, None);
        mark_drop(&mut slot, true);
        assert!(slot.is_none());
        let mut slot = session(1, Some("/out"));
        mark_drop(&mut slot, true);
        assert!(slot.as_ref().unwrap().dropped);
    }

    #[test]
    fn convert_drop_takes_only_a_dropped_session_with_that_format() {
        // The next drag's session, not dropped yet: a stray convert_drop must not take it.
        let mut slot = session(2, Some("/out"));
        assert!(take_drop(&mut slot, "jpg").is_err());
        assert!(slot.is_some());
        slot.as_mut().unwrap().dropped = true;
        assert!(take_drop(&mut slot, "png").is_err());
        assert!(take_drop(&mut slot, "nope").is_err());
        assert!(slot.is_some());
        let (s, format, dir) = take_drop(&mut slot, "jpg").unwrap();
        assert_eq!((s.clip_id, format, dir), (1, Format::Jpg, PathBuf::from("/out")));
        assert!(slot.is_none());
    }

    #[test]
    fn convert_drop_rejects_a_slice_that_needs_ffmpeg() {
        let mut slot = session(1, Some("/out"));
        let s = slot.as_mut().unwrap();
        s.slices = vec![Slice { format: Format::Mp4, needs: Some(Engine::Ffmpeg) }];
        s.dropped = true;
        assert!(matches!(take_drop(&mut slot, "mp4"), Err(AppError::InvalidInput(_))));
        assert!(slot.is_some());
    }
}
