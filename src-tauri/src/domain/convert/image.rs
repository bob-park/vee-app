//! Image conversion engines: `sips` on macOS, the `image` crate on Windows.

use super::{Format, JobError};
use std::path::Path;

/// Converts `src` to `to`, writing `out`. Any intermediate file is removed before returning.
#[cfg(target_os = "macos")]
pub fn convert_image(src: &Path, to: Format, out: &Path) -> Result<(), JobError> {
    match to {
        Format::Webp => {
            let png = out.with_extension("vee-tmp.png");
            let result = sips(src, "png", &[], &png).and_then(|()| {
                let img = ::image::open(&png).map_err(|e| engine_error("decode", e))?;
                write_webp(img, out)
            });
            let _ = std::fs::remove_file(&png);
            result
        }
        Format::Pdf => {
            let jpg = out.with_extension("vee-tmp.jpg");
            let result = sips(src, "jpeg", &["-s", "formatOptions", "90"], &jpg).and_then(|()| {
                let jpeg = std::fs::read(&jpg).map_err(|e| engine_error("read", e))?;
                write_pdf(&jpeg, out)
            });
            let _ = std::fs::remove_file(&jpg);
            result
        }
        Format::Png => sips(src, "png", &[], out),
        Format::Jpg => sips(src, "jpeg", &[], out),
        Format::Heic => sips(src, "heic", &[], out),
        Format::Tiff => sips(src, "tiff", &[], out),
        Format::Gif => sips(src, "gif", &[], out),
        Format::Bmp => sips(src, "bmp", &[], out),
    }
}

#[cfg(target_os = "macos")]
fn sips(src: &Path, format: &str, extra: &[&str], out: &Path) -> Result<(), JobError> {
    let output = std::process::Command::new("sips")
        .args(["-s", "format", format])
        .args(extra)
        .arg(src)
        .arg("--out")
        .arg(out)
        .output()
        .map_err(|e| engine_error("sips", e))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(engine_error("sips", String::from_utf8_lossy(&output.stderr)))
    }
}

/// Converts `src` to `to`, writing `out`. HEIC never reaches here (`formats_for(.., heic = false)`).
#[cfg(windows)]
pub fn convert_image(src: &Path, to: Format, out: &Path) -> Result<(), JobError> {
    use ::image::ImageFormat;
    let img = ::image::open(src).map_err(|e| engine_error("decode", e))?;
    let format = match to {
        Format::Webp => return write_webp(img, out),
        Format::Jpg => return std::fs::write(out, encode_jpeg(&img)?).map_err(|e| engine_error("write", e)),
        Format::Pdf => return write_pdf(&encode_jpeg(&img)?, out),
        Format::Png => ImageFormat::Png,
        Format::Tiff => ImageFormat::Tiff,
        Format::Gif => ImageFormat::Gif,
        Format::Bmp => ImageFormat::Bmp,
        Format::Heic => return Err(JobError::Engine),
    };
    // 8-bit RGBA is the one layout every encoder here accepts.
    ::image::DynamicImage::from(img.into_rgba8()).save_with_format(out, format).map_err(|e| engine_error("encode", e))
}

/// JPEG has no alpha, so the image is flattened to RGB first. Quality 90.
#[cfg(windows)]
fn encode_jpeg(img: &::image::DynamicImage) -> Result<Vec<u8>, JobError> {
    let mut buf = Vec::new();
    ::image::DynamicImage::from(img.to_rgb8())
        .write_with_encoder(::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90))
        .map_err(|e| engine_error("encode", e))?;
    Ok(buf)
}

// ponytail: lossless-only WebP (the `image` crate has no lossy encoder); revisit if files are too large.
fn write_webp(img: ::image::DynamicImage, out: &Path) -> Result<(), JobError> {
    let file = std::fs::File::create(out).map_err(|e| engine_error("create", e))?;
    ::image::DynamicImage::from(img.into_rgba8())
        .write_with_encoder(::image::codecs::webp::WebPEncoder::new_lossless(std::io::BufWriter::new(file)))
        .map_err(|e| engine_error("encode", e))
}

fn write_pdf(jpeg: &[u8], out: &Path) -> Result<(), JobError> {
    let pdf = jpeg_to_pdf(jpeg).ok_or_else(|| engine_error("pdf", "no baseline/progressive SOF marker"))?;
    std::fs::write(out, pdf).map_err(|e| engine_error("write", e))
}

fn engine_error(step: &str, e: impl std::fmt::Display) -> JobError {
    log::warn!("convert: {step} failed: {e}");
    JobError::Engine
}

/// A one-page PDF showing `jpeg` at its pixel size in points, embedded as-is (`DCTDecode`).
/// `None` when no SOF0/SOF1/SOF2 header is found or the colour layout is not gray/RGB.
pub fn jpeg_to_pdf(jpeg: &[u8]) -> Option<Vec<u8>> {
    let (w, h, components) = jpeg_size(jpeg)?;
    let color = match components {
        1 => "/DeviceGray",
        3 => "/DeviceRGB",
        _ => return None,
    };
    let content = format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q");
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    let mut object = |pdf: &mut Vec<u8>, body: &[u8]| {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        pdf.extend_from_slice(body);
        pdf.extend_from_slice(b"\nendobj\n");
    };
    object(&mut pdf, b"<< /Type /Catalog /Pages 2 0 R >>");
    object(&mut pdf, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    object(
        &mut pdf,
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>")
            .as_bytes(),
    );
    let image_head = format!(
        "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace {color} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
        jpeg.len()
    );
    object(&mut pdf, &[image_head.as_bytes(), jpeg, b"\nendstream"].concat());
    object(&mut pdf, format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).as_bytes());
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for offset in &offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes(),
    );
    Some(pdf)
}

/// (width, height, components) from the first SOF0/SOF1/SOF2 segment, walking markers from SOI to SOS.
fn jpeg_size(jpeg: &[u8]) -> Option<(u16, u16, u8)> {
    if !jpeg.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    loop {
        // Skip fill bytes before the marker code.
        while *jpeg.get(i)? == 0xFF && *jpeg.get(i + 1)? == 0xFF {
            i += 1;
        }
        if *jpeg.get(i)? != 0xFF {
            return None;
        }
        let marker = *jpeg.get(i + 1)?;
        let len = u16::from_be_bytes([*jpeg.get(i + 2)?, *jpeg.get(i + 3)?]) as usize;
        match marker {
            0xC0..=0xC2 => {
                let s = jpeg.get(i + 4..i + 10)?;
                return Some((u16::from_be_bytes([s[3], s[4]]), u16::from_be_bytes([s[1], s[2]]), s[5]));
            }
            0xDA | 0xD9 => return None,
            _ => i += 2 + len,
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};

    /// 1×1 RGBA PNG.
    pub fn tiny_png() -> Vec<u8> {
        STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
            .unwrap()
    }

    /// 2×1 baseline (SOF0) RGB JPEG, made with `sips`.
    fn tiny_jpeg() -> Vec<u8> {
        STANDARD.decode(TINY_JPEG).unwrap()
    }

    const TINY_JPEG: &str = "/9j/4AAQSkZJRgABAQAASABIAAD/4QBMRXhpZgAATU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6ABAAMAAAABAAEAAKACAAQAAAABAAAAAqADAAQAAAABAAAAAQAAAAD/7QA4UGhvdG9zaG9wIDMuMAA4QklNBAQAAAAAAAA4QklNBCUAAAAAABDUHYzZjwCyBOmACZjs+EJ+/8AAEQgAAQACAwEiAAIRAQMRAf/EAB8AAAEFAQEBAQEBAAAAAAAAAAABAgMEBQYHCAkKC//EALUQAAIBAwMCBAMFBQQEAAABfQECAwAEEQUSITFBBhNRYQcicRQygZGhCCNCscEVUtHwJDNicoIJChYXGBkaJSYnKCkqNDU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6g4SFhoeIiYqSk5SVlpeYmZqio6Slpqeoqaqys7S1tre4ubrCw8TFxsfIycrS09TV1tfY2drh4uPk5ebn6Onq8fLz9PX29/j5+v/EAB8BAAMBAQEBAQEBAQEAAAAAAAABAgMEBQYHCAkKC//EALURAAIBAgQEAwQHBQQEAAECdwABAgMRBAUhMQYSQVEHYXETIjKBCBRCkaGxwQkjM1LwFWJy0QoWJDThJfEXGBkaJicoKSo1Njc4OTpDREVGR0hJSlNUVVZXWFlaY2RlZmdoaWpzdHV2d3h5eoKDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uLj5OXm5+jp6vLz9PX29/j5+v/bAEMAAgICAgICAwICAwUDAwMFBgUFBQUGCAYGBgYGCAoICAgICAgKCgoKCgoKCgwMDAwMDA4ODg4ODw8PDw8PDw8PD//bAEMBAgICBAQEBwQEBxALCQsQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEP/dAAQAAf/aAAwDAQACEQMRAD8A/IvxD/yH9S/6+Zv/AEM1j1seIf8AkP6l/wBfM3/oZrHr+qD+1D//2Q==";

    #[test]
    fn pdf_wraps_the_jpeg_at_its_pixel_size() {
        let pdf = jpeg_to_pdf(&tiny_jpeg()).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(text.contains("/MediaBox [0 0 2 1]"));
        assert!(text.contains("/Filter /DCTDecode"));
        assert!(text.trim_end().ends_with("%%EOF"));
    }

    #[test]
    fn pdf_rejects_non_jpeg() {
        assert!(jpeg_to_pdf(b"not a jpeg").is_none());
    }

    #[test]
    fn png_converts_to_every_target() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("p.png");
        std::fs::write(&src, tiny_png()).unwrap();
        let magic: &[(Format, &[&[u8]])] = &[
            (Format::Jpg, &[b"\xFF\xD8"]),
            (Format::Webp, &[b"RIFF"]),
            (Format::Tiff, &[b"II", b"MM"]),
            (Format::Gif, &[b"GIF8"]),
            (Format::Bmp, &[b"BM"]),
            (Format::Pdf, &[b"%PDF"]),
        ];
        for (to, heads) in magic {
            let out = dir.path().join(format!("o.{}", to.ext()));
            convert_image(&src, *to, &out).unwrap();
            let bytes = std::fs::read(&out).unwrap();
            assert!(heads.iter().any(|h| bytes.starts_with(h)), "{to:?}");
        }
    }
}
