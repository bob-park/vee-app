//! Screen recording: the shortcut gates, the recording state machine, and saving the file.
//! The engines (`Running`) are ScreenCaptureKit on macOS and FFmpeg + WASAPI loopback on Windows:
//! `Running::start` begins a recording of a selection, `stop` finalizes the file, `cancel` deletes it.
//! Both block for seconds, so they never run on the main thread.

#[cfg(target_os = "macos")]
mod mac;
pub mod plan;
#[cfg(windows)]
mod win;

#[cfg(target_os = "macos")]
pub use mac::Running;
pub use plan::{Container, container_for};
#[cfg(windows)]
pub use win::Running;

use crate::{
    app::{
        capture::{
            self, Mode,
            geometry::{MonitorInfo, Selection},
        },
        convert::ConvertNote,
        engines, screen_access, sound,
        state::{AppState, now_ms},
        window::{self, ToastPayload},
    },
    domain::{
        clip::{self, Kind, NewClip},
        convert::{self as conv, Format, JobError, Quality, Run},
        settings, source_app,
    },
    shared::error::{AppError, AppResult},
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const RECORDER: &str = "recorder";
/// The start sound is over by then (0.34 s), so it isn't recorded; the overlay is gone too.
const START_DELAY_MS: u64 = 400;
/// The pill's gap above the bottom of the work area, in design points.
const PILL_BOTTOM: f64 = 24.0;
/// How long app exit waits for a running recording to stop.
const EXIT_WAIT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// The overlay is open in record mode, or the engine is starting.
    Selecting,
    Recording {
        started_ms: i64,
        format: Format,
        limit_ms: Option<i64>,
    },
    /// Stopping, converting and saving, or cancelling.
    Finishing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Shortcut,
    /// The engine is running.
    Start {
        format: Format,
        started_ms: i64,
    },
    Stop,
    Cancel,
    /// The file is saved or dropped.
    Finished,
    /// The GIF timer of the recording that started at `started_ms` ran out.
    LimitReached {
        started_ms: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    OpenOverlay,
    /// Keep the started engine.
    StartEngine,
    StopEngine,
    CancelEngine,
    Nothing,
}

/// The recording state machine.
pub fn next(phase: &Phase, ev: Event) -> (Phase, Action) {
    match (*phase, ev) {
        (Phase::Idle, Event::Shortcut) => (Phase::Selecting, Action::OpenOverlay),
        (Phase::Selecting, Event::Cancel) => (Phase::Idle, Action::Nothing),
        (Phase::Selecting, Event::Start { format, started_ms }) => {
            (Phase::Recording { started_ms, format, limit_ms: plan::limit_ms(format) }, Action::StartEngine)
        }
        // The start lost a race (the app is quitting): drop the engine it just started.
        (p, Event::Start { .. }) => (p, Action::CancelEngine),
        (Phase::Recording { .. }, Event::Shortcut | Event::Stop) => (Phase::Finishing, Action::StopEngine),
        (Phase::Recording { started_ms, .. }, Event::LimitReached { started_ms: s }) if s == started_ms => {
            (Phase::Finishing, Action::StopEngine)
        }
        (Phase::Recording { .. }, Event::Cancel) => (Phase::Finishing, Action::CancelEngine),
        (Phase::Finishing, Event::Finished) => (Phase::Idle, Action::Nothing),
        (p, _) => (p, Action::Nothing),
    }
}

#[derive(Default)]
struct Inner {
    phase: Phase,
    /// Set exactly while `phase` is `Recording`.
    running: Option<Running>,
}

/// Managed state of the one recording.
#[derive(Default)]
pub struct Recorder {
    inner: Mutex<Inner>,
    /// Stops a WEBM / GIF conversion (app exit).
    cancel: AtomicBool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Started {
    started_ms: i64,
    format: Format,
    limit_ms: Option<i64>,
}

fn phase(app: &AppHandle) -> Phase {
    app.state::<Recorder>().inner.lock().unwrap().phase
}

/// Feeds `ev` to the machine and runs its action; engine work goes to a thread.
fn dispatch(app: &AppHandle, ev: Event) {
    let (prev, action, running) = {
        let recorder = app.state::<Recorder>();
        let mut g = recorder.inner.lock().unwrap();
        let prev = g.phase;
        let (phase, action) = next(&prev, ev);
        g.phase = phase;
        let running = if matches!(action, Action::StopEngine | Action::CancelEngine) { g.running.take() } else { None };
        (prev, action, running)
    };
    match (action, running, prev) {
        (Action::OpenOverlay, ..) => {
            if !capture::open(app, Mode::Record) {
                dispatch(app, Event::Cancel);
            }
        }
        (Action::StopEngine | Action::CancelEngine, Some(running), Phase::Recording { format, .. }) => {
            if let Some(pill) = app.get_webview_window(RECORDER) {
                let _ = pill.hide();
            }
            let (app, keep) = (app.clone(), action == Action::StopEngine);
            std::thread::spawn(move || {
                if keep {
                    finalize(&app, running, format);
                } else {
                    running.cancel();
                    sound::set_recording(false);
                }
                let _ = std::fs::remove_dir_all(temp_dir(&app));
                dispatch(&app, Event::Finished);
            });
        }
        _ => {}
    }
}

/// The record shortcut: opens the overlay when idle (if this machine can record), stops a recording.
pub fn on_shortcut(app: &AppHandle) {
    if phase(app) == Phase::Idle && !can_record(app) {
        return;
    }
    dispatch(app, Event::Shortcut);
}

/// The OS and engine gates; tells the user why not. Screen access is checked by `capture::open`.
fn can_record(app: &AppHandle) -> bool {
    if !screen_access::recording_supported() {
        window::show_toast(app, ToastPayload::convert(ConvertNote::RecordUnsupported, Kind::Files));
        return false;
    }
    #[cfg(windows)]
    if engines::ffmpeg(app).is_none() {
        window::show_toast(app, ToastPayload::convert(ConvertNote::RecordNeedsFfmpeg, Kind::Files));
        window::show_settings(app, "convert");
        return false;
    }
    true
}

/// The record overlay closed without a recording (Esc, or the page never answered).
pub fn overlay_closed(app: &AppHandle) {
    if phase(app) == Phase::Selecting {
        dispatch(app, Event::Cancel);
    }
}

/// App exit: cancels a running recording (bounded) and stops a conversion.
pub fn shutdown(app: &AppHandle) {
    let Some(r) = app.try_state::<Recorder>() else { return };
    r.cancel.store(true, Ordering::SeqCst);
    let running = {
        let mut g = r.inner.lock().unwrap();
        g.phase = Phase::Finishing;
        g.running.take()
    };
    if let Some(running) = running {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            running.cancel();
            let _ = tx.send(());
        });
        let _ = rx.recv_timeout(EXIT_WAIT);
    }
    let _ = std::fs::remove_dir_all(temp_dir(app));
}

fn temp_dir(app: &AppHandle) -> PathBuf {
    std::env::temp_dir().join(format!("vee-record-{}", app.config().identifier))
}

fn sounds_on(app: &AppHandle) -> bool {
    settings::capture_sounds(&app.state::<AppState>().db.lock().unwrap())
}

/// Plays a record cue (AppKit: main thread), then sets the copy-sound mute.
fn cue(app: &AppHandle, start: bool) {
    let sound = sounds_on(app);
    let _ = app.run_on_main_thread(move || {
        if sound {
            sound::play_record(start);
        }
        if !start {
            sound::set_recording(false);
        }
    });
}

/// Hides the overlay, plays the start sound and starts the engine on the selection.
fn start(app: &AppHandle, selection: Selection, format: &str) -> AppResult<()> {
    let format = plan::record_format(format).ok_or_else(|| AppError::InvalidInput(format!("unknown record format {format}")))?;
    if plan::needs_ffmpeg(format, cfg!(target_os = "macos")) && engines::ffmpeg(app).is_none() {
        return Err(AppError::InvalidInput(format!("{format:?} needs FFmpeg")));
    }
    if phase(app) != Phase::Selecting {
        return Err(AppError::InvalidInput("not selecting a recording".into()));
    }
    let monitor = capture::take_record(app).ok_or_else(|| AppError::InvalidInput("record overlay is not open".into()))?;
    begin(app, &monitor, &selection, format)
}

/// The start once the overlay is closed: the start sound, the engine, the pill and the GIF timer.
fn begin(app: &AppHandle, monitor: &MonitorInfo, selection: &Selection, format: Format) -> AppResult<()> {
    app.state::<Recorder>().cancel.store(false, Ordering::SeqCst);
    // Muted before the start sound, so a copy can't sound while the recording runs.
    sound::set_recording(true);
    cue(app, true);
    std::thread::sleep(Duration::from_millis(START_DELAY_MS));

    let dir = temp_dir(app);
    let _ = std::fs::remove_dir_all(&dir);
    let container = container_for(format.ext());
    let out = dir.join(if container == Container::Mov { "recording.mov" } else { "recording.mp4" });
    let running =
        std::fs::create_dir_all(&dir).map_err(AppError::from).and_then(|()| Running::start(app, monitor, selection, container, &out));
    let running = match running {
        Ok(r) => r,
        Err(e) => {
            log::warn!("recording didn't start: {e}");
            sound::set_recording(false);
            let _ = std::fs::remove_dir_all(&dir);
            dispatch(app, Event::Cancel);
            window::show_toast(app, ToastPayload::convert(ConvertNote::RecordFailed, Kind::Files));
            return Err(e);
        }
    };
    let started_ms = now_ms();
    let lost = {
        let recorder = app.state::<Recorder>();
        let mut g = recorder.inner.lock().unwrap();
        let (phase, action) = next(&g.phase, Event::Start { format, started_ms });
        g.phase = phase;
        if action == Action::StartEngine {
            g.running = Some(running);
            None
        } else {
            Some(running)
        }
    };
    if let Some(running) = lost {
        running.cancel();
        sound::set_recording(false);
        let _ = std::fs::remove_dir_all(&dir);
        return Err(AppError::Other("recording stopped while starting".into()));
    }
    show_pill(app, monitor);
    let limit_ms = plan::limit_ms(format);
    let _ = app.emit_to(RECORDER, "recorder://started", Started { started_ms, format, limit_ms });
    if let Some(limit) = limit_ms {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(limit as u64));
            dispatch(&app, Event::LimitReached { started_ms });
        });
    }
    Ok(())
}

/// The pill at the bottom centre of the recorded monitor's work area.
fn show_pill(app: &AppHandle, monitor: &MonitorInfo) {
    let Some(pill) = app.get_webview_window(RECORDER) else { return };
    let found = app.available_monitors().ok().and_then(|ms| {
        ms.into_iter().find(|m| {
            let s = m.scale_factor();
            ((m.position().x as f64 / s) - monitor.x).abs() < 1.0 && ((m.position().y as f64 / s) - monitor.y).abs() < 1.0
        })
    });
    if let (Some(m), Ok(size)) = (found, pill.outer_size()) {
        let (work, unit) = window::work_area(&m);
        // `outer_size` is physical; positioning space is points on macOS.
        let k = if window::USES_POINTS { m.scale_factor() } else { 1.0 };
        let (w, h) = (size.width as f64 / k, size.height as f64 / k);
        let r = window::Rect { x: work.x + (work.w - w) / 2.0, y: work.y + work.h - h - PILL_BOTTOM * unit, w, h };
        let _ = window::place(&pill, r);
    }
    let _ = pill.show();
}

/// The local time as `(year, month, day, hour, minute, second)`, Gregorian, 24-hour.
#[cfg(target_os = "macos")]
fn local_now() -> (i32, u32, u32, u32, u32, u32) {
    use objc2_foundation::{NSCalendar, NSCalendarIdentifierGregorian, NSCalendarUnit, NSDate};
    let now = NSDate::now();
    let cal = NSCalendar::calendarWithIdentifier(unsafe { NSCalendarIdentifierGregorian }).unwrap_or_else(NSCalendar::currentCalendar);
    let c = |u| cal.component_fromDate(u, &now) as u32;
    (
        c(NSCalendarUnit::Year) as i32,
        c(NSCalendarUnit::Month),
        c(NSCalendarUnit::Day),
        c(NSCalendarUnit::Hour),
        c(NSCalendarUnit::Minute),
        c(NSCalendarUnit::Second),
    )
}

#[cfg(windows)]
fn local_now() -> (i32, u32, u32, u32, u32, u32) {
    use ::windows::Globalization::{Calendar, CalendarIdentifiers, ClockIdentifiers};
    let read = || -> ::windows::core::Result<_> {
        let cal = Calendar::new()?;
        cal.ChangeCalendarSystem(&CalendarIdentifiers::Gregorian()?)?;
        cal.ChangeClock(&ClockIdentifiers::TwentyFourHour()?)?;
        let u = |v: i32| v as u32;
        Ok((cal.Year()?, u(cal.Month()?), u(cal.Day()?), u(cal.Hour()?), u(cal.Minute()?), u(cal.Second()?)))
    };
    read().unwrap_or((1970, 1, 1, 0, 0, 0))
}

/// Stops the engine, plays the stop sound, then converts and saves the file and tells the user.
fn finalize(app: &AppHandle, running: Running, format: Format) {
    let recorded = running.stop();
    cue(app, false);
    let note = save(app, recorded, format);
    window::show_toast(app, ToastPayload::convert(note, Kind::Files));
}

fn save(app: &AppHandle, recorded: AppResult<PathBuf>, format: Format) -> ConvertNote {
    let recorded = match recorded {
        Ok(p) => p,
        Err(e) => {
            log::warn!("recording failed: {e}");
            return ConvertNote::RecordFailed;
        }
    };
    let downloads = app.path().download_dir().unwrap_or_else(|_| std::env::temp_dir());
    let setting = settings::get(&app.state::<AppState>().db.lock().unwrap(), "recordFolder");
    let (dir, fell_back) = plan::resolve_folder(&setting, &downloads, Path::is_dir);
    let now = local_now();
    let file = if matches!(format, Format::Webm | Format::Gif) {
        match convert(app, &recorded, format) {
            Ok(p) => p,
            Err(error) => {
                // Keep the recording, as MP4, rather than nothing.
                log::warn!("recording: converting to {format:?} failed: {error:?}");
                let kept = plan::unique_path(&dir, &plan::file_name(now, "mp4"), Path::exists);
                let error = conv::move_file(&recorded, &kept).map_or(JobError::Write, |()| error);
                return ConvertNote::Failed { name: name_of(&kept), error };
            }
        }
    } else {
        recorded
    };
    let out = plan::unique_path(&dir, &plan::file_name(now, format.ext()), Path::exists);
    if let Err(e) = conv::move_file(&file, &out) {
        log::warn!("recording: moving into {} failed: {e}", dir.display());
        return ConvertNote::Failed { name: name_of(&out), error: JobError::Write };
    }
    if let Err(e) = add_clip(app, &out) {
        log::warn!("recording: copying {} failed: {e}", out.display());
    }
    let folder = name_of(&dir);
    if fell_back { ConvertNote::RecordFolderMissing { format, folder } } else { ConvertNote::Saved { format, count: 1, folder } }
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// MP4 → WEBM / GIF next to it, through the convert pipeline; GIF at 15 fps, ≤ 800 px wide.
fn convert(app: &AppHandle, src: &Path, format: Format) -> Result<PathBuf, JobError> {
    let (ffmpeg, ffprobe) = engines::ffmpeg(app).ok_or(JobError::Engine)?;
    let quality = if format == Format::Gif {
        Quality::Original
    } else {
        Quality::parse(&settings::get(&app.state::<AppState>().db.lock().unwrap(), "videoQuality"))
    };
    let dir = src.parent().ok_or(JobError::Write)?;
    let stage = dir.join("stage");
    std::fs::create_dir_all(&stage).map_err(|_| JobError::Write)?;
    let recorder = app.state::<Recorder>();
    let mut progress = |_: Option<f64>| {};
    let mut ctx = Run {
        ffmpeg: Some((&ffmpeg, &ffprobe)),
        quality,
        hw: None,
        fell_back: false,
        cancel: &recorder.cancel,
        progress: &mut progress,
        rhwp: None,
        soffice: None,
        lo_profile: stage.clone(),
        pdf: None,
    };
    conv::run(src, "recording", format, dir, &stage, &mut ctx)
}

/// The saved file as a files clip credited to Vee, and on the clipboard.
fn add_clip(app: &AppHandle, path: &Path) -> AppResult<()> {
    let state = app.state::<AppState>();
    let content = {
        let conn = state.db.lock().unwrap();
        let vee = source_app::upsert_app(&conn, &app.config().identifier, &app.package_info().name, Some(window::DRAG_ICON))?;
        let clip = NewClip::Files { paths: vec![path.to_string_lossy().into_owned()], thumbs: vec![] };
        let id = clip::upsert(&conn, &state.images_dir, clip, Some(vee), now_ms())?;
        clip::content(&conn, id)?.ok_or_else(|| AppError::Other("clip no longer exists".into()))?
    };
    window::write_clipboard(&state, &content, Vec::new())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

pub mod commands {
    use super::{Event, Selection};
    use crate::shared::error::{AppError, AppResult};
    use tauri::AppHandle;

    /// Hides the overlay and records `selection` as `format` (`mp4` · `mov` · `webm` · `gif`).
    #[tauri::command]
    pub async fn start_recording(app: AppHandle, selection: Selection, format: String) -> AppResult<()> {
        tauri::async_runtime::spawn_blocking(move || super::start(&app, selection, &format))
            .await
            .map_err(|e| AppError::Other(e.to_string()))?
    }

    /// Stops and saves the recording.
    #[tauri::command]
    pub fn stop_recording(app: AppHandle) {
        super::dispatch(&app, Event::Stop);
    }

    /// Stops the recording and drops the file.
    #[tauri::command]
    pub fn cancel_recording(app: AppHandle) {
        super::dispatch(&app, Event::Cancel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording(started_ms: i64) -> Phase {
        Phase::Recording { started_ms, format: Format::Gif, limit_ms: Some(30_000) }
    }

    #[test]
    fn shortcut_opens_then_start_records() {
        assert_eq!(next(&Phase::Idle, Event::Shortcut), (Phase::Selecting, Action::OpenOverlay));
        assert_eq!(next(&Phase::Selecting, Event::Start { format: Format::Gif, started_ms: 5 }), (recording(5), Action::StartEngine));
        let mp4 = next(&Phase::Selecting, Event::Start { format: Format::Mp4, started_ms: 5 }).0;
        assert_eq!(mp4, Phase::Recording { started_ms: 5, format: Format::Mp4, limit_ms: None });
        assert_eq!(next(&Phase::Selecting, Event::Cancel), (Phase::Idle, Action::Nothing));
    }

    #[test]
    fn shortcut_while_recording_stops() {
        assert_eq!(next(&recording(1), Event::Shortcut), (Phase::Finishing, Action::StopEngine));
        assert_eq!(next(&recording(1), Event::Stop), (Phase::Finishing, Action::StopEngine));
        assert_eq!(next(&recording(1), Event::Cancel), (Phase::Finishing, Action::CancelEngine));
        assert_eq!(next(&Phase::Finishing, Event::Finished), (Phase::Idle, Action::Nothing));
    }

    #[test]
    fn shortcut_while_selecting_or_finishing_does_nothing() {
        for p in [Phase::Selecting, Phase::Finishing] {
            assert_eq!(next(&p, Event::Shortcut), (p, Action::Nothing));
        }
        assert_eq!(next(&Phase::Idle, Event::Stop), (Phase::Idle, Action::Nothing));
        assert_eq!(next(&Phase::Idle, Event::Cancel), (Phase::Idle, Action::Nothing));
    }

    #[test]
    fn limit_reached_stops_once() {
        assert_eq!(next(&recording(1), Event::LimitReached { started_ms: 1 }), (Phase::Finishing, Action::StopEngine));
        assert_eq!(next(&Phase::Finishing, Event::LimitReached { started_ms: 1 }), (Phase::Finishing, Action::Nothing));
        assert_eq!(next(&Phase::Finishing, Event::Stop), (Phase::Finishing, Action::Nothing));
        // A timer left over from an earlier recording leaves this one alone.
        assert_eq!(next(&recording(2), Event::LimitReached { started_ms: 1 }), (recording(2), Action::Nothing));
    }

    #[test]
    fn a_start_that_lost_the_race_cancels_its_engine() {
        for p in [Phase::Idle, Phase::Finishing, recording(1)] {
            assert_eq!(next(&p, Event::Start { format: Format::Mp4, started_ms: 9 }), (p, Action::CancelEngine));
        }
    }
}
