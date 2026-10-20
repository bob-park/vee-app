use serde::Serialize;

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
