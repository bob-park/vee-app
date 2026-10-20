use serde::Serialize;
use std::path::PathBuf;

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
}

/// Every target format, in the donut's slice order.
pub const TARGETS: [Format; 8] =
    [Format::Png, Format::Jpg, Format::Webp, Format::Heic, Format::Tiff, Format::Gif, Format::Bmp, Format::Pdf];

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
        }
    }

    /// The source format for a file extension (any case). `pdf` is a target only, so it is `None`.
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
    pub files: Vec<PathBuf>,
    pub stem: String,
    pub dir: PathBuf,
    pub done: usize,
    pub state: JobState,
    /// Ms when the job was queued.
    pub since: i64,
    /// Ms when the job finished or failed.
    pub ended: Option<i64>,
    /// Whether the panel has shown it yet.
    pub shown: bool,
}

/// What the panel draws for one job.
#[derive(Debug, Serialize)]
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
}
