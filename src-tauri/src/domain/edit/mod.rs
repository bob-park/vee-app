//! Edit domain: the pixel ops the image editor window applies to a clip image.

mod model;

pub use model::{MAX_SIDE, Op, PREVIEW_SIDE, UPSCALE_CAP};

use crate::shared::error::{AppError, AppResult};
use ::image::{DynamicImage, ImageFormat, imageops::FilterType};
use std::io::Cursor;

/// Applies a crop, rotate or resize. AI ops are rejected; they run elsewhere.
pub fn apply_geometric(img: DynamicImage, op: &Op) -> AppResult<DynamicImage> {
    match *op {
        Op::Crop { x, y, w, h } => {
            let (iw, ih) = (img.width(), img.height());
            let (x, y) = (x.min(iw), y.min(ih));
            let (w, h) = (w.min(iw - x), h.min(ih - y));
            if w == 0 || h == 0 {
                return Err(AppError::InvalidInput("crop is outside the image".into()));
            }
            Ok(img.crop_imm(x, y, w, h))
        }
        Op::Rotate { quarter: 1 } => Ok(img.rotate90()),
        Op::Rotate { quarter: -1 } => Ok(img.rotate270()),
        Op::Rotate { quarter } => Err(AppError::InvalidInput(format!("bad rotate quarter {quarter}"))),
        Op::Resize { w, h } => {
            if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
                return Err(AppError::InvalidInput(format!("bad resize {w}x{h}")));
            }
            Ok(img.resize_exact(w, h, FilterType::Lanczos3))
        }
        Op::RemoveBg | Op::Upscale { .. } => Err(AppError::InvalidInput("not geometric".into())),
    }
}

/// `w*scale × h*scale`, shrunk proportionally so the long side is at most `UPSCALE_CAP`.
pub fn upscale_size(w: u32, h: u32, scale: u8) -> (u32, u32) {
    let (w, h) = (w as u64 * scale as u64, h as u64 * scale as u64);
    let cap = UPSCALE_CAP as u64;
    let (w, h) = if w >= h && w > cap {
        (cap, h * cap / w)
    } else if h > w && h > cap {
        (w * cap / h, cap)
    } else {
        (w, h)
    };
    (w.max(1) as u32, h.max(1) as u32)
}

/// Upscale without a model: Lanczos3 to `upscale_size`, then a light unsharp mask.
pub fn upscale_fallback(img: &DynamicImage, scale: u8) -> DynamicImage {
    let (w, h) = upscale_size(img.width(), img.height(), scale);
    img.resize_exact(w, h, FilterType::Lanczos3).unsharpen(1.0, 2)
}

/// Shrinks `w × h` so the long side is at most `side`, rounding to nearest. Never upsizes.
pub fn fit(w: u32, h: u32, side: u32) -> (u32, u32) {
    let long = w.max(h);
    if long <= side {
        return (w, h);
    }
    let scaled = |v: u32| ((v as u64 * side as u64 + long as u64 / 2) / long as u64).max(1) as u32;
    (scaled(w), scaled(h))
}

pub fn to_png(img: &DynamicImage) -> AppResult<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).map_err(|e| AppError::Other(e.to_string()))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::error::AppError;
    use ::image::{DynamicImage, Rgba, RgbaImage};

    fn blank(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn crop_is_clamped_into_the_image() {
        let out = apply_geometric(blank(100, 50), &Op::Crop { x: 80, y: 40, w: 50, h: 50 }).unwrap();
        assert_eq!((out.width(), out.height()), (20, 10));
    }

    #[test]
    fn crop_outside_the_image_is_rejected() {
        let r = apply_geometric(blank(100, 50), &Op::Crop { x: 200, y: 0, w: 10, h: 10 });
        assert!(matches!(r, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rotate_swaps_sides() {
        let out = apply_geometric(blank(100, 50), &Op::Rotate { quarter: 1 }).unwrap();
        assert_eq!((out.width(), out.height()), (50, 100));

        let mut src = RgbaImage::new(100, 50);
        src.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let out = apply_geometric(DynamicImage::ImageRgba8(src), &Op::Rotate { quarter: -1 }).unwrap();
        assert_eq!((out.width(), out.height()), (50, 100));
        // Counter-clockwise: the top-left corner moves to the bottom-left.
        assert_eq!(out.to_rgba8().get_pixel(0, 99), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn resize_rejects_zero_and_huge() {
        assert!(matches!(apply_geometric(blank(10, 10), &Op::Resize { w: 0, h: 10 }), Err(AppError::InvalidInput(_))));
        assert!(matches!(apply_geometric(blank(10, 10), &Op::Resize { w: 16385, h: 10 }), Err(AppError::InvalidInput(_))));
        let out = apply_geometric(blank(10, 10), &Op::Resize { w: 40, h: 20 }).unwrap();
        assert_eq!((out.width(), out.height()), (40, 20));
    }

    #[test]
    fn ai_ops_are_not_geometric() {
        assert!(matches!(apply_geometric(blank(10, 10), &Op::RemoveBg), Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn upscale_size_caps_long_side() {
        assert_eq!(upscale_size(420, 180, 2), (840, 360));
        assert_eq!(upscale_size(3000, 1000, 4), (4096, 1365));
        assert_eq!(upscale_size(1000, 3000, 4), (1365, 4096));
    }

    #[test]
    fn fit_never_upsizes() {
        assert_eq!(fit(100, 50, 2048), (100, 50));
        assert_eq!(fit(6016, 3384, PREVIEW_SIDE), (2048, 1152));
        assert_eq!(fit(3384, 6016, 2048), (1152, 2048));
        assert_eq!(fit(10000, 1, 2048), (2048, 1));
    }

    #[test]
    fn upscale_fallback_hits_target_size() {
        let out = upscale_fallback(&blank(10, 5), 2);
        assert_eq!((out.width(), out.height()), (20, 10));
    }

    #[test]
    fn to_png_writes_a_png() {
        let bytes = to_png(&blank(3, 2)).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn op_serde_matches_the_wire() {
        let rot: Op = serde_json::from_str(r#"{"type":"rotate","quarter":-1}"#).unwrap();
        assert_eq!(rot, Op::Rotate { quarter: -1 });
        assert_eq!(serde_json::to_string(&rot).unwrap(), r#"{"type":"rotate","quarter":-1}"#);
        let bg: Op = serde_json::from_str(r#"{"type":"removeBg"}"#).unwrap();
        assert_eq!(bg, Op::RemoveBg);
        assert_eq!(serde_json::to_string(&bg).unwrap(), r#"{"type":"removeBg"}"#);
    }
}
