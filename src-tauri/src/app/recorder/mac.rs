//! macOS 15+ recording: an `SCStream` with an `SCRecordingOutput` writes screen + audio straight to the file:
//! all system audio for a region, the window's app audio for a window (see `filter`). SCK runs its completion handlers and delegate calls on its own queues, so nothing here needs the
//! main thread; each handler is bridged to the calling thread with a channel.

use super::plan::{Container, even};
use crate::{
    app::capture::geometry::{MonitorInfo, Selection},
    shared::error::{AppError, AppResult},
};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AllocAnyThread, DefinedClass, Encode, Encoding, define_class, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol, NSString, NSURL};
use objc2_screen_capture_kit::{
    SCContentFilter, SCRecordingOutput, SCRecordingOutputConfiguration, SCRecordingOutputDelegate, SCShareableContent, SCStream,
    SCStreamConfiguration,
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use tauri::AppHandle;

/// How long SCK gets to answer a request or to finish writing the file.
const TIMEOUT: Duration = Duration::from_secs(10);
/// How long `Drop` waits for SCK, best effort.
const DROP_TIMEOUT: Duration = Duration::from_secs(2);

type Done = Result<(), String>;

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Delegate` has no `Drop`.
    #[unsafe(super(NSObject))]
    #[name = "VeeRecordingDelegate"]
    #[ivars = Mutex<Option<Sender<Done>>>]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl SCRecordingOutputDelegate for Delegate {
        #[unsafe(method(recordingOutputDidFinishRecording:))]
        fn did_finish(&self, _output: &SCRecordingOutput) {
            self.send(Ok(()));
        }

        #[unsafe(method(recordingOutput:didFailWithError:))]
        fn did_fail(&self, _output: &SCRecordingOutput, error: &NSError) {
            self.send(Err(error.localizedDescription().to_string()));
        }
    }
);

impl Delegate {
    fn new(tx: Sender<Done>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(Mutex::new(Some(tx)));
        unsafe { msg_send![super(this), init] }
    }

    /// The first outcome wins: a failure mid-recording is what `stop` reports.
    fn send(&self, done: Done) {
        if let Some(tx) = self.ivars().lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.send(done);
        }
    }
}

/// A recording in progress.
pub struct Running {
    stream: Retained<SCStream>,
    // Kept alive until the file is final: SCK holds its delegate weakly.
    _output: Retained<SCRecordingOutput>,
    _delegate: Retained<Delegate>,
    done: Receiver<Done>,
    out: PathBuf,
    /// Set once `stop` / `cancel` ran, so `Drop` leaves the file alone.
    stopped: bool,
}

// SAFETY: SCStream / SCRecordingOutput may be started and stopped from any thread (SCK serializes on its own
// queue), and the delegate's state is a `Mutex`.
unsafe impl Send for Running {}

fn other(step: &str, e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("recording: {step}: {e}"))
}

/// The text of an `NSError` a completion handler got, if any.
fn error_text(e: *mut NSError) -> Option<String> {
    unsafe { e.as_ref() }.map(|e| e.localizedDescription().to_string())
}

/// `CMTime`, declared here so objc2-core-media isn't needed for the one setter that takes it.
#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

// SAFETY: matches CoreMedia's `CMTime` layout and its `{?=qiIq}` type encoding.
unsafe impl Encode for CMTime {
    const ENCODING: Encoding = Encoding::Struct("?", &[i64::ENCODING, i32::ENCODING, u32::ENCODING, i64::ENCODING]);
}

/// `kCMTimeFlags_Valid`.
const CMTIME_VALID: u32 = 1;

/// Runs an SCK call whose completion handler takes only an error, and waits up to `timeout` for it.
fn await_error(step: &str, timeout: Duration, call: impl FnOnce(&block2::DynBlock<dyn Fn(*mut NSError)>)) -> AppResult<()> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |e: *mut NSError| {
        let _ = tx.send(error_text(e));
    });
    call(&block);
    match rx.recv_timeout(timeout) {
        Ok(None) => Ok(()),
        Ok(Some(e)) => Err(other(step, e)),
        Err(_) => Err(other(step, "timed out")),
    }
}

fn shareable_content() -> AppResult<Retained<SCShareableContent>> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |content: *mut SCShareableContent, e: *mut NSError| {
        let _ = tx.send(unsafe { Retained::retain(content) }.ok_or_else(|| error_text(e).unwrap_or_default()));
    });
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    rx.recv_timeout(TIMEOUT).map_err(|_| other("shareable content", "timed out"))?.map_err(|e| other("shareable content", e))
}

/// The filter, its output size in physical px (even), and the display-local source rect in points (a region).
type Filter = (Retained<SCContentFilter>, (u32, u32), Option<CGRect>);

/// The filter for `sel`.
/// - A region: the overlay's display without Vee's windows (the overlay, the pill), cropped to the region; all
///   system audio but Vee's.
/// - A window: that window alone (`initWithDesktopIndependentWindow`), so windows in front of it stay out; SCK
///   then records only that window's app audio.
fn filter(content: &SCShareableContent, monitor: &MonitorInfo, sel: &Selection) -> AppResult<Filter> {
    let px = |points: f64| even((points * monitor.scale).round().max(2.0) as u32);
    let (x, y, width, height) = match *sel {
        Selection::Rect { x, y, width, height } => (x, y, width, height),
        Selection::Window { id } => {
            let window = unsafe { content.windows() }
                .iter()
                .find(|w| u64::from(unsafe { w.windowID() }) == id)
                .ok_or_else(|| other("window", format!("{id} is gone")))?;
            let size = unsafe { window.frame() }.size;
            let f = unsafe { SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &window) };
            return Ok((f, (px(size.width), px(size.height)), None));
        }
    };
    // Tauri's logical monitor origin is the display's CG origin in points: take the nearest display.
    let displays = unsafe { content.displays() };
    let dist = |f: CGRect| (f.origin.x - monitor.x).abs() + (f.origin.y - monitor.y).abs();
    let display =
        displays.iter().min_by(|a, b| unsafe { dist(a.frame()).total_cmp(&dist(b.frame())) }).ok_or_else(|| other("display", "none"))?;
    let own = std::process::id() as i32;
    let apps: Vec<_> = unsafe { content.applications() }.iter().filter(|a| unsafe { a.processID() } == own).collect();
    let f = unsafe {
        SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
            SCContentFilter::alloc(),
            &display,
            &NSArray::from_retained_slice(&apps),
            &NSArray::new(),
        )
    };
    Ok((f, (px(width), px(height)), Some(CGRect::new(CGPoint::new(x, y), CGSize::new(width, height)))))
}

impl Running {
    pub fn start(_app: &AppHandle, monitor: &MonitorInfo, sel: &Selection, container: Container, out: &Path) -> AppResult<Running> {
        begin(monitor, sel, container, out)
    }

    /// Stops the stream and waits until the file is final. On an error the partial file is deleted.
    pub fn stop(mut self) -> AppResult<PathBuf> {
        match self.halt(TIMEOUT) {
            Ok(()) => Ok(self.out.clone()),
            Err(e) => {
                let _ = std::fs::remove_file(&self.out);
                Err(e)
            }
        }
    }

    /// Stops and deletes the file.
    pub fn cancel(mut self) {
        if let Err(e) = self.halt(TIMEOUT) {
            log::warn!("{e}");
        }
        let _ = std::fs::remove_file(&self.out);
    }

    /// Stops the stream and waits up to `timeout` (each) for the stop and for the file to be final.
    fn halt(&mut self, timeout: Duration) -> AppResult<()> {
        self.stopped = true;
        let stopped = await_error("stop", timeout, |h| unsafe { self.stream.stopCaptureWithCompletionHandler(Some(h)) });
        let finished = match self.done.recv_timeout(timeout) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(other("write", e)),
            Err(_) => Err(other("write", "timed out")),
        };
        stopped.and(finished)
    }
}

impl Drop for Running {
    /// Never stopped (a failed start, the app quitting, a panic): stop the stream, briefly, and drop the partial file.
    fn drop(&mut self) {
        if !self.stopped {
            let _ = self.halt(DROP_TIMEOUT);
            let _ = std::fs::remove_file(&self.out);
        }
    }
}

fn begin(monitor: &MonitorInfo, sel: &Selection, container: Container, out: &Path) -> AppResult<Running> {
    let content = shareable_content()?;
    let (filter, (w, h), source) = filter(&content, monitor, sel)?;

    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setWidth(w as usize);
        config.setHeight(h as usize);
        if let Some(source) = source {
            config.setSourceRect(source);
        }
        config.setShowsCursor(true);
        config.setCapturesAudio(true);
        config.setExcludesCurrentProcessAudio(true);
        let _: () = msg_send![&*config, setMinimumFrameInterval: CMTime { value: 1, timescale: 30, flags: CMTIME_VALID, epoch: 0 }];
    }

    let rec = unsafe { SCRecordingOutputConfiguration::new() };
    let file_type = NSString::from_str(match container {
        Container::Mp4 => "public.mpeg-4",
        Container::Mov => "com.apple.quicktime-movie",
    });
    unsafe {
        rec.setOutputURL(&NSURL::fileURLWithPath(&NSString::from_str(&out.to_string_lossy())));
        // `AVFileType` is an `NSString`; set through `msg_send` so objc2-av-foundation isn't needed.
        let _: () = msg_send![&*rec, setOutputFileType: &*file_type];
    }

    let (tx, done) = mpsc::channel();
    let delegate = Delegate::new(tx);
    let output = unsafe {
        SCRecordingOutput::initWithConfiguration_delegate(SCRecordingOutput::alloc(), &rec, ProtocolObject::from_ref(&*delegate))
    };
    let stream = unsafe { SCStream::initWithFilter_configuration_delegate(SCStream::alloc(), &filter, &config, None) };
    unsafe { stream.addRecordingOutput_error(&output) }.map_err(|e| other("add output", e.localizedDescription()))?;
    // Owned before the start, so a failed or timed-out start is stopped and its file removed by `Drop`.
    let running = Running { stream, _output: output, _delegate: delegate, done, out: out.to_path_buf(), stopped: false };
    await_error("start", TIMEOUT, |h| unsafe { running.stream.startCaptureWithCompletionHandler(Some(h)) })?;
    Ok(running)
}
