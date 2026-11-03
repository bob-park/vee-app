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
            geometry::{self, MonitorInfo, RectF, Selection},
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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const RECORDER: &str = "recorder";
/// How often the pill is told where the cursor is (macOS, for its tooltips).
#[cfg(target_os = "macos")]
const POINTER_MS: u64 = 50;
/// The dashed border around the area being recorded, over a dimmed rest of its monitor.
pub const FRAME: &str = "recframe";
/// Windows: the start sound is over by then (0.34 s), so it isn't recorded; the overlay is gone too.
/// macOS leaves Vee's own sound and windows out of the recording, so it starts at once.
const START_DELAY_MS: u64 = if cfg!(windows) { 400 } else { 0 };
/// macOS: the stop sound plays on the click, not after the engine has stopped (see `START_DELAY_MS`).
const STOP_CUE_FIRST: bool = cfg!(target_os = "macos");
/// The pill window's gap above the bottom of the work area, in design points. The window is taller than
/// the pill (room for its shadow), so the pill itself sits ~35 pt up.
const PILL_BOTTOM: f64 = 12.0;
/// How long app exit waits for a running recording to stop.
const EXIT_WAIT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// The overlay of record session `session` is open, or its engine is starting.
    Selecting {
        session: u64,
    },
    Recording {
        session: u64,
        started_ms: i64,
        format: Format,
        limit_ms: Option<i64>,
    },
    /// Stopping, converting and saving, or cancelling; or the app is quitting.
    Finishing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// `session`: the number a session opened now gets.
    Shortcut {
        session: u64,
    },
    /// The engine of `session` is running.
    Start {
        session: u64,
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
        (Phase::Idle, Event::Shortcut { session }) => (Phase::Selecting { session }, Action::OpenOverlay),
        (Phase::Selecting { .. }, Event::Cancel) => (Phase::Idle, Action::Nothing),
        (Phase::Selecting { session }, Event::Start { session: s, format, started_ms }) if s == session => {
            (Phase::Recording { session, started_ms, format, limit_ms: plan::limit_ms(format) }, Action::StartEngine)
        }
        // A start that lost a race (an older session, the app quitting): drop the engine it just started.
        (p, Event::Start { .. }) => (p, Action::CancelEngine),
        (Phase::Recording { .. }, Event::Shortcut { .. } | Event::Stop) => (Phase::Finishing, Action::StopEngine),
        (Phase::Recording { started_ms, .. }, Event::LimitReached { started_ms: s }) if s == started_ms => {
            (Phase::Finishing, Action::StopEngine)
        }
        (Phase::Recording { .. }, Event::Cancel) => (Phase::Finishing, Action::CancelEngine),
        (Phase::Finishing, Event::Finished) => (Phase::Idle, Action::Nothing),
        (p, _) => (p, Action::Nothing),
    }
}

/// What a start does with its engine once `next` answered `action`.
#[derive(Debug, PartialEq, Eq)]
enum AfterStart {
    Keep,
    /// The app is quitting: leave it for `shutdown` to cancel, as the process may end before this thread does.
    HandBack,
    Cancel,
}

fn after_start(action: Action, closing: bool) -> AfterStart {
    match action {
        Action::StartEngine => AfterStart::Keep,
        _ if closing => AfterStart::HandBack,
        _ => AfterStart::Cancel,
    }
}

/// The phase after `begin` refused `session`: an overlay-closed `Selecting { session }` goes back to Idle so the
/// shortcut works again, unless `session`'s own start is in flight (it settles the phase itself).
fn refused(phase: Phase, session: u64, starting: Option<u64>) -> Phase {
    if phase == (Phase::Selecting { session }) && starting != Some(session) { Phase::Idle } else { phase }
}

/// Whether app exit must still wait: an engine is starting and hasn't handed itself over yet.
fn exit_waits(starting: Option<u64>) -> bool {
    starting.is_some()
}

#[derive(Default)]
struct Inner {
    phase: Phase,
    /// Set while `phase` is `Recording`, or once handed back to `shutdown`.
    running: Option<Running>,
    /// The session whose engine is starting.
    starting: Option<u64>,
    /// The session that muted the copy sounds.
    muted: Option<u64>,
    /// `shutdown` ran.
    closing: bool,
}

/// Managed state of the one recording.
#[derive(Default)]
pub struct Recorder {
    inner: Mutex<Inner>,
    /// Signalled when a start ends (`Inner::starting` cleared).
    started: Condvar,
    sessions: AtomicU64,
    /// Stops a WEBM / GIF conversion (app exit).
    cancel: AtomicBool,
}

/// The `recorder://started` payload, and `get_recorder_state` while recording.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
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
        (Action::StopEngine | Action::CancelEngine, Some(running), Phase::Recording { session, format, .. }) => {
            hide_controls(app);
            // The hidden pill drops its timer, so it never shows this recording's time when shown again.
            let _ = app.emit_to(RECORDER, "recorder://stopped", ());
            let (app, keep) = (app.clone(), action == Action::StopEngine);
            // macOS leaves Vee's sound out of the recording, so the stop sound needn't wait for the engine.
            if keep && STOP_CUE_FIRST {
                cue(&app, false, session);
            }
            std::thread::spawn(move || {
                if keep {
                    finalize(&app, running, format, session);
                } else {
                    running.cancel();
                    unmute(&app, session);
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
    let session = app.state::<Recorder>().sessions.fetch_add(1, Ordering::SeqCst) + 1;
    dispatch(app, Event::Shortcut { session });
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
    if matches!(phase(app), Phase::Selecting { .. }) {
        dispatch(app, Event::Cancel);
    }
}

/// App exit: waits (bounded) for an engine that is starting, cancels the running one, stops a conversion.
/// SCK and the Windows engine never need the main thread to stop, so blocking it here can't deadlock.
pub fn shutdown(app: &AppHandle) {
    let Some(r) = app.try_state::<Recorder>() else { return };
    r.cancel.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + EXIT_WAIT;
    let running = {
        let mut g = r.inner.lock().unwrap();
        g.closing = true;
        g.phase = Phase::Finishing;
        while exit_waits(g.starting) {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { break };
            g = r.started.wait_timeout(g, left).unwrap().0;
        }
        g.running.take()
    };
    if let Some(running) = running {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            running.cancel();
            let _ = tx.send(());
        });
        let _ = rx.recv_timeout(deadline.saturating_duration_since(Instant::now()).max(Duration::from_secs(1)));
    }
    let _ = std::fs::remove_dir_all(temp_dir(app));
}

/// The root of every recording's temp files; removed whenever no recording runs.
fn temp_dir(app: &AppHandle) -> PathBuf {
    std::env::temp_dir().join(format!("vee-record-{}", app.config().identifier))
}

fn sounds_on(app: &AppHandle) -> bool {
    settings::capture_sounds(&app.state::<AppState>().db.lock().unwrap())
}

/// Lifts the copy-sound mute, if `session` is the one that set it.
fn unmute(app: &AppHandle, session: u64) {
    let r = app.state::<Recorder>();
    let mut g = r.inner.lock().unwrap();
    if g.muted == Some(session) {
        g.muted = None;
        sound::set_recording(false);
    }
}

/// Plays a record cue (AppKit: main thread); after the stop cue, lifts `session`'s copy-sound mute.
fn cue(app: &AppHandle, start: bool, session: u64) {
    let sound = sounds_on(app);
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if sound {
            sound::play_record(start);
        }
        if !start {
            unmute(&app2, session);
        }
    });
}

/// Hides the overlay, plays the start sound and starts the engine on the selection made in overlay `label`.
fn start(app: &AppHandle, label: &str, selection: Selection, format: &str) -> AppResult<()> {
    let format = plan::record_format(format).ok_or_else(|| AppError::InvalidInput(format!("unknown record format {format}")))?;
    if plan::needs_ffmpeg(format, cfg!(target_os = "macos")) && engines::ffmpeg(app).is_none() {
        return Err(AppError::InvalidInput(format!("{format:?} needs FFmpeg")));
    }
    let Phase::Selecting { session } = phase(app) else {
        return Err(AppError::InvalidInput("not selecting a recording".into()));
    };
    let (monitor, area) =
        capture::take_record(app, label, &selection).ok_or_else(|| AppError::InvalidInput("record overlay is not open".into()))?;
    begin(app, session, &monitor, &selection, area, format)
}

/// Ends the start of `session`: `shutdown` may stop waiting.
fn end_start(r: &Recorder, g: &mut Inner, session: u64) {
    if g.starting == Some(session) {
        g.starting = None;
    }
    r.started.notify_all();
}

/// The start once the overlay is closed: the start sound, the engine, the pill and the GIF timer.
fn begin(
    app: &AppHandle,
    session: u64,
    monitor: &MonitorInfo,
    selection: &Selection,
    area: Option<RectF>,
    format: Format,
) -> AppResult<()> {
    let r = app.state::<Recorder>();
    {
        let mut g = r.inner.lock().unwrap();
        if g.closing || g.phase != (Phase::Selecting { session }) || g.starting.is_some() {
            g.phase = refused(g.phase, session, g.starting);
            return Err(AppError::InvalidInput("record session is gone".into()));
        }
        g.starting = Some(session);
        // Muted before the start sound, so a copy can't sound while the recording runs.
        g.muted = Some(session);
        sound::set_recording(true);
    }
    r.cancel.store(false, Ordering::SeqCst);
    // At once, so the click shows where the recording is before the engine is up.
    if let Some(area) = area {
        show_frame(app, monitor, area);
    }
    cue(app, true, session);
    std::thread::sleep(Duration::from_millis(START_DELAY_MS));

    // Its own dir, so a stale start never touches a newer session's files.
    let dir = temp_dir(app).join(session.to_string());
    let container = container_for(format.ext());
    let out = dir.join(if container == Container::Mov { "recording.mov" } else { "recording.mp4" });
    let quitting = r.inner.lock().unwrap().closing;
    let running = if quitting {
        Err(AppError::Other("the app is quitting".into()))
    } else {
        std::fs::create_dir_all(&dir).map_err(AppError::from).and_then(|()| Running::start(app, monitor, selection, container, &out))
    };
    let running = match running {
        Ok(running) => running,
        Err(e) => {
            log::warn!("recording didn't start: {e}");
            hide_controls(app);
            let _ = std::fs::remove_dir_all(&dir);
            {
                let mut g = r.inner.lock().unwrap();
                if g.phase == (Phase::Selecting { session }) {
                    g.phase = Phase::Idle;
                }
                end_start(&r, &mut g, session);
            }
            unmute(app, session);
            if !quitting {
                window::show_toast(app, ToastPayload::convert(ConvertNote::RecordFailed, Kind::Files));
            }
            return Err(e);
        }
    };
    let started_ms = now_ms();
    let (action, lost) = {
        let mut g = r.inner.lock().unwrap();
        let (phase, action) = next(&g.phase, Event::Start { session, format, started_ms });
        g.phase = phase;
        let lost = match after_start(action, g.closing) {
            AfterStart::Keep | AfterStart::HandBack => {
                g.running = Some(running);
                None
            }
            AfterStart::Cancel => Some(running),
        };
        end_start(&r, &mut g, session);
        (action, lost)
    };
    if action != Action::StartEngine {
        hide_controls(app);
        if let Some(running) = lost {
            running.cancel();
            let _ = std::fs::remove_dir_all(&dir);
        }
        unmute(app, session);
        return Err(AppError::Other("recording stopped while starting".into()));
    }
    let limit_ms = plan::limit_ms(format);
    // Before `show_pill`, so the pill never shows with a previous recording's time.
    let _ = app.emit_to(RECORDER, "recorder://started", Started { started_ms, format, limit_ms });
    show_pill(app, monitor);
    if let Some(limit) = limit_ms {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(limit as u64));
            dispatch(&app, Event::LimitReached { started_ms });
        });
    }
    Ok(())
}

/// The running recording, for a pill that missed `recorder://started`.
fn started(app: &AppHandle) -> Option<Started> {
    match phase(app) {
        Phase::Recording { started_ms, format, limit_ms, .. } => Some(Started { started_ms, format, limit_ms }),
        _ => None,
    }
}

/// Windows: keeps the overlay, the pill and the border out of every capture and recording.
/// macOS needs nothing: ScreenCaptureKit leaves out Vee's own windows.
pub fn exclude_from_capture(app: &AppHandle) {
    for label in [capture::CAPTURE, RECORDER, FRAME] {
        if let Some(w) = app.get_webview_window(label) {
            exclude_window(&w);
        }
    }
}

/// Windows: keeps `window` out of every capture and recording. A no-op on macOS (see `exclude_from_capture`).
pub fn exclude_window(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        use ::windows::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE},
        };
        let Ok(hwnd) = window.hwnd() else { return };
        // SAFETY: a live top-level window of this process.
        if let Err(e) = unsafe { SetWindowDisplayAffinity(HWND(hwnd.0 as _), WDA_EXCLUDEFROMCAPTURE) } {
            log::warn!("couldn't exclude {} from capture: {e}", window.label());
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

/// Hides the pill and the border.
fn hide_controls(app: &AppHandle) {
    for label in [RECORDER, FRAME] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.hide();
        }
    }
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
        #[cfg(target_os = "macos")]
        follow_pointer(app.clone(), r);
    }
    let _ = pill.show();
}

/// macOS gives a window that isn't key no mouse moves, so the pill never sees a hover. While it is up, this
/// sends it the cursor (pill-local px, null when outside) for its tooltips.
#[cfg(target_os = "macos")]
fn follow_pointer(app: AppHandle, r: window::Rect) {
    std::thread::spawn(move || {
        let mut last = None;
        loop {
            std::thread::sleep(Duration::from_millis(POINTER_MS));
            let Some(pill) = app.get_webview_window(RECORDER).filter(|p| p.is_visible().unwrap_or(false)) else { return };
            let at = window::cursor_at(&app)
                .map(|(x, y)| (x - r.x, y - r.y))
                .filter(|&(x, y)| (0.0..r.w).contains(&x) && (0.0..r.h).contains(&y))
                .map(|(x, y)| geometry::Point { x, y });
            if at != last {
                let _ = pill.emit("recorder://pointer", at);
                last = at;
            }
        }
    });
}

/// Covers the monitor: dims all but the recorded `area` (monitor-local logical px) and dashes its edge.
/// Clicks pass through, and it stays out of the recording (Vee is excluded from it).
fn show_frame(app: &AppHandle, monitor: &MonitorInfo, (x, y, w, h): RectF) {
    let Some(frame) = app.get_webview_window(FRAME) else { return };
    let (px, py, pw, ph) = geometry::to_physical((0.0, 0.0, monitor.width, monitor.height), monitor);
    let _ = window::place(&frame, window::rect_from((px, py), (pw, ph), monitor.scale, window::USES_POINTS));
    let _ = frame.emit("recframe://area", (x, y, w, h));
    let _ = frame.set_ignore_cursor_events(true);
    let _ = frame.show();
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
fn finalize(app: &AppHandle, running: Running, format: Format, session: u64) {
    let recorded = running.stop();
    if !STOP_CUE_FIRST {
        cue(app, false, session);
    }
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
        window::show_toast(app, ToastPayload::convert(ConvertNote::RecordConverting { format }, Kind::Files));
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
    use tauri::{AppHandle, WebviewWindow};

    /// Hides the overlay and records `selection` as `format` (`mp4` · `mov` · `webm` · `gif`).
    #[tauri::command]
    pub async fn start_recording(app: AppHandle, window: WebviewWindow, selection: Selection, format: String) -> AppResult<()> {
        let label = window.label().to_string();
        tauri::async_runtime::spawn_blocking(move || super::start(&app, &label, selection, &format))
            .await
            .map_err(|e| AppError::Other(e.to_string()))?
    }

    /// The running recording, or `None`; the pill asks on mount in case it missed `recorder://started`.
    #[tauri::command]
    pub fn get_recorder_state(app: AppHandle) -> Option<super::Started> {
        super::started(&app)
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

    const SEL: Phase = Phase::Selecting { session: 3 };

    fn recording(started_ms: i64) -> Phase {
        Phase::Recording { session: 3, started_ms, format: Format::Gif, limit_ms: Some(30_000) }
    }

    fn start(session: u64, format: Format) -> Event {
        Event::Start { session, format, started_ms: 5 }
    }

    #[test]
    fn refused_start_frees_the_shortcut() {
        // An older session's start is in flight: this session's overlay is closed, so back to Idle.
        assert_eq!(refused(SEL, 3, Some(2)), Phase::Idle);
        assert_eq!(refused(SEL, 3, None), Phase::Idle);
        // Its own start is in flight, or the phase belongs to another session or has moved on: untouched.
        assert_eq!(refused(SEL, 3, Some(3)), SEL);
        assert_eq!(refused(SEL, 4, None), SEL);
        assert_eq!(refused(Phase::Finishing, 3, None), Phase::Finishing);
    }

    #[test]
    fn shortcut_opens_then_start_records() {
        assert_eq!(next(&Phase::Idle, Event::Shortcut { session: 3 }), (SEL, Action::OpenOverlay));
        assert_eq!(next(&SEL, start(3, Format::Gif)), (recording(5), Action::StartEngine));
        let mp4 = next(&SEL, start(3, Format::Mp4)).0;
        assert_eq!(mp4, Phase::Recording { session: 3, started_ms: 5, format: Format::Mp4, limit_ms: None });
        assert_eq!(next(&SEL, Event::Cancel), (Phase::Idle, Action::Nothing));
    }

    #[test]
    fn shortcut_while_recording_stops() {
        assert_eq!(next(&recording(1), Event::Shortcut { session: 4 }), (Phase::Finishing, Action::StopEngine));
        assert_eq!(next(&recording(1), Event::Stop), (Phase::Finishing, Action::StopEngine));
        assert_eq!(next(&recording(1), Event::Cancel), (Phase::Finishing, Action::CancelEngine));
        assert_eq!(next(&Phase::Finishing, Event::Finished), (Phase::Idle, Action::Nothing));
    }

    #[test]
    fn shortcut_while_selecting_or_finishing_does_nothing() {
        for p in [SEL, Phase::Finishing] {
            assert_eq!(next(&p, Event::Shortcut { session: 4 }), (p, Action::Nothing));
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
            assert_eq!(next(&p, start(3, Format::Mp4)), (p, Action::CancelEngine));
        }
    }

    #[test]
    fn a_stale_session_start_is_rejected() {
        assert_eq!(next(&SEL, start(2, Format::Mp4)), (SEL, Action::CancelEngine));
    }

    #[test]
    fn a_start_during_exit_hands_its_engine_back() {
        // `shutdown` set Finishing, so the start is refused and its engine goes to `shutdown`.
        let (_, action) = next(&Phase::Finishing, start(3, Format::Mp4));
        assert_eq!(after_start(action, true), AfterStart::HandBack);
        assert_eq!(after_start(action, false), AfterStart::Cancel);
        assert_eq!(after_start(Action::StartEngine, false), AfterStart::Keep);
    }

    #[test]
    fn exit_waits_while_an_engine_is_starting() {
        assert!(exit_waits(Some(3)));
        assert!(!exit_waits(None));
    }
}
