//! Pure pieces of recording: the container for a format and the FFmpeg args of the Windows engine.

use std::ffi::OsString;
use std::path::Path;

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
    fn container_for_maps_formats() {
        assert_eq!(container_for("mov"), Container::Mov);
        for f in ["mp4", "webm", "gif"] {
            assert_eq!(container_for(f), Container::Mp4);
        }
    }

    #[test]
    fn even_rounds_down_to_at_least_two() {
        assert_eq!((even(301), even(300), even(1)), (300, 300, 2));
    }
}
