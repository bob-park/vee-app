//! Video conversion through an installed FFmpeg CLI: argument building, progress parsing, a cancellable run.

use super::{Format, JobError, Quality};
use std::ffi::OsString;
use std::io::BufRead;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// How often a run checks the cancel flag and the process while FFmpeg prints nothing.
const POLL: Duration = Duration::from_millis(200);

/// What a video run needs besides the files.
pub struct Run<'a> {
    /// `(ffmpeg, ffprobe)`; `None` while FFmpeg is not installed.
    pub ffmpeg: Option<(&'a Path, &'a Path)>,
    pub quality: Quality,
    /// Set to stop a running conversion.
    pub cancel: &'a AtomicBool,
    /// Called with the done share (0.0..=0.99), or `None` when the length is unknown.
    pub progress: &'a mut dyn FnMut(Option<f64>),
}

/// The full FFmpeg argument list converting `src` to `to` at `q`, writing `out`.
pub fn ffmpeg_args(src: &Path, to: Format, q: Quality, out: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-y", "-nostats", "-progress", "pipe:1", "-i"].map(OsString::from).into();
    a.push(src.into());
    let rest: Vec<String> = match to {
        Format::Webm => vec![
            "-c:v".into(),
            "libvpx-vp9".into(),
            "-crf".into(),
            q.pick(["40", "32", "24"]).into(),
            "-b:v".into(),
            "0".into(),
            "-row-mt".into(),
            "1".into(),
            "-c:a".into(),
            "libopus".into(),
            "-b:a".into(),
            q.pick(["64k", "96k", "160k"]).into(),
        ],
        Format::Gif => vec![
            "-vf".into(),
            format!(
                "fps={},scale={}:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse",
                q.pick(["10", "12", "15"]),
                q.pick(["320", "480", "'min(800,iw)'"])
            ),
            "-an".into(),
        ],
        Format::Mp3 => {
            vec!["-vn".into(), "-c:a".into(), "libmp3lame".into(), "-q:a".into(), q.pick(["5", "2", "0"]).into()]
        }
        // Mp4, Mov, Mp4720; image targets never reach FFmpeg.
        _ => {
            // libx264 with yuv420p needs even sides; `-2` alone evens only the width.
            let scale = if to == Format::Mp4720 { "scale=-2:'trunc(min(720,ih)/2)*2'" } else { "scale=trunc(iw/2)*2:trunc(ih/2)*2" };
            let mut v: Vec<String> = vec!["-vf".into(), scale.into()];
            v.extend([
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                q.pick(["28", "23", "18"]).into(),
                "-preset".into(),
                q.pick(["veryfast", "medium", "slow"]).into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                q.pick(["96k", "128k", "192k"]).into(),
            ]);
            if to != Format::Mov {
                v.extend(["-movflags".into(), "+faststart".into()]);
            }
            v
        }
    };
    a.extend(rest.into_iter().map(OsString::from));
    a.push(out.into());
    a
}

/// Seconds from `ffprobe -show_entries format=duration -of csv=p=0`; `None` for `N/A`, empty or zero.
pub fn parse_duration(stdout: &str) -> Option<f64> {
    stdout.trim().parse::<f64>().ok().filter(|d| d.is_finite() && *d > 0.0)
}

/// The done share (0.0..=1.0) from an `out_time_us=` progress line; `None` for any other line.
pub fn progress_of(line: &str, duration_s: f64) -> Option<f64> {
    let us: f64 = line.trim().strip_prefix("out_time_us=")?.parse().ok()?;
    (duration_s > 0.0).then(|| (us / (duration_s * 1_000_000.0)).clamp(0.0, 1.0))
}

/// Converts `src` to `to`, writing `out`. Stops with `Cancelled` once `ctx.cancel` is set.
/// The caller removes `out` on any error.
pub fn convert(src: &Path, to: Format, out: &Path, ffmpeg: &Path, ffprobe: &Path, ctx: &mut Run) -> Result<(), JobError> {
    let duration = probe_duration(ffprobe, src);
    (ctx.progress)(duration.map(|_| 0.0));
    // ponytail: ffprobe isn't cancellable; a cancel (or quit) during it lands here.
    if ctx.cancel.load(Ordering::SeqCst) {
        return Err(JobError::Cancelled);
    }
    let mut child = Killed(command(ffmpeg)
        .args(ffmpeg_args(src, to, ctx.quality, out))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // ponytail: FFmpeg's log is dropped; pipe and tail it if failures need a reason.
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| engine_error("ffmpeg spawn", e))?);
    let (tx, lines) = mpsc::channel();
    if let Some(stdout) = child.0.stdout.take() {
        // Not joined: it ends when the pipe closes.
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    loop {
        let closed = match lines.recv_timeout(POLL) {
            Ok(line) => {
                if let Some(p) = duration.and_then(|d| progress_of(&line, d)) {
                    (ctx.progress)(Some(p.min(0.99)));
                }
                false
            }
            Err(mpsc::RecvTimeoutError::Timeout) => false,
            Err(mpsc::RecvTimeoutError::Disconnected) => true,
        };
        let status = child.0.try_wait().map_err(|e| engine_error("ffmpeg wait", e))?;
        // Checked after the wait, so a cancel that lands as FFmpeg finishes still wins.
        // Every return drops `child`, which kills and reaps FFmpeg unless it already exited.
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err(JobError::Cancelled);
        }
        match status {
            Some(status) if status.success() => return Ok(()),
            Some(status) => return Err(engine_error("ffmpeg", status)),
            // Pipe closed but the process still runs: wait without spinning.
            None if closed => std::thread::sleep(POLL),
            None => {}
        }
    }
}

fn probe_duration(ffprobe: &Path, src: &Path) -> Option<f64> {
    let out = command(ffprobe)
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
        .arg(src)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .inspect_err(|e| log::warn!("convert: ffprobe failed: {e}"))
        .ok()?;
    out.status.success().then(|| parse_duration(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// A command that opens no console window on Windows.
pub fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// A running FFmpeg, killed and reaped when dropped: on cancel, on error, and on a panic mid-run.
/// After `try_wait` has reaped it, `kill` is a no-op (std keeps the status), so no stray PID is signalled.
struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn engine_error(step: &str, e: impl std::fmt::Display) -> JobError {
    log::warn!("convert: {step} failed: {e}");
    JobError::Engine
}

#[cfg(test)]
mod tests {
    use super::super::{run, VIDEO_TARGETS};
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn mp4_normal_args() {
        let a = ffmpeg_args(Path::new("/i.mov"), Format::Mp4, Quality::Normal, Path::new("/o.mp4"));
        let a: Vec<&str> = a.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(&a[..6], ["-y", "-nostats", "-progress", "pipe:1", "-i", "/i.mov"]);
        for w in [["-crf", "23"], ["-preset", "medium"], ["-b:a", "128k"], ["-movflags", "+faststart"]] {
            assert!(a.windows(2).any(|p| p == w), "{w:?}");
        }
        assert_eq!(*a.last().unwrap(), "/o.mp4");
    }

    #[test]
    fn p720_scales_down_only_and_mov_has_no_faststart() {
        let a = ffmpeg_args(Path::new("i"), Format::Mp4720, Quality::Small, Path::new("o"));
        assert!(a.windows(2).any(|p| p[0] == "-vf" && p[1] == "scale=-2:'trunc(min(720,ih)/2)*2'"));
        let m = ffmpeg_args(Path::new("i"), Format::Mov, Quality::Original, Path::new("o"));
        assert!(!m.iter().any(|s| s == "-movflags") && m.iter().any(|s| s == "18"));
        for to in [Format::Mp4, Format::Mov] {
            let a = ffmpeg_args(Path::new("i"), to, Quality::Normal, Path::new("o"));
            assert!(a.windows(2).any(|p| p[0] == "-vf" && p[1] == "scale=trunc(iw/2)*2:trunc(ih/2)*2"), "{to:?}");
        }
    }

    #[test]
    fn mp3_drops_video_and_gif_drops_audio() {
        assert!(ffmpeg_args(Path::new("i"), Format::Mp3, Quality::Normal, Path::new("o")).iter().any(|s| s == "-vn"));
        assert!(ffmpeg_args(Path::new("i"), Format::Gif, Quality::Normal, Path::new("o")).iter().any(|s| s == "-an"));
    }

    #[test]
    fn webm_and_gif_args_follow_quality() {
        let w = ffmpeg_args(Path::new("i"), Format::Webm, Quality::Small, Path::new("o"));
        assert!(w.windows(2).any(|p| p[0] == "-crf" && p[1] == "40") && w.iter().any(|s| s == "64k"));
        let g = ffmpeg_args(Path::new("i"), Format::Gif, Quality::Original, Path::new("o"));
        assert!(g.iter().any(|s| s
            == "fps=15,scale='min(800,iw)':-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse"));
    }

    #[test]
    fn quality_parses_with_normal_fallback() {
        assert_eq!(Quality::parse("small"), Quality::Small);
        assert_eq!(Quality::parse("original"), Quality::Original);
        assert_eq!(Quality::parse("huge"), Quality::Normal);
    }

    #[test]
    fn durations_and_progress_lines() {
        assert_eq!(parse_duration("12.500000\n"), Some(12.5));
        assert_eq!(parse_duration("N/A\n"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(progress_of("out_time_us=6250000", 12.5), Some(0.5));
        assert_eq!(progress_of("frame=10", 12.5), None);
        assert_eq!(progress_of("out_time_us=99000000", 12.5), Some(1.0));
        assert_eq!(progress_of("out_time_us=N/A", 12.5), None);
        assert_eq!(progress_of("out_time_us=-5", 12.5), Some(0.0));
    }

    /// Writes an executable shell script standing in for ffmpeg or ffprobe.
    #[cfg(unix)]
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// A fake ffmpeg that writes its output (the last argument), then runs `then`.
    #[cfg(unix)]
    fn fake(bin: &Path, then: &str) -> (PathBuf, PathBuf) {
        let ffmpeg = script(bin, "ffmpeg", &format!("for a; do out=$a; done\necho partial > \"$out\"\n{then}"));
        (ffmpeg, script(bin, "ffprobe", "echo 2.0"))
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_ffmpeg_is_an_engine_error_and_leaves_no_temp() {
        let (bin, dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = dir.path().join("a.mov");
        std::fs::write(&src, b"v").unwrap();
        let (ffmpeg, ffprobe) = fake(bin.path(), "exit 1");
        let cancel = AtomicBool::new(false);
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, cancel: &cancel, progress: &mut |_| {} };
        assert_eq!(run(&src, "a", Format::Mp4, dir.path(), tmp.path(), &mut ctx), Err(JobError::Engine));
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn cancel_mid_run_kills_ffmpeg_and_leaves_no_temp() {
        let (bin, dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = dir.path().join("a.mov");
        std::fs::write(&src, b"v").unwrap();
        let (ffmpeg, ffprobe) = fake(bin.path(), "echo out_time_us=500000\nexec sleep 10");
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let mut progress = |p: Option<f64>| {
            seen.push(p);
            if p.is_some_and(|p| p > 0.0) {
                cancel.store(true, Ordering::SeqCst);
            }
        };
        let started = std::time::Instant::now();
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, cancel: &cancel, progress: &mut progress };
        assert_eq!(run(&src, "a", Format::Mp4, dir.path(), tmp.path(), &mut ctx), Err(JobError::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(seen, vec![Some(0.0), Some(0.25)]);
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_cancel_as_ffmpeg_exits_0_saves_nothing() {
        let (bin, dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = dir.path().join("a.mov");
        std::fs::write(&src, b"v").unwrap();
        // The last progress line, then a clean exit; the cancel lands on that line.
        let (ffmpeg, ffprobe) = fake(bin.path(), "echo out_time_us=2000000\nexit 0");
        let cancel = AtomicBool::new(false);
        let mut progress = |p: Option<f64>| {
            if p.is_some_and(|p| p > 0.0) {
                cancel.store(true, Ordering::SeqCst);
            }
        };
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, cancel: &cancel, progress: &mut progress };
        assert_eq!(run(&src, "a", Format::Mp4, dir.path(), tmp.path(), &mut ctx), Err(JobError::Cancelled));
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_cancel_before_spawn_never_starts_ffmpeg() {
        let (bin, dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = dir.path().join("a.mov");
        std::fs::write(&src, b"v").unwrap();
        let marker = bin.path().join("ran");
        let (ffmpeg, ffprobe) = fake(bin.path(), &format!("touch '{}'", marker.display()));
        let cancel = AtomicBool::new(true);
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, cancel: &cancel, progress: &mut |_| {} };
        assert_eq!(run(&src, "a", Format::Mp4, dir.path(), tmp.path(), &mut ctx), Err(JobError::Cancelled));
        assert!(!marker.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_panic_mid_run_kills_ffmpeg() {
        let (bin, dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = dir.path().join("a.mov");
        std::fs::write(&src, b"v").unwrap();
        let (ffmpeg, ffprobe) = fake(bin.path(), "echo out_time_us=500000\nexec sleep 10");
        let cancel = AtomicBool::new(false);
        let started = std::time::Instant::now();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut progress = |p: Option<f64>| assert!(p.is_none_or(|p| p == 0.0), "boom");
            let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, cancel: &cancel, progress: &mut progress };
            let _ = run(&src, "a", Format::Mp4, dir.path(), tmp.path(), &mut ctx);
        }));
        assert!(panicked.is_err());
        // Unwinding reaped it: without the kill the guard would wait out the 10 s sleep.
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_video_without_ffmpeg_is_an_engine_error() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.mp4");
        std::fs::write(&src, b"v").unwrap();
        let cancel = AtomicBool::new(false);
        let mut ctx = Run { ffmpeg: None, quality: Quality::Normal, cancel: &cancel, progress: &mut |_| {} };
        assert_eq!(run(&src, "a", Format::Gif, d.path(), d.path(), &mut ctx), Err(JobError::Engine));
    }

    /// Every target at every quality, plus an odd-sized source for the libx264 targets:
    /// `VEE_FFMPEG_DIR=/opt/homebrew/bin cargo test --lib converts_a_generated_clip -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn converts_a_generated_clip() {
        let Ok(bin) = std::env::var("VEE_FFMPEG_DIR") else { return };
        let exe = std::env::consts::EXE_SUFFIX;
        let (ffmpeg, ffprobe) = (Path::new(&bin).join(format!("ffmpeg{exe}")), Path::new(&bin).join(format!("ffprobe{exe}")));
        let (dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let clip = |name: &str, size: &str| {
            let src = dir.path().join(name);
            let made = Command::new(&ffmpeg)
                .args(["-y", "-v", "error", "-f", "lavfi", "-i", &format!("testsrc=duration=1:size={size}:rate=10")])
                .args(["-f", "lavfi", "-i", "sine=duration=1"])
                .arg(&src)
                .status()
                .unwrap();
            assert!(made.success());
            src
        };
        let (even, odd) = (clip("clip.mkv", "320x240"), clip("odd.mkv", "321x241"));
        let size_of = |out: &Path| {
            let o = Command::new(&ffprobe)
                .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0"])
                .arg(out)
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        };
        let cancel = AtomicBool::new(false);
        let mut failed = Vec::new();
        let mut cases: Vec<(&Path, Format, Quality)> = Vec::new();
        for q in [Quality::Small, Quality::Normal, Quality::Original] {
            cases.extend(VIDEO_TARGETS.map(|to| (even.as_path(), to, q)));
        }
        cases.extend([Format::Mp4, Format::Mov, Format::Mp4720].map(|to| (odd.as_path(), to, Quality::Normal)));
        for (src, to, q) in cases {
            let mut last = None;
            let mut progress = |p: Option<f64>| last = p;
            let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: q, cancel: &cancel, progress: &mut progress };
            let stem = src.file_stem().unwrap().to_string_lossy().into_owned();
            let result = run(src, &stem, to, dir.path(), tmp.path(), &mut ctx);
            let ok = result.as_ref().is_ok_and(|out| std::fs::metadata(out).is_ok_and(|m| m.len() > 0));
            let size = result.as_ref().map(|out| size_of(out)).unwrap_or_default();
            println!("{stem} {} {q:?}: ok={ok} size={size} last progress {last:?}", to.id());
            let odd_side = size.split(',').any(|n| n.parse::<u32>().is_ok_and(|n| n % 2 == 1));
            if !ok || (to != Format::Gif && odd_side) {
                failed.push(format!("{stem} {} {q:?}", to.id()));
            }
        }
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        let before = std::fs::read_dir(dir.path()).unwrap().count();
        let cancelled = AtomicBool::new(true);
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, cancel: &cancelled, progress: &mut |_| {} };
        assert_eq!(run(&even, "clip", Format::Webm, dir.path(), tmp.path(), &mut ctx), Err(JobError::Cancelled));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), before);
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        assert!(failed.is_empty(), "failed: {failed:?}");
    }
}
