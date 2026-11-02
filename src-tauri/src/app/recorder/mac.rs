//! macOS 15+ recording: an `SCStream` with an `SCRecordingOutput` writes screen + system audio straight to
//! the file. SCK runs its completion handlers and delegate calls on its own queues, so nothing here needs the
//! main thread; each handler is bridged to the calling thread with a channel.

use super::plan::{Container, even};
use crate::{
    app::capture::geometry::{MonitorInfo, Selection},
    shared::error::{AppError, AppResult},
};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};
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

/// Runs an SCK call whose completion handler takes only an error, and waits for it.
fn await_error(step: &str, call: impl FnOnce(&block2::DynBlock<dyn Fn(*mut NSError)>)) -> AppResult<()> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |e: *mut NSError| {
        let _ = tx.send(error_text(e));
    });
    call(&block);
    match rx.recv_timeout(TIMEOUT) {
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

type Filter = (Retained<SCContentFilter>, (u32, u32), Option<CGRect>);

/// The filter for `sel` and its output size in physical px (even), plus the display-local source rect in points.
fn filter(content: &SCShareableContent, monitor: &MonitorInfo, sel: &Selection) -> AppResult<Filter> {
    let px = |points: f64, scale: f64| even((points * scale).round().max(2.0) as u32);
    match *sel {
        Selection::Rect { x, y, width, height } => {
            // Tauri's logical monitor origin is the display's CG origin in points: take the nearest display.
            let displays = unsafe { content.displays() };
            let dist = |f: CGRect| (f.origin.x - monitor.x).abs() + (f.origin.y - monitor.y).abs();
            let display = displays
                .iter()
                .min_by(|a, b| unsafe { dist(a.frame()).total_cmp(&dist(b.frame())) })
                .ok_or_else(|| other("display", "none"))?;
            // Vee's own windows (the overlay, the pill) never show in the recording.
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
            let rect = CGRect::new(CGPoint::new(x, y), CGSize::new(width, height));
            Ok((f, (px(width, monitor.scale), px(height, monitor.scale)), Some(rect)))
        }
        Selection::Window { id } => {
            let window = unsafe { content.windows() }
                .iter()
                .find(|w| u64::from(unsafe { w.windowID() }) == id)
                .ok_or_else(|| other("window", format!("{id} is gone")))?;
            let f = unsafe { SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &window) };
            // The window may sit on another display than the overlay's: SCK knows its true scale.
            let info = unsafe { SCShareableContent::infoForFilter(&f) };
            let (rect, scale) = unsafe { (info.contentRect(), f64::from(info.pointPixelScale())) };
            Ok((f, (px(rect.size.width, scale), px(rect.size.height, scale)), None))
        }
    }
}

impl Running {
    pub fn start(_app: &AppHandle, monitor: &MonitorInfo, sel: &Selection, container: Container, out: &Path) -> AppResult<Running> {
        begin(monitor, sel, container, out)
    }

    /// Stops the stream and waits until the file is final.
    pub fn stop(self) -> AppResult<PathBuf> {
        let stopped = await_error("stop", |h| unsafe { self.stream.stopCaptureWithCompletionHandler(Some(h)) });
        let finished = match self.done.recv_timeout(TIMEOUT) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(other("write", e)),
            Err(_) => Err(other("write", "timed out")),
        };
        stopped.and(finished)?;
        Ok(self.out)
    }

    /// Stops and deletes the file.
    pub fn cancel(self) {
        let out = self.out.clone();
        if let Err(e) = self.stop() {
            log::warn!("{e}");
        }
        let _ = std::fs::remove_file(out);
    }
}

fn begin(monitor: &MonitorInfo, sel: &Selection, container: Container, out: &Path) -> AppResult<Running> {
    let content = shareable_content()?;
    let (filter, (w, h), source) = filter(&content, monitor, sel)?;

    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setWidth(w as usize);
        config.setHeight(h as usize);
        if let Some(rect) = source {
            config.setSourceRect(rect);
        }
        config.setShowsCursor(true);
        config.setCapturesAudio(true);
        config.setExcludesCurrentProcessAudio(true);
        // ponytail: frame interval left at SCK's default (up to the display's refresh rate); 30 fps needs
        // `setMinimumFrameInterval(CMTime)` from objc2-core-media, add it with that crate if files run large.
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
    await_error("start", |h| unsafe { stream.startCaptureWithCompletionHandler(Some(h)) })?;
    Ok(Running { stream, _output: output, _delegate: delegate, done, out: out.to_path_buf() })
}
