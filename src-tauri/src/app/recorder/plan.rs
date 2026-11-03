//! Pure pieces of recording: formats, the saved file's name and folder, and the FFmpeg args of the Windows engine.

use crate::domain::convert::Format;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// How long a GIF may record before it stops itself.
pub const GIF_LIMIT_MS: i64 = 30_000;

/// The `Format` of a `recordFormat` value (`mp4` · `mov` · `webm` · `gif`).
pub fn record_format(value: &str) -> Option<Format> {
    Some(match value {
        "mp4" => Format::Mp4,
        "mov" => Format::Mov,
        "webm" => Format::Webm,
        "gif" => Format::Gif,
        _ => return None,
    })
}

/// The auto-stop of `format`: GIF only.
pub fn limit_ms(format: Format) -> Option<i64> {
    (format == Format::Gif).then_some(GIF_LIMIT_MS)
}

/// Whether recording `format` needs FFmpeg: on macOS only to convert to WEBM / GIF, on Windows always.
pub fn needs_ffmpeg(format: Format, macos: bool) -> bool {
    !macos || matches!(format, Format::Webm | Format::Gif)
}

/// The saved file's name from the local time `(year, month, day, hour, minute, second)`; the same in every locale.
pub fn file_name((y, mo, d, h, mi, s): (i32, u32, u32, u32, u32, u32), ext: &str) -> String {
    format!("Vee Recording {y:04}-{mo:02}-{d:02} at {h:02}.{mi:02}.{s:02}.{ext}")
}

/// `dir/name`, or the first free `dir/<stem> (2).<ext>`, `(3)`, …
pub fn unique_path(dir: &Path, name: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let first = dir.join(name);
    if !exists(&first) {
        return first;
    }
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let ext = if ext.is_empty() { String::new() } else { format!(".{ext}") };
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !exists(p)).unwrap()
}

/// The folder a recording is saved in: the `recordFolder` setting, or `downloads` when it is unset or gone.
/// Second: the setting's folder is gone, so the user is told.
pub fn resolve_folder(setting: &str, downloads: &Path, is_dir: impl Fn(&Path) -> bool) -> (PathBuf, bool) {
    match setting {
        "" => (downloads.to_path_buf(), false),
        s if is_dir(Path::new(s)) => (PathBuf::from(s), false),
        _ => (downloads.to_path_buf(), true),
    }
}

/// The file the OS engine writes; WEBM and GIF are recorded as MP4 and converted afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Mp4,
    Mov,
}

/// The container a `recordFormat` setting value is recorded into.
pub fn container_for(format: &str) -> Container {
    if format == "mov" { Container::Mov } else { Container::Mp4 }
}

/// `n` rounded down to even, at least 2: H.264 with 4:2:0 chroma needs even sides.
pub fn even(n: u32) -> u32 {
    (n & !1).max(2)
}

/// FFmpeg args grabbing the global physical rect `(x, y, w, h)` of the desktop at 30 fps into `out` (Matroska,
/// which stays readable if FFmpeg dies mid-write). gdigrab, not ddagrab: it works on every GPU and in RDP / VM
/// sessions and takes a plain offset + size, where ddagrab is per-output and needs a D3D11 device.
/// `ultrafast` keeps the encoder from dropping frames; the file is re-muxed, not re-encoded, on stop.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn grab_args(phys: (i32, i32, u32, u32), out: &Path) -> Vec<OsString> {
    let (x, y, w, h) = phys;
    let mut a: Vec<OsString> =
        ["-y", "-hide_banner", "-loglevel", "error", "-nostats", "-f", "gdigrab", "-framerate", "30", "-draw_mouse", "1"]
            .map(OsString::from)
            .into();
    a.extend(["-offset_x".into(), x.to_string(), "-offset_y".into(), y.to_string()].map(OsString::from));
    a.extend(["-video_size".into(), format!("{}x{}", even(w), even(h))].map(OsString::from));
    a.extend(["-i", "desktop", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "18", "-pix_fmt", "yuv420p"].map(OsString::from));
    a.push(out.into());
    a
}

/// FFmpeg args joining the grabbed `video` and the loopback `audio` WAV into `out` (MP4 or MOV by extension):
/// the video stream is copied, the audio encoded to AAC, and the longer input is cut to the shorter.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn mux_args(video: &Path, audio: &Path, out: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-y", "-hide_banner", "-loglevel", "error", "-nostats", "-i"].map(OsString::from).into();
    a.push(video.into());
    a.push("-i".into());
    a.push(audio.into());
    a.extend(
        ["-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-shortest", "-movflags", "+faststart"]
            .map(OsString::from),
    );
    a.push(out.into());
    a
}

/// FFmpeg args remuxing the grabbed `video` alone into `out`, at its full length: the fallback when the loopback
/// audio failed mid-recording, where `-shortest` against the cut WAV would trim the video.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn mux_args_video_only(video: &Path, out: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-y", "-hide_banner", "-loglevel", "error", "-nostats", "-i"].map(OsString::from).into();
    a.push(video.into());
    a.extend(["-map", "0:v:0", "-c:v", "copy", "-movflags", "+faststart"].map(OsString::from));
    a.push(out.into());
    a
}

/// The longest side of a piped window recording; larger windows are scaled down to it.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub const MAX_SIDE: u32 = 1920;

/// The frame rate of a piped window recording.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub const FPS: u32 = 30;

/// The fixed output size of a window first seen at `w`×`h`: the longest side capped at `MAX_SIDE`, both sides even.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn output_size(w: u32, h: u32) -> (u32, u32) {
    let max = w.max(h);
    let (w, h) = if max > MAX_SIDE {
        let s = f64::from(MAX_SIDE) / f64::from(max);
        ((f64::from(w) * s).round() as u32, (f64::from(h) * s).round() as u32)
    } else {
        (w, h)
    };
    (even(w), even(h))
}

/// Where a `src`-sized frame sits in a `dst` canvas as `(x, y, w, h)`: shrunk to fit keeping its aspect, never
/// upscaled, centred.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn fit_rect(src: (u32, u32), dst: (u32, u32)) -> (u32, u32, u32, u32) {
    let f = f64::from;
    let scale = (f(dst.0) / f(src.0)).min(f(dst.1) / f(src.1)).min(1.0);
    let w = ((f(src.0) * scale).round() as u32).clamp(1, dst.0);
    let h = ((f(src.1) * scale).round() as u32).clamp(1, dst.1);
    ((dst.0 - w) / 2, (dst.1 - h) / 2, w, h)
}

/// A tight BGRA `src` frame laid onto a black tight BGRA `dst_size` canvas at its `fit_rect`, so a resized window
/// keeps the output size fixed.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn compose(src: &[u8], src_size: (u32, u32), dst_size: (u32, u32)) -> Vec<u8> {
    use image::{RgbaImage, imageops};
    if src_size == dst_size {
        return src.to_vec();
    }
    let (x, y, w, h) = fit_rect(src_size, dst_size);
    // BGRA treated as RGBA: resizing doesn't care about channel order.
    let fitted: std::borrow::Cow<[u8]> = if (w, h) == src_size {
        src.into()
    } else {
        let img = RgbaImage::from_raw(src_size.0, src_size.1, src.to_vec()).expect("src is tight BGRA of src_size");
        imageops::resize(&img, w, h, imageops::FilterType::Triangle).into_raw().into()
    };
    let mut out = [0u8, 0, 0, 255].repeat((dst_size.0 * dst_size.1) as usize);
    let (row, stride) = (w as usize * 4, dst_size.0 as usize * 4);
    for (r, line) in fitted.chunks_exact(row).enumerate() {
        let at = (y as usize + r) * stride + x as usize * 4;
        out[at..at + row].copy_from_slice(line);
    }
    out
}

/// How many frames should have been written `elapsed` after the start, at `FPS`; frame 0 is due at once.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn frames_due(elapsed: std::time::Duration) -> u64 {
    (elapsed.as_nanos() * u128::from(FPS) / 1_000_000_000) as u64 + 1
}

/// FFmpeg args reading rawvideo BGRA `w`×`h` frames at `FPS` from stdin into `out`, with the same encoder
/// settings as `grab_args`.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn pipe_args(w: u32, h: u32, out: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> =
        ["-y", "-hide_banner", "-loglevel", "error", "-nostats", "-f", "rawvideo", "-pix_fmt", "bgra"].map(OsString::from).into();
    a.extend(["-s".into(), format!("{w}x{h}"), "-r".into(), FPS.to_string()].map(OsString::from));
    a.extend(["-i", "-", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "18", "-pix_fmt", "yuv420p"].map(OsString::from));
    a.push(out.into());
    a
}

/// A global rect `win` clipped to the global `display` rect, in display-local coordinates; `None` if off it.
pub fn clip_to_display(win: (f64, f64, f64, f64), display: (f64, f64, f64, f64)) -> Option<(f64, f64, f64, f64)> {
    let (x0, y0) = (win.0.max(display.0), win.1.max(display.1));
    let (x1, y1) = ((win.0 + win.2).min(display.0 + display.2), (win.1 + win.3).min(display.1 + display.3));
    (x1 > x0 && y1 > y0).then_some((x0 - display.0, y0 - display.1, x1 - x0, y1 - y0))
}

/// `clip_to_display` in global physical px: the window rect `win` cut to the monitor rect `display`, still global.
#[cfg_attr(not(windows), allow(dead_code))] // the Windows engine
pub fn clip_physical(win: (i32, i32, u32, u32), display: (i32, i32, u32, u32)) -> Option<(i32, i32, u32, u32)> {
    let f = |(x, y, w, h): (i32, i32, u32, u32)| (f64::from(x), f64::from(y), f64::from(w), f64::from(h));
    let (x, y, w, h) = clip_to_display(f(win), f(display))?;
    Some((display.0 + x as i32, display.1 + y as i32, w as u32, h as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(a: &[OsString], w: &[&str]) -> bool {
        a.windows(w.len()).any(|s| s.iter().zip(w).all(|(x, y)| x == y))
    }

    #[test]
    fn grab_args_sets_offset_and_even_size() {
        let a = grab_args((10, 20, 301, 201), Path::new("/t/video.mkv"));
        assert!(has(&a, &["-offset_x", "10", "-offset_y", "20", "-video_size", "300x200"]));
        assert!(has(&a, &["-draw_mouse", "1"]));
        assert_eq!(a.last().unwrap(), "/t/video.mkv");
    }

    #[test]
    fn mux_args_copies_video_and_encodes_aac() {
        let a = mux_args(Path::new("/t/v.mkv"), Path::new("/t/a.wav"), Path::new("/o/r.mp4"));
        assert!(has(&a, &["-i", "/t/v.mkv"]) && has(&a, &["-i", "/t/a.wav"]));
        assert!(has(&a, &["-c:v", "copy"]) && has(&a, &["-c:a", "aac"]) && has(&a, &["-shortest"]));
        assert_eq!(a.last().unwrap(), "/o/r.mp4");
    }

    #[test]
    fn mux_args_video_only_keeps_full_length_without_audio() {
        let a = mux_args_video_only(Path::new("/t/v.mkv"), Path::new("/o/r.mp4"));
        assert!(has(&a, &["-i", "/t/v.mkv"]) && has(&a, &["-map", "0:v:0"]) && has(&a, &["-c:v", "copy"]));
        assert_eq!(a.iter().filter(|s| *s == "-i").count(), 1);
        assert!(!a.iter().any(|s| s == "-shortest" || s == "1:a:0" || s == "-c:a"));
        assert_eq!(a.last().unwrap(), "/o/r.mp4");
    }

    #[test]
    fn container_for_maps_formats() {
        assert_eq!(container_for("mov"), Container::Mov);
        for f in ["mp4", "webm", "gif"] {
            assert_eq!(container_for(f), Container::Mp4);
        }
    }

    #[test]
    fn clip_physical_stays_global() {
        let d = (1920, 0, 2560, 1440);
        assert_eq!(clip_physical((1800, -40, 400, 300), d), Some((1920, 0, 280, 260)));
        assert_eq!(clip_physical((2000, 100, 400, 300), d), Some((2000, 100, 400, 300)));
        assert_eq!(clip_physical((0, 0, 400, 300), d), None);
    }

    #[test]
    fn clip_to_display_crops_and_localizes() {
        let d = (1512.0, 0.0, 1920.0, 1080.0);
        assert_eq!(clip_to_display((1600.0, 100.0, 400.0, 300.0), d), Some((88.0, 100.0, 400.0, 300.0)));
        assert_eq!(clip_to_display((1400.0, -50.0, 400.0, 300.0), d), Some((0.0, 0.0, 288.0, 250.0)));
        assert_eq!(clip_to_display((0.0, 0.0, 400.0, 300.0), d), None);
    }

    #[test]
    fn file_name_is_fixed_format() {
        assert_eq!(file_name((2026, 11, 3, 15, 12, 8), "mp4"), "Vee Recording 2026-11-03 at 15.12.08.mp4");
    }

    #[test]
    fn unique_path_appends_counter() {
        let dir = Path::new("/d");
        let taken = [dir.join("a.mp4"), dir.join("a (2).mp4")];
        assert_eq!(unique_path(dir, "a.mp4", |p| taken.iter().any(|t| t == p)), dir.join("a (3).mp4"));
        assert_eq!(unique_path(dir, "b.mp4", |p| taken.iter().any(|t| t == p)), dir.join("b.mp4"));
        let name = "Vee Recording 2026-11-03 at 15.12.08.gif";
        assert_eq!(unique_path(dir, name, |p| p == dir.join(name)), dir.join("Vee Recording 2026-11-03 at 15.12.08 (2).gif"));
    }

    #[test]
    fn resolve_folder_falls_back_to_downloads() {
        let dl = Path::new("/Users/u/Downloads");
        assert_eq!(resolve_folder("", dl, |_| true), (dl.to_path_buf(), false));
        assert_eq!(resolve_folder("/gone", dl, |_| false), (dl.to_path_buf(), true));
        assert_eq!(resolve_folder("/Movies", dl, |p| p == Path::new("/Movies")), (PathBuf::from("/Movies"), false));
    }

    #[test]
    fn needs_ffmpeg_per_os() {
        assert!(!needs_ffmpeg(Format::Mp4, true) && !needs_ffmpeg(Format::Mov, true));
        assert!(needs_ffmpeg(Format::Webm, true) && needs_ffmpeg(Format::Gif, true));
        for f in [Format::Mp4, Format::Mov, Format::Webm, Format::Gif] {
            assert!(needs_ffmpeg(f, false));
        }
    }

    #[test]
    fn record_format_parses_and_only_gif_has_a_limit() {
        assert_eq!(record_format("gif"), Some(Format::Gif));
        assert_eq!(record_format("mkv"), None);
        assert_eq!(limit_ms(Format::Gif), Some(30_000));
        assert_eq!(limit_ms(Format::Mp4), None);
    }

    #[test]
    fn even_rounds_down_to_at_least_two() {
        assert_eq!((even(301), even(300), even(1)), (300, 300, 2));
    }

    #[test]
    fn output_size_caps_longest_side_and_keeps_even() {
        assert_eq!(output_size(3840, 2160), (1920, 1080));
        assert_eq!(output_size(2160, 3840), (1080, 1920));
        assert_eq!(output_size(4000, 1000), (1920, 480));
        assert_eq!(output_size(1000, 700), (1000, 700));
        assert_eq!(output_size(1001, 701), (1000, 700));
        assert_eq!(output_size(1920, 1200), (1920, 1200));
        assert_eq!(output_size(1, 1), (2, 2));
    }

    #[test]
    fn fit_rect_shrinks_larger_and_centres_smaller() {
        assert_eq!(fit_rect((1920, 1080), (1920, 1080)), (0, 0, 1920, 1080));
        assert_eq!(fit_rect((800, 600), (1920, 1080)), (560, 240, 800, 600)); // no upscale
        assert_eq!(fit_rect((3840, 1080), (1920, 1080)), (0, 270, 1920, 540));
        assert_eq!(fit_rect((1000, 2000), (1920, 1080)), (690, 0, 540, 1080));
        let (_, _, w, h) = fit_rect((10_000, 1), (100, 100));
        assert!(w >= 1 && h >= 1);
    }

    #[test]
    fn compose_same_size_is_identity() {
        let src: Vec<u8> = (0..2 * 2 * 4).map(|i| i as u8).collect();
        assert_eq!(compose(&src, (2, 2), (2, 2)), src);
    }

    #[test]
    fn compose_centres_smaller_on_black() {
        let red = [0u8, 0, 255, 255];
        let src: Vec<u8> = red.repeat(4); // 2×2
        let out = compose(&src, (2, 2), (4, 4));
        assert_eq!(out.len(), 4 * 4 * 4);
        let px = |x: usize, y: usize| &out[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
        assert_eq!(px(0, 0), [0, 0, 0, 255]);
        assert_eq!(px(1, 1), red);
        assert_eq!(px(2, 2), red);
        assert_eq!(px(3, 3), [0, 0, 0, 255]);
    }

    #[test]
    fn compose_shrinks_wider_with_letterbox() {
        let src: Vec<u8> = [255u8, 255, 255, 255].repeat(4 * 2); // white 4×2
        let out = compose(&src, (4, 2), (2, 2)); // fits as 2×1 at y = 0
        assert_eq!(out.len(), 2 * 2 * 4);
        assert_eq!(&out[2 * 4..], [0, 0, 0, 255, 0, 0, 0, 255]); // row 1 black
        assert!(out[..2 * 4].iter().all(|&b| b > 200)); // row 0 white-ish
    }

    #[test]
    fn frames_due_follows_the_wall_clock() {
        use std::time::Duration;
        assert_eq!(frames_due(Duration::ZERO), 1);
        assert_eq!(frames_due(Duration::from_millis(33)), 1);
        assert_eq!(frames_due(Duration::from_millis(34)), 2);
        assert_eq!(frames_due(Duration::from_secs(1)), 31);
        assert_eq!(frames_due(Duration::from_secs(60)), 1801);
    }

    #[test]
    fn pipe_args_read_bgra_from_stdin() {
        let a = pipe_args(1920, 1080, Path::new("v.mkv"));
        let s: Vec<&str> = a.iter().map(|x| x.to_str().unwrap()).collect();
        assert_eq!(
            s,
            [
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostats",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "bgra",
                "-s",
                "1920x1080",
                "-r",
                "30",
                "-i",
                "-",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-crf",
                "18",
                "-pix_fmt",
                "yuv420p",
                "v.mkv"
            ]
        );
    }
}
