//! On-device image models (background removal, upscaling) through ONNX Runtime.
//! For now only a benchmark that compares the candidate models.

#[cfg(test)]
mod tests {
    use crate::domain::edit::upscale_fallback;
    use ::image::{DynamicImage, GrayImage, ImageBuffer, Rgb, RgbImage, imageops, imageops::FilterType};
    use ort::{session::Session, value::Tensor};
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    const BG_SIDE: u32 = 1024;
    const TILE: u32 = 128;
    const OVERLAP: u32 = 16;

    fn env_path(key: &str) -> PathBuf {
        PathBuf::from(std::env::var(key).unwrap_or_else(|_| panic!("set {key}")))
    }

    /// NCHW f32 from RGB, `(v / max - mean) / std` per channel (rembg's `normalize`).
    fn bg_input(img: &DynamicImage, mean: [f32; 3], std: [f32; 3]) -> Vec<f32> {
        let rgb = img.resize_exact(BG_SIDE, BG_SIDE, FilterType::Lanczos3).to_rgb8();
        let max = rgb.as_raw().iter().copied().max().unwrap_or(1).max(1) as f32;
        let plane = (BG_SIDE * BG_SIDE) as usize;
        let mut data = vec![0f32; 3 * plane];
        for (i, p) in rgb.pixels().enumerate() {
            for c in 0..3 {
                data[c * plane + i] = (p[c] as f32 / max - mean[c]) / std[c];
            }
        }
        data
    }

    /// Runs a bg model and returns the input with the mask as alpha. `sigmoid` for BiRefNet.
    fn remove_bg(session: &mut Session, img: &DynamicImage, mean: [f32; 3], std: [f32; 3], sigmoid: bool) -> DynamicImage {
        let side = BG_SIDE as usize;
        let input = Tensor::from_array(([1usize, 3, side, side], bg_input(img, mean, std))).unwrap();
        let outputs = session.run(ort::inputs![input]).unwrap();
        let (_, pred) = outputs[0].try_extract_tensor::<f32>().unwrap();
        let pred: Vec<f32> = pred[..side * side].iter().map(|&v| if sigmoid { 1.0 / (1.0 + (-v).exp()) } else { v }).collect();
        let (lo, hi) = pred.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        let span = (hi - lo).max(1e-6);
        let mask = GrayImage::from_fn(BG_SIDE, BG_SIDE, |x, y| {
            ::image::Luma([(((pred[(y * BG_SIDE + x) as usize] - lo) / span) * 255.0) as u8])
        });
        let mask = imageops::resize(&mask, img.width(), img.height(), FilterType::Lanczos3);
        let mut rgba = img.to_rgba8();
        for (p, m) in rgba.pixels_mut().zip(mask.pixels()) {
            p[3] = m[0];
        }
        DynamicImage::ImageRgba8(rgba)
    }

    /// One ×4 pass of a Real-ESRGAN model on an RGB tile.
    fn esrgan_tile(session: &mut Session, tile: &RgbImage) -> RgbImage {
        let (w, h) = tile.dimensions();
        let plane = (w * h) as usize;
        let mut data = vec![0f32; 3 * plane];
        for (i, p) in tile.pixels().enumerate() {
            for c in 0..3 {
                data[c * plane + i] = p[c] as f32 / 255.0;
            }
        }
        let input = Tensor::from_array(([1usize, 3, h as usize, w as usize], data)).unwrap();
        let outputs = session.run(ort::inputs![input]).unwrap();
        let (_, out) = outputs[0].try_extract_tensor::<f32>().unwrap();
        let (ow, oh) = (w * 4, h * 4);
        let oplane = (ow * oh) as usize;
        ImageBuffer::from_fn(ow, oh, |x, y| {
            let i = (y * ow + x) as usize;
            let px = |c: usize| (out[c * oplane + i].clamp(0.0, 1.0) * 255.0).round() as u8;
            Rgb([px(0), px(1), px(2)])
        })
    }

    /// ×4 with 128-px tiles: each tile covers a 96-px core plus up to 16 px of context per side,
    /// and only the core is kept, so seams never show.
    fn esrgan_x4(session: &mut Session, img: &DynamicImage) -> RgbImage {
        let src = img.to_rgb8();
        let (w, h) = src.dimensions();
        let core = TILE - 2 * OVERLAP;
        let mut out = RgbImage::new(w * 4, h * 4);
        for cy in (0..h).step_by(core as usize) {
            for cx in (0..w).step_by(core as usize) {
                let (cw, ch) = (core.min(w - cx), core.min(h - cy));
                let (x0, y0) = (cx.saturating_sub(OVERLAP), cy.saturating_sub(OVERLAP));
                let (x1, y1) = ((cx + cw + OVERLAP).min(w), (cy + ch + OVERLAP).min(h));
                let tile = imageops::crop_imm(&src, x0, y0, x1 - x0, y1 - y0).to_image();
                let up = esrgan_tile(session, &tile);
                let keep = imageops::crop_imm(&up, (cx - x0) * 4, (cy - y0) * 4, cw * 4, ch * 4).to_image();
                imageops::replace(&mut out, &keep, cx as i64 * 4, cy as i64 * 4);
            }
        }
        out
    }

    fn save(out: &Path, name: &str, img: &DynamicImage) -> PathBuf {
        let path = out.join(name);
        img.save(&path).unwrap();
        path
    }

    /// Stacks images top to bottom (all as RGB), for side-by-side review.
    fn stack(imgs: &[&DynamicImage]) -> DynamicImage {
        let w = imgs.iter().map(|i| i.width()).max().unwrap();
        let h = imgs.iter().map(|i| i.height() + 8).sum();
        let mut canvas = RgbImage::from_pixel(w, h, Rgb([255, 64, 160]));
        let mut y = 0;
        for img in imgs {
            imageops::replace(&mut canvas, &img.to_rgb8(), 0, y);
            y += img.height() as i64 + 8;
        }
        DynamicImage::ImageRgb8(canvas)
    }

    /// Compares the candidate models. `VEE_AI_BENCH_DIR` holds `product-*.png` (background removal)
    /// and `text-*.png` (sharp UI text; downscaled ×1/2 and ×1/3 with Lanczos to make the small
    /// blurry input). `VEE_AI_ORT` is the ONNX Runtime dylib, `VEE_AI_MODELS` the folder with
    /// the four `.onnx` files, `VEE_AI_OUT` where outputs go.
    #[test]
    #[ignore]
    fn bench_models() {
        let dir = env_path("VEE_AI_BENCH_DIR");
        let models = env_path("VEE_AI_MODELS");
        let out = env_path("VEE_AI_OUT");
        std::fs::create_dir_all(&out).unwrap();
        let env = ort::init_from(env_path("VEE_AI_ORT")).expect("load ORT").build().unwrap();
        let open = |file: &str| {
            let start = Instant::now();
            let s = Session::builder(&env).unwrap().commit_from_file(models.join(file)).unwrap();
            println!("load {file}: {} ms", start.elapsed().as_millis());
            s
        };

        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        let named = |prefix: &str| -> Vec<PathBuf> {
            files.iter().filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix)).cloned().collect()
        };

        // name, file, mean, std, sigmoid (per rembg's sessions)
        let bg = [
            ("isnet", "isnet-general-use.onnx", [0.5; 3], [1.0; 3], false),
            ("birefnet", "BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx", [0.485, 0.456, 0.406], [0.229, 0.224, 0.225], true),
        ];
        for (name, file, mean, std, sigmoid) in bg {
            let mut session = open(file);
            for path in named("product-") {
                let img = ::image::open(&path).unwrap();
                let start = Instant::now();
                let cut = remove_bg(&mut session, &img, mean, std, sigmoid);
                let ms = start.elapsed().as_millis();
                let stem = path.file_stem().unwrap().to_string_lossy();
                let saved = save(&out, &format!("{stem}.{name}.png"), &cut);
                // Over magenta so halos and missing edges show when reviewing.
                let mut bg = ::image::RgbaImage::from_pixel(img.width(), img.height(), ::image::Rgba([255, 0, 160, 255]));
                imageops::overlay(&mut bg, &cut.to_rgba8(), 0, 0);
                save(&out, &format!("{stem}.{name}.on-magenta.png"), &DynamicImage::ImageRgba8(bg));
                println!("{name}\t{}x{}\t{ms} ms\t{}", img.width(), img.height(), saved.display());
            }
        }

        let up = [("general-x4v3", "realesr-general-x4v3.onnx"), ("x4plus", "realesrgan-x4plus.onnx")];
        let mut sessions: Vec<_> = up.iter().map(|(name, file)| (*name, open(file))).collect();
        for path in named("text-") {
            let truth = ::image::open(&path).unwrap();
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            for down in [2u32, 3] {
                let small = truth.resize_exact(truth.width() / down, truth.height() / down, FilterType::Lanczos3);
                let tag = format!("{stem}.d{down}");
                save(&out, &format!("{tag}.input.png"), &small);
                let mut at2 = vec![("lanczos", upscale_fallback(&small, 2))];
                save(&out, &format!("{tag}.lanczos.x2.png"), &at2[0].1);
                save(&out, &format!("{tag}.lanczos.x4.png"), &upscale_fallback(&small, 4));
                for (name, session) in sessions.iter_mut() {
                    let start = Instant::now();
                    let x4 = DynamicImage::ImageRgb8(esrgan_x4(session, &small));
                    let ms = start.elapsed().as_millis();
                    let x2 = x4.resize_exact(small.width() * 2, small.height() * 2, FilterType::Lanczos3);
                    save(&out, &format!("{tag}.{name}.x4.png"), &x4);
                    save(&out, &format!("{tag}.{name}.x2.png"), &x2);
                    println!("{name}\t{tag} {}x{}\t{ms} ms", small.width(), small.height());
                    at2.push((name, x2));
                }
                // truth (resized to the ×2 size), then lanczos, then each model, all at ×2.
                let truth2 = truth.resize_exact(small.width() * 2, small.height() * 2, FilterType::Lanczos3);
                let mut rows = vec![&truth2];
                rows.extend(at2.iter().map(|(_, img)| img));
                let saved = save(&out, &format!("{tag}.compare-x2.png"), &stack(&rows));
                println!("compare (truth, {}): {}", at2.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "), saved.display());
            }
        }
    }
}
