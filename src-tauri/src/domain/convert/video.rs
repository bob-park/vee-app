//! Video conversion through an installed FFmpeg CLI: argument building, progress parsing, a cancellable run.

use super::{Format, JobError, Quality};
use std::ffi::OsString;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, mpsc};
use std::time::Duration;

/// How often a run checks the cancel flag and the process while FFmpeg prints nothing.
const POLL: Duration = Duration::from_millis(200);
/// How long a hardware encoder gets to encode its test frame (Windows probe).
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// What a video run needs besides the files.
pub struct Run<'a> {
    /// `(ffmpeg, ffprobe)`; `None` while FFmpeg is not installed.
    pub ffmpeg: Option<(&'a Path, &'a Path)>,
    pub quality: Quality,
    /// Hardware H.264 encoder to try first for MP4 / MOV / 720p; software on failure.
    pub hw: Option<HwEncoder>,
    /// Set to stop a running conversion.
    pub cancel: &'a AtomicBool,
    /// Called with the done share (0.0..=0.99), or `None` when the length is unknown.
    pub progress: &'a mut dyn FnMut(Option<f64>),
    /// The rhwp CLI; `None` while it is not installed.
    pub rhwp: Option<PathBuf>,
    /// LibreOffice's `soffice`; `None` while it is not found.
    pub soffice: Option<PathBuf>,
    /// LibreOffice's own profile dir, so a LibreOffice the user has open doesn't take the job.
    pub lo_profile: PathBuf,
}

/// A hardware H.264 encoder FFmpeg can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HwEncoder {
    /// macOS.
    VideoToolbox,
    /// Windows: NVIDIA, Intel, AMD.
    Nvenc,
    Qsv,
    Amf,
}

impl HwEncoder {
    fn name(self) -> &'static str {
        match self {
            HwEncoder::VideoToolbox => "h264_videotoolbox",
            HwEncoder::Nvenc => "h264_nvenc",
            HwEncoder::Qsv => "h264_qsv",
            HwEncoder::Amf => "h264_amf",
        }
    }

    /// The `-c:v …` encoder args at `q`, replacing libx264's `-crf` / `-preset`.
    fn video_args(self, q: Quality) -> Vec<String> {
        let mut v = vec!["-c:v".to_string(), self.name().into()];
        let cq = q.pick(["28", "23", "19"]).to_string();
        let rest: Vec<String> = match self {
            // Matched to libx264 crf 28/23/18 by output size (within ±15 %) on a noisy 1080p clip.
            HwEncoder::VideoToolbox => vec!["-q:v".into(), q.pick(["47", "60", "70"]).into()],
            HwEncoder::Nvenc => ["-preset", "p4", "-rc", "vbr", "-cq", &cq, "-b:v", "0"].map(String::from).into(),
            HwEncoder::Qsv => vec!["-global_quality".into(), cq],
            HwEncoder::Amf => vec!["-rc".into(), "cqp".into(), "-qp_i".into(), cq.clone(), "-qp_p".into(), cq],
        };
        v.extend(rest);
        v
    }
}

/// Whether `to` is encoded with H.264, the only targets a hardware encoder takes.
fn is_h264(to: Format) -> bool {
    matches!(to, Format::Mp4 | Format::Mov | Format::Mp4720)
}

/// The hardware encoder to try on this machine: VideoToolbox on macOS, elsewhere the first of
/// NVENC / QSV / AMF that encodes a test frame, probed once per app run.
pub fn hw_encoder(ffmpeg: &Path) -> Option<HwEncoder> {
    if cfg!(target_os = "macos") {
        return Some(HwEncoder::VideoToolbox);
    }
    // ponytail: cached for the run; an FFmpeg update or a new GPU is seen after a restart.
    static PROBED: OnceLock<Option<HwEncoder>> = OnceLock::new();
    *PROBED.get_or_init(|| {
        let found = [HwEncoder::Nvenc, HwEncoder::Qsv, HwEncoder::Amf].into_iter().find(|e| {
            let Ok(child) = command(ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=256x256:d=0.1"])
                .args(["-frames:v", "1", "-c:v", e.name(), "-f", "null", "-"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                return false;
            };
            // A hung GPU driver must not stall the worker: give up on this encoder after a few seconds.
            let mut child = Killed(child);
            let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
            loop {
                match child.0.try_wait() {
                    Ok(Some(s)) => return s.success(),
                    Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(POLL),
                    _ => return false,
                }
            }
        });
        log::info!("convert: hardware H.264 encoder {found:?}");
        found
    })
}

/// The full FFmpeg argument list converting `src` to `to` at `q`, writing `out`.
/// `hw` swaps libx264 for that encoder on H.264 targets; other targets ignore it.
pub fn ffmpeg_args(src: &Path, to: Format, q: Quality, out: &Path, hw: Option<HwEncoder>) -> Vec<OsString> {
    let hw = hw.filter(|_| is_h264(to));
    let mut a: Vec<OsString> = ["-y", "-nostats", "-progress", "pipe:1"].map(OsString::from).into();
    if let Some(hw) = hw {
        let decoder = if hw == HwEncoder::VideoToolbox { "videotoolbox" } else { "auto" };
        a.extend(["-hwaccel", decoder].map(OsString::from));
    }
    a.push("-i".into());
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
            match hw {
                Some(hw) => v.extend(hw.video_args(q)),
                None => v.extend([
                    "-c:v".into(),
                    "libx264".into(),
                    "-crf".into(),
                    q.pick(["28", "23", "18"]).into(),
                    "-preset".into(),
                    q.pick(["veryfast", "medium", "slow"]).into(),
                ]),
            }
            // QSV takes no yuv420p; nv12 is the same 4:2:0 layout.
            let pix_fmt = if hw == Some(HwEncoder::Qsv) { "nv12" } else { "yuv420p" };
            v.extend([
                "-pix_fmt".into(),
                pix_fmt.into(),
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
    let hw = ctx.hw.filter(|_| is_h264(to));
    match encode(ffmpeg, ffmpeg_args(src, to, ctx.quality, out, hw), duration, ctx) {
        // A hardware encoder that fails (driver, size, profile) gets one software rerun, from 0 %.
        Err(JobError::Engine) if hw.is_some() => {
            log::warn!("convert: {hw:?} failed on {}; retrying in software", src.display());
            let _ = std::fs::remove_file(out);
            (ctx.progress)(duration.map(|_| 0.0));
            encode(ffmpeg, ffmpeg_args(src, to, ctx.quality, out, None), duration, ctx)
        }
        r => r,
    }
}

/// Runs FFmpeg with `args`, reporting progress against `duration`, until it exits or `ctx.cancel` is set.
fn encode(ffmpeg: &Path, args: Vec<OsString>, duration: Option<f64>, ctx: &mut Run) -> Result<(), JobError> {
    let mut child = Killed(command(ffmpeg)
        .args(args)
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

/// Exit code of a Windows program whose DLLs are missing (`0xC0000135`, STATUS_DLL_NOT_FOUND).
const DLL_NOT_FOUND: i32 = -1073741515;

/// Runs `cmd` with no stdio until it exits, killing it after `timeout`.
/// Exit 0 is `Ok`; missing DLLs (the MSVC runtime) are `Runtime`; anything else is `Engine`.
pub(super) fn exec(cmd: &mut Command, timeout: Duration, name: &str) -> Result<(), JobError> {
    // ponytail: the tool's log is dropped; pipe and tail it if failures need a reason.
    let child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let mut child = Killed(child.map_err(|e| engine_error(name, e))?);
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.0.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                log::warn!("convert: {name} failed: {status}");
                return Err(exit_error(status.code()));
            }
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            // Dropping `child` kills and reaps it.
            Ok(None) => return Err(engine_error(name, "timed out")),
            Err(e) => return Err(engine_error(name, e)),
        }
    }
}

/// The error for a failed exit `code`.
fn exit_error(code: Option<i32>) -> JobError {
    if code == Some(DLL_NOT_FOUND) { JobError::Runtime } else { JobError::Engine }
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

    #[test]
    fn missing_dlls_are_a_runtime_error() {
        assert_eq!(exit_error(Some(-1073741515)), JobError::Runtime);
        assert_eq!(exit_error(Some(1)), JobError::Engine);
        assert_eq!(exit_error(None), JobError::Engine);
    }

    #[test]
    fn mp4_normal_args() {
        let a = ffmpeg_args(Path::new("/i.mov"), Format::Mp4, Quality::Normal, Path::new("/o.mp4"), None);
        let a: Vec<&str> = a.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(&a[..6], ["-y", "-nostats", "-progress", "pipe:1", "-i", "/i.mov"]);
        for w in [["-crf", "23"], ["-preset", "medium"], ["-b:a", "128k"], ["-movflags", "+faststart"]] {
            assert!(a.windows(2).any(|p| p == w), "{w:?}");
        }
        assert_eq!(*a.last().unwrap(), "/o.mp4");
    }

    #[test]
    fn p720_scales_down_only_and_mov_has_no_faststart() {
        let a = ffmpeg_args(Path::new("i"), Format::Mp4720, Quality::Small, Path::new("o"), None);
        assert!(a.windows(2).any(|p| p[0] == "-vf" && p[1] == "scale=-2:'trunc(min(720,ih)/2)*2'"));
        let m = ffmpeg_args(Path::new("i"), Format::Mov, Quality::Original, Path::new("o"), None);
        assert!(!m.iter().any(|s| s == "-movflags") && m.iter().any(|s| s == "18"));
        for to in [Format::Mp4, Format::Mov] {
            let a = ffmpeg_args(Path::new("i"), to, Quality::Normal, Path::new("o"), None);
            assert!(a.windows(2).any(|p| p[0] == "-vf" && p[1] == "scale=trunc(iw/2)*2:trunc(ih/2)*2"), "{to:?}");
        }
    }

    #[test]
    fn mp3_drops_video_and_gif_drops_audio() {
        assert!(ffmpeg_args(Path::new("i"), Format::Mp3, Quality::Normal, Path::new("o"), None).iter().any(|s| s == "-vn"));
        assert!(ffmpeg_args(Path::new("i"), Format::Gif, Quality::Normal, Path::new("o"), None).iter().any(|s| s == "-an"));
    }

    #[test]
    fn webm_and_gif_args_follow_quality() {
        let w = ffmpeg_args(Path::new("i"), Format::Webm, Quality::Small, Path::new("o"), None);
        assert!(w.windows(2).any(|p| p[0] == "-crf" && p[1] == "40") && w.iter().any(|s| s == "64k"));
        let g = ffmpeg_args(Path::new("i"), Format::Gif, Quality::Original, Path::new("o"), None);
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
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, hw: None, cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut |_| {} };
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
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, hw: None, cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut progress };
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
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, hw: None, cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut progress };
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
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, hw: None, cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut |_| {} };
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
            let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, hw: None, cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut progress };
            let _ = run(&src, "a", Format::Mp4, dir.path(), tmp.path(), &mut ctx);
        }));
        assert!(panicked.is_err());
        // Unwinding reaped it: without the kill the guard would wait out the 10 s sleep.
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    /// `ffmpeg_args` as `&str`s.
    fn args(to: Format, q: Quality, hw: Option<HwEncoder>) -> Vec<String> {
        ffmpeg_args(Path::new("i"), to, q, Path::new("o"), hw).iter().map(|s| s.to_string_lossy().into_owned()).collect()
    }

    fn has(a: &[String], pair: [&str; 2]) -> bool {
        a.windows(2).any(|p| p[0] == pair[0] && p[1] == pair[1])
    }

    #[test]
    fn videotoolbox_replaces_libx264_and_decodes_in_hardware() {
        let a = args(Format::Mp4, Quality::Normal, Some(HwEncoder::VideoToolbox));
        assert_eq!(&a[4..8], ["-hwaccel", "videotoolbox", "-i", "i"]);
        for w in [["-c:v", "h264_videotoolbox"], ["-q:v", "60"], ["-pix_fmt", "yuv420p"], ["-movflags", "+faststart"], ["-b:a", "128k"]] {
            assert!(has(&a, w), "{w:?}");
        }
        assert!(has(&a, ["-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2"]));
        assert!(!a.iter().any(|s| s == "libx264" || s == "-crf" || s == "-preset"));
        let p = args(Format::Mp4720, Quality::Small, Some(HwEncoder::VideoToolbox));
        assert!(has(&p, ["-q:v", "47"]) && has(&p, ["-vf", "scale=-2:'trunc(min(720,ih)/2)*2'"]));
        let m = args(Format::Mov, Quality::Original, Some(HwEncoder::VideoToolbox));
        assert!(has(&m, ["-q:v", "70"]) && !m.iter().any(|s| s == "-movflags"));
    }

    #[test]
    fn non_h264_targets_ignore_the_hardware_encoder() {
        for to in [Format::Webm, Format::Gif, Format::Mp3] {
            assert_eq!(args(to, Quality::Normal, Some(HwEncoder::Nvenc)), args(to, Quality::Normal, None), "{to:?}");
        }
    }

    #[test]
    fn windows_encoder_args() {
        let n = args(Format::Mp4, Quality::Original, Some(HwEncoder::Nvenc));
        assert_eq!(&n[4..6], ["-hwaccel", "auto"]);
        for w in [["-c:v", "h264_nvenc"], ["-preset", "p4"], ["-rc", "vbr"], ["-cq", "19"], ["-b:v", "0"], ["-pix_fmt", "yuv420p"]] {
            assert!(has(&n, w), "{w:?}");
        }
        let q = args(Format::Mp4, Quality::Small, Some(HwEncoder::Qsv));
        assert!(has(&q, ["-c:v", "h264_qsv"]) && has(&q, ["-global_quality", "28"]) && has(&q, ["-pix_fmt", "nv12"]));
        let a = args(Format::Mov, Quality::Normal, Some(HwEncoder::Amf));
        for w in [["-c:v", "h264_amf"], ["-rc", "cqp"], ["-qp_i", "23"], ["-qp_p", "23"]] {
            assert!(has(&a, w), "{w:?}");
        }
        for a in [&n, &q, &a] {
            assert!(!a.iter().any(|s| s == "libx264" || s == "-crf"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_hardware_encode_reruns_in_software() {
        let (bin, dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = dir.path().join("a.mov");
        std::fs::write(&src, b"v").unwrap();
        // Fails with the hardware encoder after a partial write; writes `sw` without it.
        let ffmpeg = script(
            bin.path(),
            "ffmpeg",
            "for a; do out=$a; done\ncase \"$*\" in *h264_videotoolbox*) echo partial > \"$out\"; echo out_time_us=1000000; exit 1;; esac\necho sw > \"$out\"",
        );
        let ffprobe = script(bin.path(), "ffprobe", "echo 2.0");
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let mut progress = |p: Option<f64>| seen.push(p);
        let mut ctx = Run {
            ffmpeg: Some((&ffmpeg, &ffprobe)),
            quality: Quality::Normal,
            hw: Some(HwEncoder::VideoToolbox),
            cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(),
            progress: &mut progress,
        };
        let out = run(&src, "a", Format::Mp4, dir.path(), tmp.path(), &mut ctx).unwrap();
        assert_eq!(std::fs::read_to_string(out).unwrap(), "sw\n");
        // Progress restarts at 0 for the software run.
        assert_eq!(seen, vec![Some(0.0), Some(0.5), Some(0.0)]);
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_video_without_ffmpeg_is_an_engine_error() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.mp4");
        std::fs::write(&src, b"v").unwrap();
        let cancel = AtomicBool::new(false);
        let mut ctx = Run { ffmpeg: None, quality: Quality::Normal, hw: None, cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut |_| {} };
        assert_eq!(run(&src, "a", Format::Gif, d.path(), d.path(), &mut ctx), Err(JobError::Engine));
    }

    /// Every target at every quality, plus an odd-sized source for the H.264 targets, and those
    /// again through this machine's hardware encoder:
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
        let mut cases: Vec<(&Path, Format, Quality, Option<HwEncoder>)> = Vec::new();
        for q in [Quality::Small, Quality::Normal, Quality::Original] {
            cases.extend(VIDEO_TARGETS.map(|to| (even.as_path(), to, q, None)));
        }
        const H264: [Format; 3] = [Format::Mp4, Format::Mov, Format::Mp4720];
        cases.extend(H264.map(|to| (odd.as_path(), to, Quality::Normal, None)));
        // The hardware path, where this machine has one (VideoToolbox on macOS, a probed encoder elsewhere).
        if let Some(hw) = hw_encoder(&ffmpeg) {
            for src in [even.as_path(), odd.as_path()] {
                cases.extend(H264.map(|to| (src, to, Quality::Normal, Some(hw))));
            }
        }
        for (src, to, q, hw) in cases {
            let mut last = None;
            let mut progress = |p: Option<f64>| last = p;
            let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: q, hw, cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut progress };
            let stem = src.file_stem().unwrap().to_string_lossy().into_owned();
            let result = run(src, &stem, to, dir.path(), tmp.path(), &mut ctx);
            let ok = result.as_ref().is_ok_and(|out| std::fs::metadata(out).is_ok_and(|m| m.len() > 0));
            let size = result.as_ref().map(|out| size_of(out)).unwrap_or_default();
            println!("{stem} {} {q:?} {hw:?}: ok={ok} size={size} last progress {last:?}", to.id());
            let odd_side = size.split(',').any(|n| n.parse::<u32>().is_ok_and(|n| n % 2 == 1));
            // libx264 stamps its options into the stream; a hardware case carrying it fell back to software.
            let x264 = result.as_ref().is_ok_and(|out| std::fs::read(out).unwrap().windows(4).any(|w| w == b"x264"));
            if !ok || (to != Format::Gif && odd_side) || (hw.is_some() && x264) {
                failed.push(format!("{stem} {} {q:?} {hw:?}", to.id()));
            }
        }
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        let before = std::fs::read_dir(dir.path()).unwrap().count();
        let cancelled = AtomicBool::new(true);
        let mut ctx = Run { ffmpeg: Some((&ffmpeg, &ffprobe)), quality: Quality::Normal, hw: None, cancel: &cancelled, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut |_| {} };
        assert_eq!(run(&even, "clip", Format::Webm, dir.path(), tmp.path(), &mut ctx), Err(JobError::Cancelled));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), before);
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        assert!(failed.is_empty(), "failed: {failed:?}");
    }
}
