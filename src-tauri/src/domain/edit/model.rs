use serde::{Deserialize, Serialize};

/// Largest side a resize may ask for.
pub const MAX_SIDE: u32 = 16384;
/// Largest long side an upscale may produce.
pub const UPSCALE_CAP: u32 = 4096;
/// Long side of the preview the editor window renders.
pub const PREVIEW_SIDE: u32 = 2048;

/// One step of an edit, as the editor window sends it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Op {
    Crop {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    },
    /// `1` turns clockwise, `-1` counter-clockwise.
    Rotate {
        quarter: i8,
    },
    Resize {
        w: u32,
        h: u32,
    },
    RemoveBg,
    /// `2` or `4`.
    Upscale {
        scale: u8,
    },
    /// Cut out the subject marked by clicks on the image, and optionally a lasso drawn around it:
    /// a polygon in the same px as the points. Its bbox goes to SAM as a box prompt, and alpha
    /// outside it (plus a margin) is cleared.
    Subject {
        points: Vec<SubjectPoint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lasso: Option<Vec<[u32; 2]>>,
    },
    /// Paint alpha by hand: erase or restore along strokes.
    Brush {
        strokes: Vec<Stroke>,
    },
}

/// A click for `Op::Subject`, in full-res px of the image the ops before it produce. `keep: false` marks background.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubjectPoint {
    pub x: u32,
    pub y: u32,
    pub keep: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrushMode {
    Erase,
    Restore,
}

/// One drag of the brush: a polyline in full-res px of the image the ops before it produce,
/// painted with a round brush of `radius` (so the stroke is `2 × radius` wide).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stroke {
    pub mode: BrushMode,
    pub radius: u32,
    pub points: Vec<[u32; 2]>,
}
