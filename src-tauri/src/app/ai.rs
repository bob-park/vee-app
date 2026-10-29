//! On-device image models through ONNX Runtime: background removal with BiRefNet, and the
//! upscale entry point (Lanczos only; no model passed the glyph check).

use crate::app::engines::{self, EngineId};
use crate::domain::edit::upscale_fallback;
use crate::shared::error::{AppError, AppResult};
use ::image::{DynamicImage, GrayImage, Luma, RgbaImage, imageops, imageops::FilterType};
use ort::{
    environment::Environment,
    ep,
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError, TryLockError};
use std::time::Instant;
use tauri::AppHandle;

/// BiRefNet's fixed input side.
const SIDE: u32 = 1024;
/// ImageNet mean/std, as rembg feeds BiRefNet.
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// ORT allows one environment per process, so it lives until exit once built (under `RUN`'s lock).
static ENV: OnceLock<Environment> = OnceLock::new();
/// Held for a whole run. The session is not kept between runs: BiRefNet holds GBs while loaded,
/// and reloading costs about a second.
// ponytail: one global lock, so runs are serial; fine for one editor window.
static RUN: Mutex<()> = Mutex::new(());

fn ort_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("onnx runtime: {e}"))
}

/// `(runtime lib, model)` of an installed model engine in `dir`.
fn session_paths(dir: &Path, id: EngineId) -> AppResult<(PathBuf, PathBuf)> {
    let [lib, model] = id.spec().binaries else { return Err(AppError::Other("model missing".into())) };
    let (lib, model) = (dir.join(lib), dir.join(model));
    if !lib.is_file() || !model.is_file() {
        return Err(AppError::Other("model missing".into()));
    }
    Ok((lib, model))
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
        // The guard holds no data, so a panic in an earlier run leaves nothing to clean up.
        let _run = RUN.lock().unwrap_or_else(PoisonError::into_inner);
        let env = match ENV.get() {
            Some(env) => env,
            None => {
                let env = ort::init_from(lib).map_err(ort_err)?.build().map_err(ort_err)?;
                ENV.get_or_init(|| env)
            }
        };
        let start = Instant::now();
        let mut session = load(env, model)?;
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

/// `img` as RGBA with the background made transparent. `Other("model missing")` when the
/// engine isn't installed.
pub fn remove_background(app: &AppHandle, img: &DynamicImage) -> AppResult<DynamicImage> {
    let dir = engines::model_dir(app, EngineId::Bgremove).ok_or_else(|| AppError::Other("model missing".into()))?;
    let (lib, model) = session_paths(&dir, EngineId::Bgremove)?;
    cut(&lib, &model, img)
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

/// Call before removing `id`'s engine files: errors while a run has the model open. No session
/// outlives a run, so there is nothing else to drop; the ORT library stays loaded.
pub fn forget(id: EngineId) -> AppResult<()> {
    if id != EngineId::Bgremove {
        return Ok(());
    }
    match RUN.try_lock() {
        Err(TryLockError::WouldBlock) => Err(AppError::InvalidInput("bgremove is running".into())),
        Ok(_) | Err(TryLockError::Poisoned(_)) => Ok(()),
    }
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
        let err = session_paths(dir.path(), EngineId::Bgremove).unwrap_err();
        assert_eq!(err.to_string(), "model missing");
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
        let held = RUN.lock().unwrap_or_else(PoisonError::into_inner);
        assert!(matches!(forget(EngineId::Bgremove), Err(AppError::InvalidInput(_))));
        drop(held);
        assert!(forget(EngineId::Bgremove).is_ok());
    }

    /// Runs the real path on `product-*.png` in `VEE_AI_BENCH_DIR`, with `VEE_AI_ORT` (the ORT
    /// dylib) and `VEE_AI_MODEL` (BiRefNet `.onnx`), writing RGBA and on-magenta PNGs to `VEE_AI_OUT`.
    /// Prints time and RSS after each image (the session is dropped at the end of each run).
    #[test]
    #[ignore]
    fn bench_models() {
        let dir = env_path("VEE_AI_BENCH_DIR");
        let out = env_path("VEE_AI_OUT");
        let (lib, model) = (env_path("VEE_AI_ORT"), env_path("VEE_AI_MODEL"));
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
