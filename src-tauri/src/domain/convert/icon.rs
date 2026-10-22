//! ICO and ICNS writers: a square, transparent-padded copy of the image at each icon size, stored as PNGs.

use super::{Format, JobError};
use ::image::codecs::png::PngEncoder;
use ::image::imageops::{self, FilterType};
use ::image::{DynamicImage, ImageEncoder, RgbaImage};
use std::path::Path;

/// ICO image sizes. 256 is the largest an ICO entry can name.
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
/// ICNS PNG blocks: type and pixel size (`ic11`–`ic14` are the @2x variants of 16–256).
const ICNS_BLOCKS: [(&[u8; 4], u32); 10] = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic09", 512),
    (b"ic10", 1024),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic13", 256),
    (b"ic14", 512),
];

/// Writes `img` to `out` as an ICO or ICNS (`to`).
pub fn write_icon(img: DynamicImage, to: Format, out: &Path) -> Result<(), JobError> {
    let src = img.into_rgba8();
    let bytes = match to {
        Format::Ico => ico_bytes(&src)?,
        Format::Icns => icns_bytes(&src)?,
        _ => return Err(JobError::Engine),
    };
    std::fs::write(out, bytes).map_err(|e| super::image::engine_error("write", e))
}

/// The largest icon size; bigger sources are scaled down to it first.
const MAX_SIDE: u32 = 1024;

/// `src` centred on a transparent square as wide as its longer side, at most `MAX_SIDE`.
fn square(src: &RgbaImage) -> RgbaImage {
    let (w, h) = src.dimensions();
    if w.max(h) > MAX_SIDE {
        // A panorama would otherwise pad to a gigabyte-sized canvas before any resize.
        let scale = f64::from(MAX_SIDE) / f64::from(w.max(h));
        let fit = |v: u32| ((f64::from(v) * scale).round() as u32).max(1);
        return square(&imageops::resize(src, fit(w), fit(h), FilterType::Lanczos3));
    }
    let side = w.max(h);
    let mut canvas = RgbaImage::new(side, side);
    imageops::overlay(&mut canvas, src, i64::from((side - w) / 2), i64::from((side - h) / 2));
    canvas
}

/// The square image scaled to `size`, PNG-encoded. Small sources are scaled up.
fn png_at(sq: &RgbaImage, size: u32) -> Result<Vec<u8>, JobError> {
    let scaled = imageops::resize(sq, size, size, FilterType::Lanczos3);
    let mut buf = Vec::new();
    PngEncoder::new(&mut buf)
        .write_image(&scaled, size, size, ::image::ExtendedColorType::Rgba8)
        .map_err(|e| super::image::engine_error("encode", e))?;
    Ok(buf)
}

/// ICONDIR, one 16-byte ICONDIRENTRY per size, then the PNGs (little-endian throughout).
fn ico_bytes(src: &RgbaImage) -> Result<Vec<u8>, JobError> {
    let sq = square(src);
    let pngs = ICO_SIZES.iter().map(|&s| png_at(&sq, s)).collect::<Result<Vec<_>, _>>()?;
    let mut out = vec![0, 0, 1, 0];
    out.extend((ICO_SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * ICO_SIZES.len();
    for (&size, png) in ICO_SIZES.iter().zip(&pngs) {
        // 256 doesn't fit a byte; 0 means 256.
        let side = if size >= 256 { 0 } else { size as u8 };
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes()); // planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((png.len() as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += png.len();
    }
    pngs.iter().for_each(|png| out.extend(png));
    Ok(out)
}

/// `icns` + total length, then one block per entry: type, (8 + data) length, PNG (big-endian lengths).
fn icns_bytes(src: &RgbaImage) -> Result<Vec<u8>, JobError> {
    let sq = square(src);
    let mut by_size: Vec<(u32, Vec<u8>)> = Vec::new();
    let mut body = Vec::new();
    for (kind, size) in ICNS_BLOCKS {
        // Several blocks share a pixel size; encode each size once.
        let png = match by_size.iter().find(|(s, _)| *s == size) {
            Some((_, png)) => png.clone(),
            None => {
                let png = png_at(&sq, size)?;
                by_size.push((size, png.clone()));
                png
            }
        };
        body.extend(kind);
        body.extend((8 + png.len() as u32).to_be_bytes());
        body.extend(png);
    }
    let mut out = b"icns".to_vec();
    out.extend((8 + body.len() as u32).to_be_bytes());
    out.extend(body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::{Rgba, RgbaImage};

    fn img(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 255]))
    }

    #[test]
    fn ico_holds_every_size_with_256_written_as_zero() {
        let b = ico_bytes(&img(300, 120)).unwrap(); // non-square
        assert_eq!(&b[..6], &[0, 0, 1, 0, 7, 0]); // reserved, type 1, 7 images
        let widths: Vec<u8> = (0..7).map(|i| b[6 + i * 16]).collect();
        assert_eq!(widths, [16, 24, 32, 48, 64, 128, 0]);
        let first = u32::from_le_bytes(b[18..22].try_into().unwrap()) as usize; // entry 0 offset
        assert_eq!(&b[first..first + 8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn icns_lists_png_blocks_and_its_own_length() {
        let b = icns_bytes(&img(1, 1)).unwrap(); // tiny
        assert_eq!(&b[..4], b"icns");
        assert_eq!(u32::from_be_bytes(b[4..8].try_into().unwrap()) as usize, b.len());
        let mut types = vec![];
        let mut at = 8;
        while at < b.len() {
            types.push(std::str::from_utf8(&b[at..at + 4]).unwrap().to_owned());
            at += u32::from_be_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        }
        assert_eq!(types, ["icp4", "icp5", "ic07", "ic08", "ic09", "ic10", "ic11", "ic12", "ic13", "ic14"]);
    }

    #[test]
    fn huge_sources_are_scaled_down_before_padding() {
        let sq = square(&img(4096, 1024));
        assert_eq!(sq.dimensions(), (1024, 1024));
        assert_eq!(sq.get_pixel(0, 0)[3], 0);
        assert_eq!(sq.get_pixel(0, 512)[3], 255);
    }

    #[test]
    fn padding_is_transparent_and_centered() {
        let sq = square(&img(4, 2));
        assert_eq!(sq.dimensions(), (4, 4));
        assert_eq!(sq.get_pixel(0, 0)[3], 0);
        assert_eq!(sq.get_pixel(0, 1)[3], 255);
    }
}
