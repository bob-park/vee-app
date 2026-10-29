use serde::{Deserialize, Serialize};

/// Largest side a resize may ask for.
pub const MAX_SIDE: u32 = 16384;
/// Largest long side an upscale may produce.
pub const UPSCALE_CAP: u32 = 4096;
/// Long side of the preview the editor window renders.
pub const PREVIEW_SIDE: u32 = 2048;

/// One step of an edit, as the editor window sends it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Op {
    Crop { x: u32, y: u32, w: u32, h: u32 },
    /// `1` turns clockwise, `-1` counter-clockwise.
    Rotate { quarter: i8 },
    Resize { w: u32, h: u32 },
    RemoveBg,
    /// `2` or `4`.
    Upscale { scale: u8 },
}
