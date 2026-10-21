use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Png,
    Jpg,
    Webp,
    Heic,
    Tiff,
    Gif,
    Bmp,
    Pdf,
    Mp4,
    Webm,
    Mov,
    Mp3,
    /// MP4 scaled down to at most 720 lines.
    #[serde(rename = "720p")]
    Mp4720,
    Hwp,
    Hwpx,
    Docx,
    Odt,
    Rtf,
    Txt,
    Html,
    Xlsx,
    Ods,
    Pptx,
    Odp,
}

/// Image target formats, in the donut's slice order.
pub const IMAGE_TARGETS: [Format; 8] =
    [Format::Png, Format::Jpg, Format::Webp, Format::Heic, Format::Tiff, Format::Gif, Format::Bmp, Format::Pdf];
/// Video target formats, in the donut's slice order. `Gif` is shared with images.
pub const VIDEO_TARGETS: [Format; 6] = [Format::Mp4, Format::Webm, Format::Mov, Format::Gif, Format::Mp3, Format::Mp4720];

/// A document family's targets in donut order, each with the engine it runs on.
pub type DocTargets = &'static [(Format, Engine)];
const RHWP: Engine = Engine::Rhwp;
const LO: Engine = Engine::LibreOffice;
pub const HWP_TARGETS: DocTargets = &[(Format::Pdf, RHWP), (Format::Hwpx, RHWP), (Format::Hwp, RHWP)];
/// Word-processor documents and plain text / HTML alike: LibreOffice Writer reads them all.
pub const TEXT_TARGETS: DocTargets =
    &[(Format::Pdf, LO), (Format::Docx, LO), (Format::Odt, LO), (Format::Txt, LO), (Format::Rtf, LO), (Format::Html, LO)];
pub const SHEET_TARGETS: DocTargets = &[(Format::Pdf, LO), (Format::Xlsx, LO), (Format::Ods, LO)];
pub const SLIDE_TARGETS: DocTargets = &[(Format::Pdf, LO), (Format::Pptx, LO), (Format::Odp, LO)];

/// A converter that has to be installed before its slices work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Ffmpeg,
    Rhwp,
    LibreOffice,
}

/// One donut slice: a target format, and the engine it still needs (dimmed while `Some`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slice {
    pub format: Format,
    pub needs: Option<Engine>,
}

/// What this machine can convert with right now.
#[derive(Debug, Clone, Copy)]
pub struct Caps {
    /// HEIC read/write (macOS `sips`).
    pub heic: bool,
    /// FFmpeg is installed.
    pub ffmpeg: bool,
    /// rhwp is installed.
    pub rhwp: bool,
    /// LibreOffice is installed.
    pub libreoffice: bool,
}

impl Caps {
    /// Whether `engine` is available now.
    pub fn has(self, engine: Engine) -> bool {
        match engine {
            Engine::Ffmpeg => self.ffmpeg,
            Engine::Rhwp => self.rhwp,
            Engine::LibreOffice => self.libreoffice,
        }
    }
}

impl Format {
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpg => "jpg",
            Format::Webp => "webp",
            Format::Heic => "heic",
            Format::Tiff => "tiff",
            Format::Gif => "gif",
            Format::Bmp => "bmp",
            Format::Pdf => "pdf",
            Format::Mp4 | Format::Mp4720 => "mp4",
            Format::Webm => "webm",
            Format::Mov => "mov",
            Format::Mp3 => "mp3",
            Format::Hwp => "hwp",
            Format::Hwpx => "hwpx",
            Format::Docx => "docx",
            Format::Odt => "odt",
            Format::Rtf => "rtf",
            Format::Txt => "txt",
            Format::Html => "html",
            Format::Xlsx => "xlsx",
            Format::Ods => "ods",
            Format::Pptx => "pptx",
            Format::Odp => "odp",
        }
    }

    /// The serde name: unique per format, unlike `ext` (`720p` writes `.mp4`).
    pub fn id(self) -> &'static str {
        match self {
            Format::Mp4720 => "720p",
            f => f.ext(),
        }
    }

    /// The image source format for a file extension (any case). `pdf` is a target only, so it is `None`.
    pub fn from_ext(ext: &str) -> Option<Format> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Format::Png),
            "jpg" | "jpeg" => Some(Format::Jpg),
            "webp" => Some(Format::Webp),
            "heic" | "heif" => Some(Format::Heic),
            "tif" | "tiff" => Some(Format::Tiff),
            "gif" => Some(Format::Gif),
            "bmp" => Some(Format::Bmp),
            _ => None,
        }
    }
}

/// Why one conversion job failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JobError {
    /// The source file is gone.
    Missing,
    /// The engine could not read or convert it.
    Engine,
    /// The output could not be written to the destination folder.
    Write,
    /// The engine needs the Microsoft Visual C++ runtime (Windows rhwp).
    Runtime,
    /// The user cancelled a running video job. Never shown: the worker drops the job.
    Cancelled,
}

/// Video encoding quality (the `videoQuality` setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Small,
    Normal,
    Original,
}

impl Quality {
    /// The setting value; anything unknown is `Normal`.
    pub fn parse(value: &str) -> Quality {
        match value {
            "small" => Quality::Small,
            "original" => Quality::Original,
            _ => Quality::Normal,
        }
    }

    /// Picks the value for this quality from a `[small, normal, original]` triple.
    pub fn pick<T>(self, [small, normal, original]: [T; 3]) -> T {
        match self {
            Quality::Small => small,
            Quality::Normal => normal,
            Quality::Original => original,
        }
    }
}

/// A queued or running job shows in the panel only after this long, so quick ones never flash.
pub const SHOW_AFTER_MS: i64 = 1000;
/// How long a finished job that was shown stays in the panel.
pub const DONE_MS: i64 = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed(JobError),
}

/// One convert request: every file of a clip into one format.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: u64,
    pub clip_id: i64,
    pub format: Format,
    /// Each output is named after its source file's stem.
    pub files: Vec<PathBuf>,
    pub dir: PathBuf,
    pub done: usize,
    pub state: JobState,
    /// Ms when the job was queued.
    pub since: i64,
    /// Ms when the job finished or failed.
    pub ended: Option<i64>,
    /// Whether the panel has shown it yet.
    pub shown: bool,
    /// Done share of the file being converted now; `None` for images or an unknown video length.
    pub progress: Option<f64>,
    /// Ms when the job last started running.
    pub started: Option<i64>,
    /// Set by `cancel_convert` to stop a running video job.
    pub cancel: Arc<AtomicBool>,
}

impl Job {
    pub fn new(id: u64, clip_id: i64, format: Format, files: Vec<PathBuf>, dir: PathBuf, since: i64) -> Job {
        Job {
            id,
            clip_id,
            format,
            files,
            dir,
            done: 0,
            state: JobState::Queued,
            since,
            ended: None,
            shown: false,
            progress: None,
            started: None,
            cancel: Arc::default(),
        }
    }

    /// Marks the job running from `now`. Clears a cancel that landed too late to stop the last run,
    /// so a retry doesn't stop at once.
    pub fn start(&mut self, now: i64) {
        self.state = JobState::Running;
        self.started = Some(now);
        self.progress = None;
        self.cancel.store(false, Ordering::SeqCst);
    }
}

/// Seconds left after `elapsed_ms` at done share `p`; `None` below 3 %, where the guess is noise.
pub fn eta(elapsed_ms: i64, p: f64) -> Option<u32> {
    (p >= 0.03).then(|| (elapsed_ms.max(0) as f64 / 1000.0 * (1.0 - p) / p).round() as u32)
}

/// What the panel draws for one job.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: u64,
    pub clip_id: i64,
    pub format: Format,
    /// "queued" | "running" | "done" | "failed"
    pub state: &'static str,
    /// 1-based among queued jobs, 0 otherwise.
    pub position: usize,
    pub done: usize,
    pub total: usize,
    /// Last path component of the destination folder.
    pub folder: String,
    pub error: Option<JobError>,
    /// Overall percent (0–99) of a running job whose current file reports progress.
    pub percent: Option<u8>,
    /// Seconds left, once `percent` is past 3 %.
    pub eta: Option<u32>,
    /// A running video job, which `cancel_convert` can stop.
    pub cancellable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_clears_a_stale_cancel() {
        let mut job = Job::new(1, 1, Format::Mp4, vec![], PathBuf::new(), 0);
        job.cancel.store(true, Ordering::SeqCst);
        job.progress = Some(0.5);
        job.start(42);
        assert!(!job.cancel.load(Ordering::SeqCst));
        assert_eq!((job.state, job.started, job.progress), (JobState::Running, Some(42), None));
    }

    #[test]
    fn eta_scales_elapsed_by_what_is_left() {
        assert_eq!(eta(10_000, 0.5), Some(10));
        assert_eq!(eta(1_000, 0.02), None);
        assert_eq!(eta(3_000, 0.75), Some(1));
    }
}
