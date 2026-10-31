//! On-device image models through ONNX Runtime: background removal with BiRefNet, subject cut
//! with SAM (ViT-B), and the upscale entry point (Lanczos only; no model passed the glyph check).
//! The models run in the worker process (`ai_worker`); the app itself never loads ORT.

use crate::app::ai_worker::{self, Request};
use crate::app::engines::{self, EngineId};
use crate::domain::edit::{SubjectPoint, upscale_fallback};
use crate::shared::error::{AppError, AppResult};
use ::image::{DynamicImage, GrayImage, Luma, RgbaImage, imageops, imageops::FilterType};
use ort::{
    environment::Environment,
    ep,
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;
use tauri::AppHandle;

/// BiRefNet's fixed input side.
const SIDE: u32 = 1024;
/// ImageNet mean/std, as rembg feeds BiRefNet.
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// SAM's encoder input, `W × H`: the source fits inside, top-left, zero padded (rembg's `input_size`).
const SAM_W: u32 = 1024;
const SAM_H: u32 = 684;

/// ORT allows one environment per process, so it lives until the worker exits once built.
/// Sessions are not kept between runs: BiRefNet holds GBs while loaded, and reloading costs about a second.
static ENV: OnceLock<Environment> = OnceLock::new();

fn ort_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("onnx runtime: {e}"))
}

/// Every model file of an installed model engine in `dir`, in `binaries` order.
fn model_paths(dir: &Path, id: EngineId) -> AppResult<Vec<PathBuf>> {
    let paths: Vec<PathBuf> = id.spec().binaries.iter().map(|b| dir.join(b)).collect();
    if !paths.iter().all(|p| p.is_file()) {
        return Err(AppError::Other("model missing".into()));
    }
    Ok(paths)
}

/// The ONNX Runtime library the app ships (`scripts/fetch-onnxruntime.mjs`, bundled by `tauri.<os>.conf.json`).
const ORT_LIB: &str = if cfg!(windows) { "onnxruntime.dll" } else { "libonnxruntime.dylib" };

/// Where the bundle keeps the runtime: macOS `Contents/Frameworks/` (signed with the app, so the
/// hardened runtime loads it), Windows the install dir beside the exe.
fn bundled_runtime(exe_dir: &Path) -> PathBuf {
    if cfg!(target_os = "macos") { exe_dir.join("../Frameworks").join(ORT_LIB) } else { exe_dir.join(ORT_LIB) }
}

/// The runtime as fetched into the source tree; what dev builds and tests load.
pub(crate) fn vendor_runtime() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/onnxruntime").join(ORT_LIB)
}

/// The bundled runtime, or in a debug build (not bundled on macOS) the vendored one.
fn runtime_path() -> AppResult<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| AppError::Other(format!("onnx runtime: {e}")))?;
    let mut candidates = vec![bundled_runtime(exe.parent().unwrap_or(Path::new(".")))];
    if cfg!(debug_assertions) {
        candidates.push(vendor_runtime());
    }
    candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| AppError::Other("onnx runtime missing from the app".into()))
}

/// NCHW f32 at `side × side`, `(v / max - mean) / std` per channel (rembg's `normalize`).
/// Scales with the integer thumbnail filter, so a large source never gets a float copy.
fn to_input(img: &DynamicImage, side: u32) -> Vec<f32> {
    let small = if img.width() > side || img.height() > side {
        img.thumbnail_exact(side, side)
    } else {
        img.resize_exact(side, side, FilterType::Triangle)
    };
    let rgb = small.to_rgb8();
    let max = rgb.as_raw().iter().copied().max().unwrap_or(1).max(1) as f32;
    let plane = (side * side) as usize;
    let mut data = vec![0f32; 3 * plane];
    for (i, p) in rgb.pixels().enumerate() {
        for c in 0..3 {
            data[c * plane + i] = (p[c] as f32 / max - MEAN[c]) / STD[c];
        }
    }
    data
}

/// The model's `side × side` logits as a mask: sigmoid, then stretched to 0..255.
fn from_output(pred: &[f32], side: u32) -> GrayImage {
    let pred: Vec<f32> = pred.iter().take((side * side) as usize).map(|&v| 1.0 / (1.0 + (-v).exp())).collect();
    let (lo, hi) = pred.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let span = (hi - lo).max(1e-6);
    GrayImage::from_fn(side, side, |x, y| {
        let v = pred.get((y * side + x) as usize).copied().unwrap_or(lo);
        Luma([(((v - lo) / span) * 255.0) as u8])
    })
}

/// Sets `img`'s alpha to `mask`, resized to `img`'s size.
fn apply_mask(mut img: RgbaImage, mask: &GrayImage) -> RgbaImage {
    let mask = imageops::resize(mask, img.width(), img.height(), FilterType::Lanczos3);
    for (p, m) in img.pixels_mut().zip(mask.pixels()) {
        p[3] = m[0];
    }
    img
}

/// The shared environment, loading `lib` on first use.
fn env(lib: &Path) -> AppResult<&'static Environment> {
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let env = ort::init_from(lib).map_err(ort_err)?.build().map_err(ort_err)?;
    Ok(ENV.get_or_init(|| env))
}

/// A CPU session tuned for peak memory: no arena, no memory pattern, at most 4 threads.
fn load(env: &Environment, model: &Path) -> AppResult<Session> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(4);
    Session::builder(env)
        .map_err(ort_err)?
        .with_execution_providers([ep::CPU::default().with_arena_allocator(false).build()])
        .map_err(ort_err)?
        .with_memory_pattern(false)
        .map_err(ort_err)?
        .with_intra_threads(threads)
        .map_err(ort_err)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(ort_err)?
        .commit_from_file(model)
        .map_err(ort_err)
}

/// Runs BiRefNet from `model` (loading `lib` and the session on first use) and returns `img` as
/// RGBA with the mask as alpha.
fn cut(lib: &Path, model: &Path, img: &DynamicImage) -> AppResult<DynamicImage> {
    if img.width() == 0 || img.height() == 0 {
        return Err(AppError::InvalidInput("empty image".into()));
    }
    let data = to_input(img, SIDE);
    let mask = {
        let start = Instant::now();
        let mut session = load(env(lib)?, model)?;
        log::info!("ai: loaded background model in {} ms", start.elapsed().as_millis());
        // Any ORT call (even building a tensor) needs the library loaded first, or ort panics.
        let input = Tensor::from_array(([1usize, 3, SIDE as usize, SIDE as usize], data)).map_err(ort_err)?;
        let start = Instant::now();
        let outputs = session.run(ort::inputs![input]).map_err(ort_err)?;
        let out = outputs.values().next().ok_or_else(|| AppError::Other("onnx runtime: no output".into()))?;
        let (_, pred) = out.try_extract_tensor::<f32>().map_err(ort_err)?;
        let mask = from_output(pred, SIDE);
        log::info!("ai: background removed from {}x{} in {} ms", img.width(), img.height(), start.elapsed().as_millis());
        mask
    };
    Ok(DynamicImage::ImageRgba8(apply_mask(img.to_rgba8(), &mask)))
}

/// SAM's encoder output for one `w × h` source, scaled by `scale` into the encoder input.
/// Reused for every click on the same image; the sessions themselves are not kept.
pub(crate) struct SamEmbedding {
    data: Vec<f32>,
    shape: Vec<usize>,
    scale: f32,
    w: u32,
    h: u32,
}

/// `w × h` scaled by `scale`, as it sits in SAM's input.
fn sam_size(w: u32, h: u32, scale: f32) -> (u32, u32) {
    (((w as f32 * scale).round() as u32).clamp(1, SAM_W), ((h as f32 * scale).round() as u32).clamp(1, SAM_H))
}

/// The encoder input, HWC f32 `SAM_H × SAM_W × 3` of raw 0..255 RGB (the model normalizes), and
/// the scale the source was fitted by.
fn sam_input(img: &DynamicImage) -> (Vec<f32>, f32) {
    let (w, h) = (img.width(), img.height());
    let scale = (SAM_W as f32 / w as f32).min(SAM_H as f32 / h as f32);
    let (nw, nh) = sam_size(w, h, scale);
    // The integer thumbnail filter when shrinking, as in `to_input`.
    let small = if nw < w { img.thumbnail_exact(nw, nh) } else { img.resize_exact(nw, nh, FilterType::Triangle) }.to_rgb8();
    let mut data = vec![0f32; (SAM_W * SAM_H * 3) as usize];
    for (x, y, p) in small.enumerate_pixels() {
        let i = ((y * SAM_W + x) * 3) as usize;
        data[i..i + 3].copy_from_slice(&p.0.map(f32::from));
    }
    (data, scale)
}

/// Decoder `point_coords` (flat `[n+1, 2]`, or `[n+3, 2]` with a box) and `point_labels`: the
/// points in input space, keep → 1 and background → 0, then `bbox`'s corners as labels 2 and 3,
/// then the `(0, 0)` / −1 pad (rembg's `get_input_points`).
fn sam_prompt(points: &[SubjectPoint], bbox: Option<[u32; 4]>, scale: f32) -> (Vec<f32>, Vec<f32>) {
    let corners = bbox.map(|[x0, y0, x1, y1]| [(x0, y0, 2.0), (x1, y1, 3.0)]).into_iter().flatten();
    let all: Vec<(u32, u32, f32)> = points.iter().map(|p| (p.x, p.y, if p.keep { 1.0 } else { 0.0 })).chain(corners).collect();
    let coords = all.iter().flat_map(|&(x, y, _)| [x as f32 * scale, y as f32 * scale]).chain([0.0, 0.0]).collect();
    let labels = all.iter().map(|&(_, _, l)| l).chain([-1.0]).collect();
    (coords, labels)
}

/// `lasso`'s bounding box, `[x0, y0, x1, y1]`.
fn lasso_box(lasso: &[[u32; 2]]) -> [u32; 4] {
    lasso.iter().fold([u32::MAX, u32::MAX, 0, 0], |[x0, y0, x1, y1], &[x, y]| [x0.min(x), y0.min(y), x1.max(x), y1.max(y)])
}

/// How far past the lasso the clip keeps alpha: `max(8, 2 % of the long side)` px.
fn clip_margin(w: u32, h: u32) -> u32 {
    ((w.max(h) as f32 * 0.02).round() as u32).max(8)
}

/// Where the edges of the closed polygon `lasso` cross the horizontal line at `yc`, unsorted.
fn crossings(lasso: &[[u32; 2]], yc: f32, out: &mut Vec<f32>) {
    out.clear();
    for (i, &[ax, ay]) in lasso.iter().enumerate() {
        let [bx, by] = lasso[(i + 1) % lasso.len()];
        let (ay, by) = (ay as f32, by as f32);
        if (ay <= yc) != (by <= yc) {
            out.push(ax as f32 + (yc - ay) / (by - ay) * (bx as f32 - ax as f32));
        }
    }
}

/// Whether pixel `(x, y)`'s centre is inside `lasso` (even-odd), as `lasso_mask` fills it.
fn point_in_lasso(lasso: &[[u32; 2]], x: u32, y: u32) -> bool {
    let mut xs = Vec::new();
    crossings(lasso, y as f32 + 0.5, &mut xs);
    // `<=` matches the fill's `[a, b)` spans: a centre on a crossing counts as past it.
    xs.iter().filter(|&&cx| cx <= x as f32 + 0.5).count() % 2 == 1
}

/// `w × h`, 255 inside `lasso` (even-odd scanline fill at pixel centres, so a self-crossing path
/// works) dilated by a `margin` px square, 0 elsewhere. Each filled span is widened by `margin`
/// as it's drawn, then a running per-column count over `2 · margin + 1` rows does the vertical
/// part, so it's O(w · h) whatever the margin.
fn lasso_mask(lasso: &[[u32; 2]], w: u32, h: u32, margin: u32) -> GrayImage {
    let (wu, hu, r) = (w as usize, h as usize, margin as usize);
    let mut rows = vec![0u8; wu * hu];
    let mut xs = Vec::new();
    for (y, row) in rows.chunks_exact_mut(wu.max(1)).enumerate() {
        crossings(lasso, y as f32 + 0.5, &mut xs);
        xs.sort_by(f32::total_cmp);
        for pair in xs.chunks_exact(2) {
            // Centres `x + 0.5` in `[a, b)`, widened by `r`.
            let a = ((pair[0] - 0.5).ceil() - r as f32).clamp(0.0, w as f32) as usize;
            let b = ((pair[1] - 0.5).ceil() + r as f32).clamp(0.0, w as f32) as usize;
            row[a..b.max(a)].fill(255);
        }
    }
    if r == 0 || wu == 0 {
        return GrayImage::from_raw(w, h, rows).expect("w × h buffer");
    }
    let mut out = vec![0u8; wu * hu];
    let mut count = vec![0u32; wu];
    let add = |count: &mut [u32], row: &[u8], d: i32| {
        for (c, &v) in count.iter_mut().zip(row) {
            *c = c.wrapping_add_signed(d * (v != 0) as i32);
        }
    };
    for y in 0..r.min(hu) {
        add(&mut count, &rows[y * wu..(y + 1) * wu], 1);
    }
    for y in 0..hu {
        if y + r < hu {
            add(&mut count, &rows[(y + r) * wu..(y + r + 1) * wu], 1);
        }
        for (o, &c) in out[y * wu..(y + 1) * wu].iter_mut().zip(&count) {
            *o = if c > 0 { 255 } else { 0 };
        }
        if y >= r {
            add(&mut count, &rows[(y - r) * wu..(y - r + 1) * wu], -1);
        }
    }
    GrayImage::from_raw(w, h, out).expect("w × h buffer")
}

/// `img` with `mask` as alpha, then, with a `lasso`, alpha cleared outside it plus `clip_margin`,
/// unless a keep point lies outside the lasso (the user marked something it misses) or the lasso
/// holds no pixel centre (a flat one would otherwise blank the image).
fn finish_cut(img: RgbaImage, mask: &GrayImage, points: &[SubjectPoint], lasso: Option<&[[u32; 2]]>) -> RgbaImage {
    let mut img = apply_mask(img, mask);
    let (w, h) = img.dimensions();
    // ponytail: a second full-size fill just to test emptiness; scan only the bbox rows if it shows up in profiles.
    if let Some(lasso) = lasso
        && points.iter().all(|p| !p.keep || point_in_lasso(lasso, p.x, p.y))
        && lasso_mask(lasso, w, h, 0).pixels().any(|p| p[0] != 0)
    {
        let clip = lasso_mask(lasso, w, h, clip_margin(w, h));
        for (p, c) in img.pixels_mut().zip(clip.pixels()) {
            if c[0] == 0 {
                p[3] = 0;
            }
        }
    }
    img
}

/// The decoder's `k × SAM_H × SAM_W` logits as a `w × h` alpha mask: the OR of the `k` masks (the
/// max logit), cropped to the source's area, sampled bilinearly, then `sigmoid · 255`.
/// Logits are interpolated before the sigmoid so edges stay sharp when scaled up.
fn sam_mask(logits: &[f32], scale: f32, w: u32, h: u32) -> GrayImage {
    let plane = (SAM_W * SAM_H) as usize;
    let mut best = vec![f32::MIN; plane];
    for m in logits.chunks_exact(plane) {
        for (b, &v) in best.iter_mut().zip(m) {
            *b = b.max(v);
        }
    }
    let (cw, ch) = sam_size(w, h, scale);
    let at = |x: u32, y: u32| best[(y * SAM_W + x) as usize];
    GrayImage::from_fn(w, h, |x, y| {
        // Pixel centres map to pixel centres.
        let sx = ((x as f32 + 0.5) * cw as f32 / w as f32 - 0.5).clamp(0.0, (cw - 1) as f32);
        let sy = ((y as f32 + 0.5) * ch as f32 / h as f32 - 0.5).clamp(0.0, (ch - 1) as f32);
        let (x0, y0) = (sx as u32, sy as u32);
        let (x1, y1) = ((x0 + 1).min(cw - 1), (y0 + 1).min(ch - 1));
        let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
        let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
        let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
        let v = top + (bottom - top) * fy;
        Luma([(255.0 / (1.0 + (-v).exp())).round() as u8])
    })
}

/// Runs SAM's encoder from `model` on `img`. The session is dropped before returning.
fn embed(lib: &Path, model: &Path, img: &DynamicImage) -> AppResult<SamEmbedding> {
    if img.width() == 0 || img.height() == 0 {
        return Err(AppError::InvalidInput("empty image".into()));
    }
    let (data, scale) = sam_input(img);
    let start = Instant::now();
    let mut session = load(env(lib)?, model)?;
    log::info!("ai: loaded subject encoder in {} ms", start.elapsed().as_millis());
    let input = Tensor::from_array(([SAM_H as usize, SAM_W as usize, 3], data)).map_err(ort_err)?;
    let start = Instant::now();
    let outputs = session.run(ort::inputs![input]).map_err(ort_err)?;
    let out = outputs.values().next().ok_or_else(|| AppError::Other("onnx runtime: no output".into()))?;
    let (shape, data) = out.try_extract_tensor::<f32>().map_err(ort_err)?;
    log::info!("ai: embedded {}x{} in {} ms", img.width(), img.height(), start.elapsed().as_millis());
    Ok(SamEmbedding {
        data: data.to_vec(),
        shape: shape.iter().map(|&d| d as usize).collect(),
        scale,
        w: img.width(),
        h: img.height(),
    })
}

/// Runs SAM's decoder from `model` for `points` and `lasso` (as a box) on `emb` and returns `img`
/// as RGBA with the mask as alpha, clipped to the lasso (`finish_cut`). The session is dropped
/// before returning.
fn decode(
    lib: &Path,
    model: &Path,
    img: &DynamicImage,
    emb: &SamEmbedding,
    points: &[SubjectPoint],
    lasso: Option<&[[u32; 2]]>,
) -> AppResult<DynamicImage> {
    check_subject(points, lasso)?;
    if (emb.w, emb.h) != (img.width(), img.height()) {
        return Err(AppError::InvalidInput("embedding is for another image".into()));
    }
    let (coords, labels) = sam_prompt(points, lasso.map(lasso_box), emb.scale);
    let n = labels.len();
    let mask = {
        let start = Instant::now();
        let mut session = load(env(lib)?, model)?;
        log::info!("ai: loaded subject decoder in {} ms", start.elapsed().as_millis());
        let inputs = ort::inputs! {
            "image_embeddings" => Tensor::from_array((emb.shape.clone(), emb.data.clone())).map_err(ort_err)?,
            "point_coords" => Tensor::from_array(([1usize, n, 2], coords)).map_err(ort_err)?,
            "point_labels" => Tensor::from_array(([1usize, n], labels)).map_err(ort_err)?,
            "mask_input" => Tensor::from_array(([1usize, 1, 256, 256], vec![0f32; 256 * 256])).map_err(ort_err)?,
            "has_mask_input" => Tensor::from_array(([1usize], vec![0f32])).map_err(ort_err)?,
            "orig_im_size" => Tensor::from_array(([2usize], vec![SAM_H as f32, SAM_W as f32])).map_err(ort_err)?,
        };
        let start = Instant::now();
        let outputs = session.run(inputs).map_err(ort_err)?;
        let out = outputs.values().next().ok_or_else(|| AppError::Other("onnx runtime: no output".into()))?;
        let (shape, logits) = out.try_extract_tensor::<f32>().map_err(ort_err)?;
        if shape.len() != 4 || shape[2] != SAM_H as i64 || shape[3] != SAM_W as i64 {
            return Err(AppError::Other(format!("onnx runtime: unexpected mask shape {shape:?}")));
        }
        let mask = sam_mask(logits, emb.scale, emb.w, emb.h);
        log::info!("ai: subject cut from {} points (lasso: {}) in {} ms", points.len(), lasso.is_some(), start.elapsed().as_millis());
        mask
    };
    Ok(DynamicImage::ImageRgba8(finish_cut(img.to_rgba8(), &mask, points, lasso)))
}

/// `InvalidInput` unless there are points or a lasso, and any lasso has at least 3 points.
pub(crate) fn check_subject(points: &[SubjectPoint], lasso: Option<&[[u32; 2]]>) -> AppResult<()> {
    match lasso {
        None if points.is_empty() => Err(AppError::InvalidInput("no subject points".into())),
        Some(l) if l.len() < 3 => Err(AppError::InvalidInput("lasso needs 3 points".into())),
        _ => Ok(()),
    }
}

/// Runs one worker request on `img`. `sam` holds the last subject embedding and its key.
pub(crate) fn handle(sam: &mut Option<(u64, SamEmbedding)>, req: Request, img: RgbaImage) -> AppResult<RgbaImage> {
    let lib = runtime_path()?;
    let img = DynamicImage::ImageRgba8(img);
    let out = match req {
        Request::RemoveBg { model, .. } => cut(&lib, &model, &img)?,
        Request::Subject { encoder, decoder, key, points, lasso, .. } => ai_worker::with_embedding(
            sam,
            key,
            || embed(&lib, &encoder, &img),
            |emb| decode(&lib, &decoder, &img, emb, &points, lasso.as_deref()),
        )?,
    };
    Ok(out.into_rgba8())
}

/// Every model file of `id`'s installed engine, or `Other("model missing")`.
fn installed(app: &AppHandle, id: EngineId) -> AppResult<Vec<PathBuf>> {
    model_paths(&engines::model_dir(app, id).ok_or_else(|| AppError::Other("model missing".into()))?, id)
}

/// `img` as RGBA with the background made transparent. `Other("model missing")` when the
/// engine isn't installed; `Other` too when the app lacks its runtime or the worker failed.
pub fn remove_background(app: &AppHandle, img: &DynamicImage) -> AppResult<DynamicImage> {
    let [model] = installed(app, EngineId::Bgremove)?.try_into().map_err(|_| AppError::Other("model missing".into()))?;
    let req = Request::RemoveBg { model, w: img.width(), h: img.height() };
    ai_worker::call(&req, img).map(DynamicImage::ImageRgba8)
}

/// `img` as RGBA with everything but the subject at `points` (and inside `lasso`) transparent.
/// `key` names `img`: the worker keeps one embedding and re-runs SAM's encoder only when `key`
/// changes. `InvalidInput` per `check_subject`; `Other("model missing")` when the engine isn't installed.
pub fn subject(app: &AppHandle, img: &DynamicImage, key: u64, points: &[SubjectPoint], lasso: Option<&[[u32; 2]]>) -> AppResult<DynamicImage> {
    check_subject(points, lasso)?;
    let [encoder, decoder] = installed(app, EngineId::Sam)?.try_into().map_err(|_| AppError::Other("model missing".into()))?;
    let req = Request::Subject { encoder, decoder, key, points: points.to_vec(), lasso: lasso.map(<[_]>::to_vec), w: img.width(), h: img.height() };
    ai_worker::call(&req, img).map(DynamicImage::ImageRgba8)
}

fn check_upscale(img: &DynamicImage, scale: u8) -> AppResult<()> {
    if !matches!(scale, 2 | 4) {
        return Err(AppError::InvalidInput(format!("bad upscale {scale}")));
    }
    if img.width() == 0 || img.height() == 0 {
        return Err(AppError::InvalidInput("empty image".into()));
    }
    Ok(())
}

/// `img` ×`scale` (2 or 4), capped by `upscale_size`. Lanczos only: the models rewrote Korean glyphs.
pub fn upscale(_app: &AppHandle, img: &DynamicImage, scale: u8) -> AppResult<DynamicImage> {
    check_upscale(img, scale)?;
    Ok(upscale_fallback(img, scale))
}

/// Call before removing `id`'s engine files: errors while a request is running, else stops the
/// worker so no model file stays open (Windows locks open files).
pub fn forget(id: EngineId) -> AppResult<()> {
    if !matches!(id, EngineId::Bgremove | EngineId::Sam) {
        return Ok(());
    }
    if ai_worker::in_flight() {
        return Err(AppError::InvalidInput(format!("{} is running", id.spec().dir_name)));
    }
    ai_worker::stop();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::{Rgb, RgbImage};

    /// This process's resident set in KB, from `ps` (unix only; the bench is run on macOS).
    fn rss_kb() -> String {
        let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn env_path(key: &str) -> PathBuf {
        PathBuf::from(std::env::var(key).unwrap_or_else(|_| panic!("set {key}")))
    }

    #[test]
    fn missing_model_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let err = model_paths(dir.path(), EngineId::Bgremove).unwrap_err();
        assert_eq!(err.to_string(), "model missing");
        std::fs::write(dir.path().join("model.onnx"), b"m").unwrap();
        assert_eq!(model_paths(dir.path(), EngineId::Bgremove).unwrap(), [dir.path().join("model.onnx")]);
        // Every file of a multi-file model must be there.
        std::fs::write(dir.path().join("encoder.onnx"), b"e").unwrap();
        assert_eq!(model_paths(dir.path(), EngineId::Sam).unwrap_err().to_string(), "model missing");
        std::fs::write(dir.path().join("decoder.onnx"), b"d").unwrap();
        assert_eq!(model_paths(dir.path(), EngineId::Sam).unwrap(), [dir.path().join("encoder.onnx"), dir.path().join("decoder.onnx")]);
    }

    #[test]
    fn runtime_is_looked_up_in_the_bundle() {
        let exe_dir = Path::new("/Applications/Vee.app/Contents/MacOS");
        let expected = if cfg!(target_os = "macos") { exe_dir.join("../Frameworks/libonnxruntime.dylib") } else { exe_dir.join(ORT_LIB) };
        assert_eq!(bundled_runtime(exe_dir), expected);
    }

    #[test]
    fn mask_alpha_is_resized_to_source() {
        let img = ::image::RgbaImage::from_pixel(8, 8, ::image::Rgba([10, 20, 30, 255]));
        let mask = GrayImage::from_fn(4, 4, |x, _| ::image::Luma([if x < 2 { 0 } else { 255 }]));
        let out = apply_mask(img, &mask);
        assert_eq!(out.dimensions(), (8, 8));
        // Lanczos rings a little at the step; colour is untouched.
        let (left, right) = (out.get_pixel(0, 4).0, out.get_pixel(7, 4).0);
        assert_eq!(&left[..3], &[10, 20, 30]);
        assert!(left[3] <= 16 && right[3] >= 240, "{left:?} {right:?}");
    }

    #[test]
    fn input_is_normalized_nchw() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_fn(2, 2, |x, y| if (x, y) == (1, 1) { Rgb([255, 255, 255]) } else { Rgb([0, 0, 0]) }));
        let data = to_input(&img, 2);
        assert_eq!(data.len(), 12);
        // black → (0 - mean) / std, white (v/max = 1) → (1 - mean) / std; red plane first.
        assert!((data[0] - (-MEAN[0] / STD[0])).abs() < 1e-5);
        assert!((data[3] - (1.0 - MEAN[0]) / STD[0]).abs() < 1e-5);
        assert!((data[4 + 3] - (1.0 - MEAN[1]) / STD[1]).abs() < 1e-5);
    }

    #[test]
    fn output_is_sigmoid_then_stretched() {
        let mask = from_output(&[-10.0, 0.0, 0.0, 10.0], 2);
        assert_eq!(mask.as_raw(), &[0, 127, 127, 255]);
    }

    #[test]
    fn upscale_rejects_other_scales() {
        let img = DynamicImage::new_rgba8(2, 2);
        assert!(matches!(check_upscale(&img, 3), Err(AppError::InvalidInput(_))));
        assert!(matches!(check_upscale(&DynamicImage::new_rgba8(0, 2), 2), Err(AppError::InvalidInput(_))));
        assert!(check_upscale(&img, 4).is_ok());
    }

    #[test]
    fn forget_refuses_while_running() {
        assert!(forget(EngineId::Ffmpeg).is_ok());
        let held = ai_worker::hold_for_test();
        assert!(matches!(forget(EngineId::Bgremove), Err(AppError::InvalidInput(_))));
        assert!(matches!(forget(EngineId::Sam), Err(AppError::InvalidInput(_))));
        drop(held);
        assert!(forget(EngineId::Bgremove).is_ok());
        assert!(forget(EngineId::Sam).is_ok());
    }

    fn point(x: u32, y: u32, keep: bool) -> SubjectPoint {
        SubjectPoint { x, y, keep }
    }

    #[test]
    fn sam_input_scales_and_pads() {
        let (data, scale) = sam_input(&DynamicImage::new_rgb8(2048, 1368));
        assert_eq!(scale, 0.5);
        assert_eq!(data.len(), 684 * 1024 * 3);
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(1024, 100, Rgb([200, 0, 0])));
        let (data, scale) = sam_input(&img);
        assert_eq!(scale, 1.0);
        // HWC: the image sits top-left, the rest is zero.
        assert_eq!(&data[..3], &[200.0, 0.0, 0.0]);
        let at = 200 * 1024 * 3;
        assert_eq!(&data[at..at + 3], &[0.0, 0.0, 0.0]);
    }

    #[test]
    fn sam_prompt_scales_and_pads() {
        let (coords, labels) = sam_prompt(&[point(100, 50, true), point(10, 20, false)], None, 0.5);
        assert_eq!(coords, [50.0, 25.0, 5.0, 10.0, 0.0, 0.0]);
        assert_eq!(labels, [1.0, 0.0, -1.0]);
    }

    #[test]
    fn sam_prompt_with_box() {
        let (coords, labels) = sam_prompt(&[point(100, 50, true)], Some([10, 20, 30, 40]), 0.5);
        assert_eq!(coords, [50.0, 25.0, 5.0, 10.0, 15.0, 20.0, 0.0, 0.0]);
        assert_eq!(labels, [1.0, 2.0, 3.0, -1.0]);
    }

    #[test]
    fn lasso_box_min_max() {
        assert_eq!(lasso_box(&[[5, 9], [2, 7], [8, 1]]), [2, 1, 8, 9]);
    }

    const SQUARE: [[u32; 2]; 4] = [[20, 20], [80, 20], [80, 80], [20, 80]];

    #[test]
    fn lasso_clip_square() {
        let m = lasso_mask(&SQUARE, 100, 100, 8);
        assert_eq!((m.get_pixel(50, 50)[0], m.get_pixel(85, 50)[0]), (255, 255));
        assert_eq!((m.get_pixel(5, 5)[0], m.get_pixel(95, 50)[0]), (0, 0));
        assert!(point_in_lasso(&SQUARE, 50, 50) && !point_in_lasso(&SQUARE, 85, 50));
    }

    /// A figure-8 crossing at (50, 50): even-odd keeps the left and right lobes, not the gaps above and below the crossing.
    #[test]
    fn lasso_fill_self_crossing() {
        let eight = [[10, 10], [90, 90], [90, 10], [10, 90]];
        let m = lasso_mask(&eight, 100, 100, 0);
        assert_eq!((m.get_pixel(20, 50)[0], m.get_pixel(80, 50)[0]), (255, 255));
        assert_eq!((m.get_pixel(50, 20)[0], m.get_pixel(50, 80)[0], m.get_pixel(5, 50)[0]), (0, 0, 0));
        assert_eq!(lasso_mask(&eight, 100, 100, 8).get_pixel(50, 20)[0], 0);
    }

    #[test]
    fn lasso_clip_clamped_polygon() {
        let edge = [[0, 10], [99, 10], [99, 90], [0, 90]];
        let m = lasso_mask(&edge, 100, 100, 0);
        assert_eq!((m.get_pixel(0, 50)[0], m.get_pixel(98, 50)[0], m.get_pixel(50, 5)[0]), (255, 255, 0));
        assert_eq!(lasso_mask(&edge, 100, 100, 8).get_pixel(99, 50)[0], 255);
        // Points past the image don't panic either.
        lasso_mask(&[[0, 0], [500, 0], [500, 500]], 100, 100, 8);
    }

    #[test]
    fn clip_margin_scales_with_long_side() {
        assert_eq!((clip_margin(100, 50), clip_margin(1000, 4000), clip_margin(6000, 10)), (8, 80, 120));
    }

    #[test]
    fn clip_skipped_for_outside_keep() {
        let img = RgbaImage::from_pixel(100, 100, ::image::Rgba([10, 20, 30, 255]));
        let mask = GrayImage::from_pixel(100, 100, Luma([200]));
        // Inside points: the clip runs, colour stays.
        let out = finish_cut(img.clone(), &mask, &[point(50, 50, true), point(95, 50, false)], Some(&SQUARE));
        assert_eq!((out.get_pixel(50, 50)[3], out.get_pixel(95, 50).0), (200, [10, 20, 30, 0]));
        // A keep point outside the lasso: no clip.
        let out = finish_cut(img.clone(), &mask, &[point(95, 50, true)], Some(&SQUARE));
        assert_eq!(out.get_pixel(95, 50)[3], 200);
        assert_eq!(finish_cut(img, &mask, &[point(95, 50, true)], None).get_pixel(95, 50)[3], 200);
    }

    #[test]
    fn clip_skipped_for_flat_lasso() {
        let img = RgbaImage::from_pixel(100, 100, ::image::Rgba([10, 20, 30, 255]));
        let mask = GrayImage::from_pixel(100, 100, Luma([200]));
        let out = finish_cut(img, &mask, &[], Some(&[[10, 50], [50, 50], [90, 50]]));
        assert!(out.pixels().all(|p| p[3] == 200));
    }

    /// A pixel centre exactly on a diagonal edge's crossing: `point_in_lasso` agrees with the fill.
    #[test]
    fn point_in_lasso_matches_fill_on_edge() {
        for tri in [[[0, 0], [100, 100], [0, 100]], [[0, 0], [100, 0], [100, 100]]] {
            let m = lasso_mask(&tri, 100, 100, 0);
            for (x, y) in [(50, 50), (20, 20), (49, 50), (51, 50)] {
                assert_eq!(point_in_lasso(&tri, x, y), m.get_pixel(x, y)[0] == 255, "{tri:?} ({x}, {y})");
            }
        }
    }

    #[test]
    fn lasso_clip_large_is_fast() {
        let circle: Vec<[u32; 2]> = (0..400)
            .map(|i| {
                let a = i as f32 / 400.0 * std::f32::consts::TAU;
                [(3000.0 + 1800.0 * a.cos()) as u32, (2000.0 + 1800.0 * a.sin()) as u32]
            })
            .collect();
        let start = Instant::now();
        let m = lasso_mask(&circle, 6000, 4000, clip_margin(6000, 4000));
        let ms = start.elapsed().as_millis();
        println!("lasso_mask 6000x4000, 400 points: {ms} ms");
        assert!(ms < 1000, "{ms} ms");
        assert_eq!((m.get_pixel(3000, 2000)[0], m.get_pixel(10, 10)[0]), (255, 0));
    }

    #[test]
    fn sam_mask_left_half() {
        let plane: Vec<f32> = (0..SAM_H * SAM_W).map(|i| if i % SAM_W < 512 { 10.0 } else { -10.0 }).collect();
        // Two masks: the second is all background, so the max keeps the first's left half.
        let logits: Vec<f32> = plane.iter().copied().chain(std::iter::repeat_n(-20.0, plane.len())).collect();
        let mask = sam_mask(&logits, 1.0, 1024, 684);
        assert_eq!(mask.dimensions(), (1024, 684));
        assert_eq!(mask.get_pixel(10, 10)[0], 255);
        assert_eq!(mask.get_pixel(1000, 10)[0], 0);
    }

    /// Runs SAM on `VEE_AI_IMAGE` with one centre point, using the vendored runtime (`node
    /// scripts/fetch-onnxruntime.mjs`) and the quantized ViT-B pair in `VEE_SAM_ENCODER` /
    /// `VEE_SAM_DECODER` (rembg's `sam_vit_b_01ec64.{encoder,decoder}.quant.onnx`), writing RGBA and
    /// on-magenta PNGs to `VEE_AI_OUT`:
    /// `cargo test --lib sam_real_model -- --ignored --nocapture`
    /// Prints the models' inputs/outputs, and RSS before, after embed and after each run (twice,
    /// to show sessions are dropped).
    #[test]
    #[ignore]
    fn sam_real_model() {
        let (lib, encoder, decoder) = (vendor_runtime(), env_path("VEE_SAM_ENCODER"), env_path("VEE_SAM_DECODER"));
        let img = ::image::open(env_path("VEE_AI_IMAGE")).unwrap();
        let out = env_path("VEE_AI_OUT");
        std::fs::create_dir_all(&out).unwrap();
        {
            for model in [&encoder, &decoder] {
                let session = load(env(&lib).unwrap(), model).unwrap();
                for o in session.inputs() {
                    println!("in  {}: {}", o.name(), o.dtype());
                }
                for o in session.outputs() {
                    println!("out {}: {}", o.name(), o.dtype());
                }
            }
        }
        println!("rss before {} KB", rss_kb());
        let centre = [SubjectPoint { x: img.width() / 2, y: img.height() / 2, keep: true }];
        for run in 0..2 {
            let start = Instant::now();
            let emb = embed(&lib, &encoder, &img).unwrap();
            println!("embed {:?} in {} ms, rss {} KB", emb.shape, start.elapsed().as_millis(), rss_kb());
            let start = Instant::now();
            let cut = decode(&lib, &decoder, &img, &emb, &centre, None).unwrap();
            println!("cut in {} ms, rss {} KB", start.elapsed().as_millis(), rss_kb());
            assert_eq!(cut.color(), ::image::ColorType::Rgba8);
            assert_eq!((cut.width(), cut.height()), (img.width(), img.height()));
            // The clicked subject is kept.
            assert!(cut.to_rgba8().get_pixel(centre[0].x, centre[0].y)[3] > 200);
            drop(emb);
            println!("run {run} done, rss {} KB", rss_kb());
            cut.save(out.join("sam.png")).unwrap();
            let mut bg = RgbaImage::from_pixel(img.width(), img.height(), ::image::Rgba([255, 0, 160, 255]));
            imageops::overlay(&mut bg, &cut.to_rgba8(), 0, 0);
            DynamicImage::ImageRgba8(bg).save(out.join("sam.on-magenta.png")).unwrap();
        }
        // Lasso only: a circle around the middle, then the left half as a box (clipped there).
        let (w, h) = (img.width() as f32, img.height() as f32);
        let circle: Vec<[u32; 2]> = (0..32)
            .map(|i| {
                let a = i as f32 / 32.0 * std::f32::consts::TAU;
                [(w / 2.0 + 0.42 * w * a.cos()) as u32, (h / 2.0 + 0.42 * h * a.sin()) as u32]
            })
            .collect();
        let left = vec![[(w * 0.1) as u32, (h * 0.1) as u32], [(w * 0.5) as u32, (h * 0.1) as u32], [(w * 0.5) as u32, (h * 0.9) as u32], [(w * 0.1) as u32, (h * 0.9) as u32]];
        let emb = embed(&lib, &encoder, &img).unwrap();
        for (name, lasso) in [("circle", circle), ("left", left)] {
            let cut = decode(&lib, &decoder, &img, &emb, &[], Some(&lasso)).unwrap().to_rgba8();
            let mut bg = RgbaImage::from_pixel(img.width(), img.height(), ::image::Rgba([255, 0, 160, 255]));
            imageops::overlay(&mut bg, &cut, 0, 0);
            DynamicImage::ImageRgba8(bg).save(out.join(format!("sam.lasso-{name}.on-magenta.png"))).unwrap();
        }
    }

    /// Runs the real path on `product-*.png` in `VEE_AI_BENCH_DIR`, with the vendored runtime
    /// (`node scripts/fetch-onnxruntime.mjs`) and `VEE_AI_MODEL` (BiRefNet `.onnx`), writing RGBA and
    /// on-magenta PNGs to `VEE_AI_OUT`.
    /// Prints time and RSS after each image (the session is dropped at the end of each run).
    #[test]
    #[ignore]
    fn bench_models() {
        let dir = env_path("VEE_AI_BENCH_DIR");
        let out = env_path("VEE_AI_OUT");
        let (lib, model) = (vendor_runtime(), env_path("VEE_AI_MODEL"));
        std::fs::create_dir_all(&out).unwrap();
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        for path in files.iter().filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("product-")) {
            let img = ::image::open(path).unwrap();
            let start = Instant::now();
            let cut = cut(&lib, &model, &img).unwrap();
            let ms = start.elapsed().as_millis();
            assert_eq!(cut.color(), ::image::ColorType::Rgba8);
            assert_eq!((cut.width(), cut.height()), (img.width(), img.height()));
            let stem = path.file_stem().unwrap().to_string_lossy();
            cut.save(out.join(format!("{stem}.runtime.png"))).unwrap();
            // Over magenta so halos and missing edges show when reviewing.
            let mut bg = RgbaImage::from_pixel(img.width(), img.height(), ::image::Rgba([255, 0, 160, 255]));
            imageops::overlay(&mut bg, &cut.to_rgba8(), 0, 0);
            DynamicImage::ImageRgba8(bg).save(out.join(format!("{stem}.runtime.on-magenta.png"))).unwrap();
            println!("{stem}\t{}x{}\t{ms} ms\trss after {} KB", img.width(), img.height(), rss_kb());
        }
        println!("rss at end {} KB", rss_kb());
    }
}
