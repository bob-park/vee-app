//! Edit domain: the pixel ops the image editor window applies to a clip image.

mod model;

pub use model::{BrushMode, MAX_SIDE, Op, PREVIEW_SIDE, Stroke, SubjectPoint, UPSCALE_CAP};

use crate::shared::error::{AppError, AppResult};
use ::image::{DynamicImage, ImageFormat, imageops::FilterType};
use std::io::Cursor;

/// Applies a crop, rotate or resize, or paints brush strokes. AI ops are rejected; they run elsewhere.
pub fn apply_geometric(img: DynamicImage, op: &Op) -> AppResult<DynamicImage> {
    match op {
        &Op::Crop { x, y, w, h } => {
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
        &Op::Resize { w, h } => {
            if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
                return Err(AppError::InvalidInput(format!("bad resize {w}x{h}")));
            }
            Ok(img.resize_exact(w, h, FilterType::Lanczos3))
        }
        Op::Brush { strokes } => paint_strokes(img, strokes),
        Op::RemoveBg | Op::Upscale { .. } | Op::Subject { .. } => Err(AppError::InvalidInput("not geometric".into())),
    }
}

/// Erases or restores alpha along each stroke, with a soft edge. RGB is never touched.
pub fn paint_strokes(img: DynamicImage, strokes: &[Stroke]) -> AppResult<DynamicImage> {
    if let Some(s) = strokes.iter().find(|s| s.radius == 0 || s.radius > MAX_SIDE) {
        return Err(AppError::InvalidInput(format!("bad brush radius {}", s.radius)));
    }
    let mut buf = img.into_rgba8();
    let (iw, ih) = (buf.width() as i64, buf.height() as i64);
    for stroke in strokes {
        let r = stroke.radius as f32;
        let feather = (r / 4.0).max(1.0);
        let pad = stroke.radius as i64;
        // Bounding box of the whole stroke ± radius, clipped to the image.
        let clip = |lo: Option<u32>, hi: Option<u32>, max: i64| match (lo, hi) {
            (Some(lo), Some(hi)) => ((lo as i64 - pad).max(0), (hi as i64 + pad).min(max - 1)),
            _ => (0, -1),
        };
        let (sx0, sx1) = clip(stroke.points.iter().map(|p| p[0]).min(), stroke.points.iter().map(|p| p[0]).max(), iw);
        let (sy0, sy1) = clip(stroke.points.iter().map(|p| p[1]).min(), stroke.points.iter().map(|p| p[1]).max(), ih);
        if sx0 > sx1 || sy0 > sy1 {
            continue;
        }
        // Coverage from the distance to the whole polyline: the max over its segments.
        let bw = (sx1 - sx0 + 1) as usize;
        let mut cov = vec![0f32; bw * (sy1 - sy0 + 1) as usize];
        // A single point is a zero-length segment.
        let segs: Vec<_> = match stroke.points.as_slice() {
            [p] => vec![(*p, *p)],
            pts => pts.windows(2).map(|w| (w[0], w[1])).collect(),
        };
        for ([ax, ay], [bx, by]) in segs {
            let x0 = (ax.min(bx) as i64 - pad).max(sx0);
            let y0 = (ay.min(by) as i64 - pad).max(sy0);
            let x1 = (ax.max(bx) as i64 + pad).min(sx1);
            let y1 = (ay.max(by) as i64 + pad).min(sy1);
            let (a, b) = ((ax as f32, ay as f32), (bx as f32, by as f32));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let d = segment_distance((x as f32, y as f32), a, b);
                    let c = &mut cov[(y - sy0) as usize * bw + (x - sx0) as usize];
                    *c = c.max(((r - d) / feather).clamp(0.0, 1.0));
                }
            }
        }
        for (i, &c) in cov.iter().enumerate() {
            if c == 0.0 {
                continue;
            }
            let (x, y) = (sx0 as usize + i % bw, sy0 as usize + i / bw);
            let alpha = &mut buf.get_pixel_mut(x as u32, y as u32).0[3];
            let v = *alpha as f32;
            *alpha = match stroke.mode {
                BrushMode::Erase => v * (1.0 - c),
                BrushMode::Restore => v + (255.0 - v) * c,
            }
            .round() as u8;
        }
    }
    Ok(DynamicImage::ImageRgba8(buf))
}

/// Distance from `p` to the segment `a`–`b`.
fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    let (cx, cy) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
    (cx * cx + cy * cy).sqrt()
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
        assert!(matches!(apply_geometric(blank(10, 10), &Op::Subject { points: vec![], lasso: None }), Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn subject_lasso_serde() {
        let wire = r#"{"type":"subject","points":[],"lasso":[[1,2],[3,4],[5,6]]}"#;
        let op: Op = serde_json::from_str(wire).unwrap();
        assert_eq!(op, Op::Subject { points: vec![], lasso: Some(vec![[1, 2], [3, 4], [5, 6]]) });
        assert_eq!(serde_json::to_string(&op).unwrap(), wire);
        // The old shape still parses, and goes back out without a `lasso` key.
        let old = r#"{"type":"subject","points":[{"x":1,"y":2,"keep":true}]}"#;
        let op: Op = serde_json::from_str(old).unwrap();
        assert!(matches!(&op, Op::Subject { lasso: None, .. }));
        assert_eq!(serde_json::to_string(&op).unwrap(), old);
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

    #[test]
    fn op_serde_new_variants() {
        for wire in [
            r#"{"type":"subject","points":[{"x":3,"y":4,"keep":false}]}"#,
            r#"{"type":"brush","strokes":[{"mode":"restore","radius":5,"points":[[1,2]]}]}"#,
        ] {
            let op: Op = serde_json::from_str(wire).unwrap();
            assert_eq!(serde_json::to_string(&op).unwrap(), wire);
        }
    }

    fn solid(alpha: u8) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([10, 20, 30, alpha])))
    }

    fn brush(mode: BrushMode, radius: u32, points: Vec<[u32; 2]>) -> Op {
        Op::Brush { strokes: vec![Stroke { mode, radius, points }] }
    }

    fn px(img: &DynamicImage, x: u32, y: u32) -> [u8; 4] {
        img.to_rgba8().get_pixel(x, y).0
    }

    #[test]
    fn brush_erase_clears_alpha_keeps_rgb() {
        let out = apply_geometric(solid(255), &brush(BrushMode::Erase, 4, vec![[10, 10]])).unwrap();
        assert_eq!(px(&out, 10, 10), [10, 20, 30, 0]);
        assert_eq!(px(&out, 0, 0)[3], 255);
    }

    #[test]
    fn brush_line_fills_between_points() {
        let out = apply_geometric(solid(255), &brush(BrushMode::Erase, 2, vec![[2, 10], [17, 10]])).unwrap();
        assert_eq!(px(&out, 10, 10)[3], 0);
        assert_eq!(px(&out, 10, 0)[3], 255);
    }

    #[test]
    fn brush_restore_sets_alpha() {
        let out = apply_geometric(solid(0), &brush(BrushMode::Restore, 4, vec![[10, 10]])).unwrap();
        assert_eq!(px(&out, 10, 10), [10, 20, 30, 255]);
    }

    #[test]
    fn brush_clamps_outside_points() {
        let out = apply_geometric(solid(255), &brush(BrushMode::Erase, 3, vec![[500, 500]])).unwrap();
        assert_eq!(out.to_rgba8(), solid(255).to_rgba8());
    }

    #[test]
    fn brush_rejects_zero_radius() {
        let r = apply_geometric(solid(255), &brush(BrushMode::Erase, 0, vec![[10, 10]]));
        assert!(matches!(r, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn brush_coverage_uses_the_whole_polyline() {
        let split = apply_geometric(solid(255), &brush(BrushMode::Erase, 8, vec![[2, 10], [10, 10], [17, 10]])).unwrap();
        let line = apply_geometric(solid(255), &brush(BrushMode::Erase, 8, vec![[2, 10], [17, 10]])).unwrap();
        assert_eq!(split.to_rgba8(), line.to_rgba8());
        let a = px(&line, 10, 17)[3];
        assert!(a > 0 && a < 255, "feather band alpha {a}");
    }
}
