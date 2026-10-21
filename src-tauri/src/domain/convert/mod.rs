//! Convert domain: which image, video or document formats a dragged card can become, and where the output goes.

mod hwp;
mod image;
mod model;
mod office;
mod process;
mod video;

use self::image::convert_image;
pub use model::{Caps, Engine, Format, Job, JobError, JobState, JobView, Quality, Slice, DONE_MS, SHOW_AFTER_MS};
use model::eta;
pub use process::{command, output};
pub use video::{Run, hw_encoder};
use model::{DocTargets, HWP_TARGETS, IMAGE_TARGETS, SHEET_TARGETS, SLIDE_TARGETS, TEXT_TARGETS, VIDEO_TARGETS};

use std::path::{Path, PathBuf};

/// Slices every source can become, in target order, minus each source's own format.
/// All images, all videos, or all one document family; empty when `paths` is empty, mixes families,
/// or holds anything else. A slice needs its engine while `caps` lacks it; image slices never do.
pub fn formats_for(paths: &[PathBuf], caps: Caps) -> Vec<Slice> {
    if !paths.is_empty() && paths.iter().all(|p| is_video(p)) {
        let own: Vec<Format> = paths.iter().filter_map(|p| ext_of(p).and_then(video_container)).collect();
        let needs = (!caps.ffmpeg).then_some(Engine::Ffmpeg);
        return VIDEO_TARGETS.into_iter().filter(|f| !own.contains(f)).map(|format| Slice { format, needs }).collect();
    }
    if let Some((first, _)) = paths.first().and_then(|p| doc_source(p)) {
        let sources: Option<Vec<Option<Format>>> =
            paths.iter().map(|p| doc_source(p).filter(|(t, _)| *t == first).map(|(_, own)| own)).collect();
        let Some(own) = sources else { return Vec::new() };
        return first
            .iter()
            .filter(|(f, _)| !own.contains(&Some(*f)))
            .map(|&(format, engine)| Slice { format, needs: (!caps.has(engine)).then_some(engine) })
            .collect();
    }
    let sources: Option<Vec<Format>> = paths.iter().map(|p| ext_of(p).and_then(Format::from_ext)).collect();
    match sources {
        // Without HEIC support (Windows) a HEIC source can't be read either.
        Some(sources) if !sources.is_empty() && (caps.heic || !sources.contains(&Format::Heic)) => IMAGE_TARGETS
            .into_iter()
            .filter(|f| (caps.heic || *f != Format::Heic) && !sources.contains(f))
            .map(|format| Slice { format, needs: None })
            .collect(),
        _ => Vec::new(),
    }
}

/// Whether `path` has a video extension (any case).
pub fn is_video(path: &Path) -> bool {
    // No `ts`: TypeScript files share it. MPEG-TS camcorder files come as `mts`/`m2ts`.
    const EXTS: [&str; 13] =
        ["mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv", "flv", "mpg", "mpeg", "mts", "m2ts", "3gp"];
    ext_of(path).is_some_and(|e| EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// A document source's family targets and the target format it already is (`None` for `doc`, `xls`, `ppt`).
fn doc_source(path: &Path) -> Option<(DocTargets, Option<Format>)> {
    Some(match ext_of(path)?.to_ascii_lowercase().as_str() {
        "hwp" => (HWP_TARGETS, Some(Format::Hwp)),
        "hwpx" => (HWP_TARGETS, Some(Format::Hwpx)),
        "docx" => (TEXT_TARGETS, Some(Format::Docx)),
        "doc" => (TEXT_TARGETS, None),
        "odt" => (TEXT_TARGETS, Some(Format::Odt)),
        "rtf" => (TEXT_TARGETS, Some(Format::Rtf)),
        "txt" => (TEXT_TARGETS, Some(Format::Txt)),
        "html" | "htm" => (TEXT_TARGETS, Some(Format::Html)),
        "xlsx" => (SHEET_TARGETS, Some(Format::Xlsx)),
        "xls" => (SHEET_TARGETS, None),
        "ods" => (SHEET_TARGETS, Some(Format::Ods)),
        "pptx" => (SLIDE_TARGETS, Some(Format::Pptx)),
        "ppt" => (SLIDE_TARGETS, None),
        "odp" => (SLIDE_TARGETS, Some(Format::Odp)),
        _ => return None,
    })
}

/// The video target a source already is, so the donut leaves it out. `720p` always stays.
fn video_container(ext: &str) -> Option<Format> {
    match ext.to_ascii_lowercase().as_str() {
        "mp4" | "m4v" => Some(Format::Mp4),
        "mov" => Some(Format::Mov),
        "webm" => Some(Format::Webm),
        _ => None,
    }
}

fn ext_of(path: &Path) -> Option<&str> {
    path.extension().and_then(|e| e.to_str())
}

/// `dir/stem.ext`, or the first free `dir/stem (n).ext` so an existing file is never overwritten.
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (1..).map(|n| dir.join(format!("{stem} ({n}).{ext}"))).find(|p| !p.exists()).unwrap()
}

/// Converts `src` into `tmp`, then moves the result to a free `dir/stem.<ext>` and returns that path.
/// Video sources go through FFmpeg (`ctx.ffmpeg`, `Engine` when missing), images through the image engine.
/// The source is never touched and the temp file never outlives the call (error and cancel included).
pub fn run(src: &Path, stem: &str, to: Format, dir: &Path, tmp: &Path, ctx: &mut Run) -> Result<PathBuf, JobError> {
    if !src.is_file() {
        return Err(JobError::Missing);
    }
    let staged = unique_path(tmp, stem, to.ext());
    let converted = if is_video(src) {
        match ctx.ffmpeg {
            Some((ffmpeg, ffprobe)) => video::convert(src, to, &staged, ffmpeg, ffprobe, ctx),
            None => Err(JobError::Engine),
        }
    } else if let Some((targets, _)) = doc_source(src) {
        convert_doc(src, to, &staged, tmp, targets, ctx)
    } else {
        convert_image(src, to, &staged)
    };
    let result = converted.and_then(|()| {
        let out = unique_path(dir, stem, to.ext());
        move_file(&staged, &out).map(|()| out).map_err(|e| {
            log::warn!("convert: moving {} into {} failed: {e}", staged.display(), dir.display());
            JobError::Write
        })
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    result
}

/// Converts a document with the engine its family uses for `to`.
/// `Engine` when that engine is missing or `to` isn't one of the family's targets.
fn convert_doc(src: &Path, to: Format, out: &Path, tmp: &Path, targets: DocTargets, ctx: &Run) -> Result<(), JobError> {
    let Some(&(_, engine)) = targets.iter().find(|(f, _)| *f == to) else { return Err(JobError::Engine) };
    match engine {
        Engine::Rhwp => match &ctx.rhwp {
            Some(rhwp) => hwp::convert(src, to, out, rhwp, hwp::RHWP_TIMEOUT, ctx.cancel),
            None => Err(JobError::Engine),
        },
        Engine::LibreOffice => match &ctx.soffice {
            Some(soffice) => office::convert_lo(src, to, out, tmp, soffice, &ctx.lo_profile, office::OFFICE_TIMEOUT, ctx.cancel),
            None => Err(JobError::Engine),
        },
        Engine::Ffmpeg => Err(JobError::Engine),
    }
}

/// Rename, or copy + remove when `from` and `to` are on different volumes.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to).or_else(|_| {
        std::fs::copy(from, to).inspect_err(|_| {
            let _ = std::fs::remove_file(to);
        })?;
        std::fs::remove_file(from)
    })
}

/// Views of the jobs the panel shows now, in list order. Marks queued/running jobs `shown`
/// once they have waited `SHOW_AFTER_MS`; failed jobs always show; a done job lingers
/// `DONE_MS` only if it was shown before it finished.
pub fn visible_jobs(jobs: &mut [Job], now: i64) -> Vec<JobView> {
    let mut queued = 0;
    let mut views = Vec::new();
    for job in jobs.iter_mut() {
        let mut position = 0;
        let visible = match job.state {
            JobState::Queued | JobState::Running => {
                if job.state == JobState::Queued {
                    queued += 1;
                    position = queued;
                }
                job.shown |= now - job.since >= SHOW_AFTER_MS;
                job.shown
            }
            JobState::Failed(_) => {
                job.shown = true;
                true
            }
            JobState::Done => job.shown && job.ended.is_some_and(|e| now - e < DONE_MS),
        };
        if !visible {
            continue;
        }
        let (state, error) = match job.state {
            JobState::Queued => ("queued", None),
            JobState::Running => ("running", None),
            JobState::Done => ("done", None),
            JobState::Failed(e) => ("failed", Some(e)),
        };
        let running = job.state == JobState::Running;
        let total = job.files.len();
        // Overall share: finished files plus the current one's progress. No progress, no percent.
        let share = job.progress.filter(|_| running && total > 0).map(|p| ((job.done as f64 + p) / total as f64).clamp(0.0, 0.99));
        views.push(JobView {
            id: job.id,
            clip_id: job.clip_id,
            format: job.format,
            state,
            position,
            done: job.done,
            total,
            folder: job.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            error,
            percent: share.map(|p| (p * 100.0) as u8),
            eta: share.zip(job.started).and_then(|(p, started)| eta(now - started, p)),
            cancellable: running && !job.files.is_empty() && job.files.iter().all(|f| is_video(f)),
        });
    }
    views
}

/// Drops done jobs the panel no longer shows: never shown, or finished `DONE_MS` ago.
pub fn prune(jobs: &mut Vec<Job>, now: i64) {
    jobs.retain(|j| j.state != JobState::Done || (j.shown && j.ended.is_some_and(|e| now - e < DONE_MS)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::image::tests::tiny_png;
    use std::sync::atomic::AtomicBool;

    /// macOS with every engine installed.
    const ALL: Caps = Caps { heic: true, ffmpeg: true, rhwp: true, libreoffice: true };

    /// Formats and needs for `paths` under `caps`.
    fn doc_slices(paths: &[&str], caps: Caps) -> Vec<(Format, Option<Engine>)> {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        formats_for(&paths, caps).into_iter().map(|s| (s.format, s.needs)).collect()
    }

    fn doc_formats(paths: &[&str], caps: Caps) -> Vec<Format> {
        doc_slices(paths, caps).into_iter().map(|(f, _)| f).collect()
    }

    #[test]
    fn hwp_becomes_hwpx_and_hwpx_becomes_hwp() {
        use Format::*;
        assert_eq!(doc_formats(&["a.HWP"], ALL), [Pdf, Hwpx]);
        assert_eq!(doc_formats(&["a.hwpx"], ALL), [Pdf, Hwp]);
        assert_eq!(doc_formats(&["a.hwp", "b.hwpx"], ALL), [Pdf]);
        let missing = doc_slices(&["a.hwp"], Caps { rhwp: false, ..ALL });
        assert!(missing.iter().all(|(_, n)| *n == Some(Engine::Rhwp)));
    }

    #[test]
    fn text_documents_leave_out_their_own_format() {
        use Format::*;
        assert_eq!(doc_formats(&["a.docx"], ALL), [Pdf, Odt, Txt, Rtf, Html]);
        assert_eq!(doc_formats(&["a.doc"], ALL), [Pdf, Docx, Odt, Txt, Rtf, Html]);
        assert_eq!(doc_formats(&["a.rtf", "b.odt"], ALL), [Pdf, Docx, Txt, Html]);
        assert_eq!(doc_formats(&["a.txt"], ALL), [Pdf, Docx, Odt, Rtf, Html]);
        assert_eq!(doc_formats(&["a.htm"], ALL), [Pdf, Docx, Odt, Txt, Rtf]);
    }

    #[test]
    fn text_slices_need_libreoffice_when_missing() {
        // Windows included: plain text is no longer left out, only dimmed.
        let windows = Caps { heic: false, libreoffice: false, ..ALL };
        for src in ["a.doc", "a.txt", "a.html"] {
            let s = doc_slices(&[src], windows);
            assert!(!s.is_empty() && s.iter().all(|(_, n)| *n == Some(Engine::LibreOffice)), "{src}");
        }
        assert!(doc_slices(&["a.txt"], ALL).iter().all(|(_, n)| n.is_none()));
    }

    #[test]
    fn plain_and_rich_text_now_share_a_family() {
        use Format::*;
        assert_eq!(doc_formats(&["a.txt", "b.rtf"], ALL), [Pdf, Docx, Odt, Html]);
    }

    #[test]
    fn spreadsheets_and_presentations_need_libreoffice() {
        use Format::*;
        assert_eq!(doc_formats(&["a.xls"], ALL), [Pdf, Xlsx, Ods]);
        assert_eq!(doc_formats(&["a.ODS"], ALL), [Pdf, Xlsx]);
        assert_eq!(doc_formats(&["a.pptx", "b.ppt"], ALL), [Pdf, Odp]);
        let s = doc_slices(&["a.odp"], Caps { libreoffice: false, ..ALL });
        assert_eq!(s, [(Pdf, Some(Engine::LibreOffice)), (Pptx, Some(Engine::LibreOffice))]);
    }

    #[test]
    fn mixed_families_are_unsupported() {
        for mix in [["a.docx", "b.xlsx"], ["a.hwp", "b.docx"], ["a.txt", "b.xlsx"], ["a.pptx", "b.png"], ["a.pdf", "b.docx"]] {
            assert!(doc_slices(&mix, ALL).is_empty(), "{mix:?}");
        }
        assert!(doc_slices(&["a.png", "b.docx"], ALL).is_empty());
        assert!(doc_slices(&["a.docx", "b.mp4"], ALL).is_empty());
    }

    #[test]
    fn document_ids_are_their_extensions() {
        for f in [Format::Hwp, Format::Hwpx, Format::Docx, Format::Html, Format::Odp] {
            assert_eq!(serde_json::to_value(f).unwrap(), f.ext());
        }
        assert_eq!(serde_json::to_value(Engine::LibreOffice).unwrap(), "libreoffice");
        assert_eq!(serde_json::to_value(Engine::Rhwp).unwrap(), "rhwp");
        assert_eq!(serde_json::to_value(JobError::Runtime).unwrap(), "runtime");
    }

    #[test]
    fn documents_without_their_engine_are_engine_errors() {
        let d = tempfile::tempdir().unwrap();
        for name in ["a.hwp", "a.docx", "a.xlsx"] {
            let src = d.path().join(name);
            std::fs::write(&src, b"x").unwrap();
            assert_eq!(run_image(&src, "a", Format::Pdf, d.path(), d.path()), Err(JobError::Engine), "{name}");
        }
        // A target the family doesn't have.
        assert_eq!(run_image(&d.path().join("a.hwp"), "a", Format::Docx, d.path(), d.path()), Err(JobError::Engine));
    }

    /// Writes an executable shell script standing in for rhwp or soffice.
    #[cfg(unix)]
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// `run` for a document with these engines.
    fn run_doc(src: &Path, to: Format, dir: &Path, tmp: &Path, rhwp: Option<PathBuf>, soffice: Option<PathBuf>) -> Result<PathBuf, JobError> {
        let cancel = AtomicBool::new(false);
        let stem = src.file_stem().unwrap().to_string_lossy().into_owned();
        let mut ctx = Run { ffmpeg: None, quality: Quality::Normal, hw: None, cancel: &cancel, rhwp, soffice, lo_profile: tmp.join("lo"), progress: &mut |_| {} };
        run(src, &stem, to, dir, tmp, &mut ctx)
    }

    /// Source dir with `name` in it, destination dir, temp dir, bin dir.
    fn doc_dirs(name: &str) -> (PathBuf, [tempfile::TempDir; 4]) {
        let d = [(); 4].map(|()| tempfile::tempdir().unwrap());
        let src = d[0].path().join(name);
        std::fs::write(&src, b"doc").unwrap();
        (src, d)
    }

    #[cfg(unix)]
    #[test]
    fn rhwp_writes_the_output_and_a_failing_one_leaves_nothing() {
        let (src, [_s, dst, tmp, bin]) = doc_dirs("a.hwp");
        // Writes its first argument (the subcommand) into its last (the output).
        let ok = script(bin.path(), "rhwp", "for a; do out=$a; done\necho \"$1\" > \"$out\"");
        let out = run_doc(&src, Format::Pdf, dst.path(), tmp.path(), Some(ok), None).unwrap();
        assert_eq!((out.file_name().unwrap(), std::fs::read_to_string(&out).unwrap().as_str()), ("a.pdf".as_ref(), "export-pdf\n"));
        // Can't open it (encrypted, damaged): a partial file, then exit 1.
        let bad = script(bin.path(), "bad", "for a; do out=$a; done\necho partial > \"$out\"\nexit 1");
        assert_eq!(run_doc(&src, Format::Hwpx, dst.path(), tmp.path(), Some(bad), None), Err(JobError::Engine));
        assert_eq!(std::fs::read_dir(dst.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        // Exit 0 without an output.
        let silent = script(bin.path(), "silent", "exit 0");
        assert_eq!(run_doc(&src, Format::Hwpx, dst.path(), tmp.path(), Some(silent), None), Err(JobError::Engine));
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    /// A fake soffice: writes `<outdir>/<src stem>.pdf` (and a decoy beside it), then runs `then`.
    #[cfg(unix)]
    fn fake_soffice(bin: &Path, then: &str) -> PathBuf {
        let body = format!(
            "while [ $# -gt 1 ]; do [ \"$1\" = --outdir ] && dir=$2; shift; done\nname=$(basename \"$1\")\necho decoy > \"$dir/0.pdf\"\necho right > \"$dir/${{name%.*}}.pdf\"\n{then}"
        );
        script(bin, "soffice", &body)
    }

    #[cfg(unix)]
    #[test]
    fn soffice_output_is_found_by_the_dotted_stem() {
        let (src, [_s, dst, tmp, bin]) = doc_dirs("a.b.docx");
        let out = run_doc(&src, Format::Pdf, dst.path(), tmp.path(), None, Some(fake_soffice(bin.path(), "exit 0"))).unwrap();
        assert_eq!(out, dst.path().join("a.b.pdf"));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "right\n");
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_soffice_leaves_nothing() {
        let (src, [_s, dst, tmp, bin]) = doc_dirs("a.xlsx");
        assert_eq!(run_doc(&src, Format::Pdf, dst.path(), tmp.path(), None, Some(fake_soffice(bin.path(), "exit 3"))), Err(JobError::Engine));
        // Exit 0 but nothing under the expected name (LibreOffice's way of saying it couldn't load it).
        let silent = script(bin.path(), "silent", "exit 0");
        assert_eq!(run_doc(&src, Format::Pdf, dst.path(), tmp.path(), None, Some(silent)), Err(JobError::Engine));
        assert_eq!(std::fs::read_dir(dst.path()).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_hung_soffice_is_killed_at_the_timeout() {
        let (src, [_s, _d, tmp, bin]) = doc_dirs("a.pptx");
        let soffice = fake_soffice(bin.path(), "exec sleep 10");
        let out = tmp.path().join("a.pdf");
        let started = std::time::Instant::now();
        let r = office::convert_lo(&src, Format::Pdf, &out, tmp.path(), &soffice, &tmp.path().join("lo"), std::time::Duration::from_secs(1), &AtomicBool::new(false));
        assert_eq!(r, Err(JobError::Engine));
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_cancel_stops_a_running_document_engine() {
        let (src, [_s, dst, tmp, bin]) = doc_dirs("a.hwp");
        let rhwp = script(bin.path(), "rhwp", "for a; do out=$a; done\necho partial > \"$out\"\nexec sleep 10");
        let soffice = fake_soffice(bin.path(), "exec sleep 10");
        for to in [Format::Pdf, Format::Docx] {
            let src = if to == Format::Pdf { src.clone() } else { src.with_extension("odt") };
            std::fs::write(&src, b"doc").unwrap();
            let cancel = AtomicBool::new(false);
            let started = std::time::Instant::now();
            let mut ctx = Run {
                ffmpeg: None,
                quality: Quality::Normal,
                hw: None,
                cancel: &cancel,
                rhwp: Some(rhwp.clone()),
                soffice: Some(soffice.clone()),
                lo_profile: tmp.path().join("lo"),
                progress: &mut |_| {},
            };
            let r = std::thread::scope(|s| {
                s.spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    cancel.store(true, std::sync::atomic::Ordering::SeqCst);
                });
                run(&src, "a", to, dst.path(), tmp.path(), &mut ctx)
            });
            assert_eq!(r, Err(JobError::Cancelled), "{to:?}");
            assert!(started.elapsed() < std::time::Duration::from_secs(5));
            assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
            assert_eq!(std::fs::read_dir(dst.path()).unwrap().count(), 0);
        }
    }

    /// HWPX → PDF/HWP → HWPX/PDF with rhwp, then DOCX → PDF/ODT and ODT → DOCX with LibreOffice
    /// at its default path, and TXT → DOCX with LibreOffice too:
    /// `VEE_RHWP_DIR=… VEE_HWP_SAMPLE=…/a.hwpx cargo test --lib converts_documents -- --ignored --nocapture`
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore]
    fn converts_documents() {
        const PDF: &[u8] = b"%PDF";
        const ZIP: &[u8] = b"PK";
        const CFB: &[u8] = b"\xD0\xCF\x11\xE0";
        let (Ok(bin), Ok(sample)) = (std::env::var("VEE_RHWP_DIR"), std::env::var("VEE_HWP_SAMPLE")) else { return };
        let rhwp = Some(Path::new(&bin).join("rhwp"));
        let soffice = PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice");
        let (dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        // A profile path with a space and Hangul, like `Application Support` and Korean user names.
        let profile = tmp.path().join("lo 프로필");
        let cancel = AtomicBool::new(false);
        let mut ctx = Run {
            ffmpeg: None,
            quality: Quality::Normal,
            hw: None,
            cancel: &cancel,
            rhwp,
            soffice: Some(soffice),
            lo_profile: profile.clone(),
            progress: &mut |_| {},
        };
        let mut convert = |src: &Path, to: Format, magic: &[u8]| {
            let started = std::time::Instant::now();
            let stem = src.file_stem().unwrap().to_string_lossy().into_owned();
            let out = run(src, &stem, to, dir.path(), tmp.path(), &mut ctx).unwrap_or_else(|e| panic!("{} → {}: {e:?}", src.display(), to.id()));
            let bytes = std::fs::read(&out).unwrap();
            println!("{} → {}: {} bytes in {:?}", src.display(), out.display(), bytes.len(), started.elapsed());
            assert!(bytes.starts_with(magic), "{}", out.display());
            out
        };
        convert(Path::new(&sample), Format::Pdf, PDF);
        let hwp = convert(Path::new(&sample), Format::Hwp, CFB);
        convert(&hwp, Format::Hwpx, ZIP);
        convert(&hwp, Format::Pdf, PDF);
        let txt = dir.path().join("note.txt");
        std::fs::write(&txt, "Vee 문서 변환 테스트\nSecond line.\n").unwrap();
        let docx = convert(&txt, Format::Docx, ZIP);
        convert(&docx, Format::Pdf, PDF);
        let odt = convert(&docx, Format::Odt, ZIP);
        convert(&odt, Format::Docx, ZIP);
        assert!(profile.is_dir(), "LibreOffice used its own profile");
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().filter(|e| e.as_ref().unwrap().path() != profile).count(), 0);
    }

    /// Every TXT / HTML / RTF / DOCX slice with LibreOffice at its default path, checking each
    /// output is non-empty: `cargo test --lib converts_text_with_libreoffice -- --ignored --nocapture`
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore]
    fn converts_text_with_libreoffice() {
        let soffice = PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice");
        assert!(soffice.is_file(), "LibreOffice is not installed");
        let (dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let cancel = AtomicBool::new(false);
        let mut ctx = Run {
            ffmpeg: None,
            quality: Quality::Normal,
            hw: None,
            cancel: &cancel,
            rhwp: None,
            soffice: Some(soffice),
            lo_profile: tmp.path().join("lo-profile"),
            progress: &mut |_| {},
        };
        let txt = dir.path().join("note.txt");
        std::fs::write(&txt, "Vee 문서 변환 테스트\nSecond line.\n").unwrap();
        let html = dir.path().join("page.html");
        std::fs::write(&html, "<html><body><h1>Vee 변환</h1><p>Second line.</p></body></html>").unwrap();
        let mut sources = vec![txt, html];
        let rtf = run(&sources[0], "from-txt", Format::Rtf, dir.path(), tmp.path(), &mut ctx).unwrap();
        let docx = run(&sources[0], "from-txt", Format::Docx, dir.path(), tmp.path(), &mut ctx).unwrap();
        sources.extend([rtf, docx]);
        for src in &sources {
            for slice in formats_for(std::slice::from_ref(src), Caps { heic: true, ffmpeg: false, rhwp: false, libreoffice: true }) {
                let stem = format!("{}-to", src.file_name().unwrap().to_string_lossy());
                let out = run(src, &stem, slice.format, dir.path(), tmp.path(), &mut ctx)
                    .unwrap_or_else(|e| panic!("{} → {}: {e:?}", src.display(), slice.format.id()));
                let len = std::fs::metadata(&out).unwrap().len();
                println!("{} → {}: {len} bytes", src.display(), out.display());
                assert!(len > 0, "{}", out.display());
            }
        }
    }

    /// `run` for an image source: no FFmpeg, never cancelled.
    fn run_image(src: &Path, stem: &str, to: Format, dir: &Path, tmp: &Path) -> Result<PathBuf, JobError> {
        let cancel = AtomicBool::new(false);
        run(src, stem, to, dir, tmp, &mut Run { ffmpeg: None, quality: Quality::Normal, hw: None, cancel: &cancel, rhwp: None, soffice: None, lo_profile: PathBuf::new(), progress: &mut |_| {} })
    }

    #[test]
    fn typescript_files_are_not_videos() {
        assert!(formats_for(&[PathBuf::from("a.ts")], ALL).is_empty());
        assert!(is_video(Path::new("a.MTS")) && is_video(Path::new("a.m2ts")));
    }

    /// Image-rule formats, with FFmpeg missing (image slices never need it).
    fn formats(paths: &[PathBuf], heic: bool) -> Vec<Format> {
        let s = formats_for(paths, Caps { heic, ffmpeg: false, ..ALL });
        assert!(s.iter().all(|s| s.needs.is_none()));
        s.iter().map(|s| s.format).collect()
    }

    #[test]
    fn video_sources_get_video_slices_without_their_own_container() {
        let s = formats_for(&[PathBuf::from("a.MOV")], ALL);
        let f: Vec<Format> = s.iter().map(|s| s.format).collect();
        assert_eq!(f, vec![Format::Mp4, Format::Webm, Format::Gif, Format::Mp3, Format::Mp4720]);
        assert!(s.iter().all(|s| s.needs.is_none()));
    }

    #[test]
    fn mp4_keeps_720p() {
        let f: Vec<Format> =
            formats_for(&[PathBuf::from("a.m4v")], ALL).iter().map(|s| s.format).collect();
        assert!(!f.contains(&Format::Mp4) && f.contains(&Format::Mp4720));
    }

    #[test]
    fn video_slices_need_ffmpeg_when_missing() {
        let s = formats_for(&[PathBuf::from("a.mkv")], Caps { ffmpeg: false, ..ALL });
        assert!(!s.is_empty() && s.iter().all(|s| s.needs == Some(Engine::Ffmpeg)));
    }

    #[test]
    fn images_and_videos_together_are_unsupported() {
        assert!(formats_for(&[PathBuf::from("a.png"), PathBuf::from("b.mp4")], ALL).is_empty());
    }

    #[test]
    fn format_ids_are_unique_and_720p_writes_mp4() {
        assert_eq!(Format::Mp4720.id(), "720p");
        assert_eq!(Format::Mp4720.ext(), "mp4");
    }

    #[test]
    fn formats_leave_out_the_source_format() {
        let f = formats(&[PathBuf::from("/a/shot.PNG")], true);
        assert_eq!(f, vec![Format::Jpg, Format::Webp, Format::Heic, Format::Tiff, Format::Gif, Format::Bmp, Format::Pdf]);
    }

    #[test]
    fn heic_target_only_when_supported() {
        assert!(!formats(&[PathBuf::from("a.png")], false).contains(&Format::Heic));
    }

    #[test]
    fn heic_sources_need_heic_support() {
        assert!(formats(&[PathBuf::from("a.HEIC")], false).is_empty());
        assert!(formats(&[PathBuf::from("a.png"), PathBuf::from("b.heif")], false).is_empty());
        assert!(formats(&[PathBuf::from("a.heic")], true).contains(&Format::Png));
    }

    #[test]
    fn mixed_images_drop_every_source_format() {
        let f = formats(&[PathBuf::from("a.jpeg"), PathBuf::from("b.tif")], true);
        assert!(!f.contains(&Format::Jpg) && !f.contains(&Format::Tiff));
        assert!(f.contains(&Format::Png));
    }

    #[test]
    fn any_non_image_means_no_formats() {
        assert!(formats(&[PathBuf::from("a.png"), PathBuf::from("b.docx")], true).is_empty());
        assert!(formats(&[PathBuf::from("noext")], true).is_empty());
        assert!(formats(&[], true).is_empty());
    }

    #[test]
    fn unique_path_numbers_past_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unique_path(dir.path(), "a", "png"), dir.path().join("a.png"));
        std::fs::write(dir.path().join("a.png"), b"1").unwrap();
        std::fs::write(dir.path().join("a (1).png"), b"2").unwrap();
        assert_eq!(unique_path(dir.path(), "a", "png"), dir.path().join("a (2).png"));
        assert_eq!(std::fs::read(dir.path().join("a.png")).unwrap(), b"1");
    }

    #[test]
    fn run_moves_the_output_and_never_touches_the_source() {
        let (src_dir, dst, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = src_dir.path().join("p.png");
        std::fs::write(&src, tiny_png()).unwrap();
        std::fs::write(dst.path().join("p.jpg"), b"old").unwrap();
        let out = run_image(&src, "p", Format::Jpg, dst.path(), tmp.path()).unwrap();
        assert_eq!(out, dst.path().join("p (1).jpg"));
        assert_eq!(std::fs::read(dst.path().join("p.jpg")).unwrap(), b"old");
        assert_eq!(std::fs::read(&src).unwrap(), tiny_png());
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn run_reports_a_missing_source() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(run_image(&d.path().join("gone.png"), "gone", Format::Jpg, d.path(), d.path()), Err(JobError::Missing));
    }

    #[test]
    fn run_reports_an_unwritable_folder_and_cleans_up() {
        let (src_dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = src_dir.path().join("p.png");
        std::fs::write(&src, tiny_png()).unwrap();
        let missing_dir = src_dir.path().join("no/such/dir");
        assert_eq!(run_image(&src, "p", Format::Jpg, &missing_dir, tmp.path()), Err(JobError::Write));
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    fn job(id: u64, state: JobState, since: i64) -> Job {
        Job { state, ..Job::new(id, 1, Format::Png, vec![PathBuf::from("/tmp/out/a.jpg")], PathBuf::from("/tmp/out"), since) }
    }

    #[test]
    fn running_videos_show_overall_percent_eta_and_can_cancel() {
        let mut v = job(1, JobState::Running, 0);
        v.files = vec![PathBuf::from("/a/1.mov"), PathBuf::from("/a/2.mov")];
        v.done = 1;
        v.progress = Some(0.5);
        v.started = Some(0);
        let mut jobs = vec![v, job(2, JobState::Running, 0)];
        let views = visible_jobs(&mut jobs, 10_000);
        assert_eq!((views[0].percent, views[0].eta, views[0].cancellable), (Some(75), Some(3), true));
        // An image job: no progress, so the card falls back to done/total; never cancellable.
        assert_eq!((views[1].percent, views[1].eta, views[1].cancellable), (None, None, false));
        // Still clamped below 100 until the file is moved into place.
        jobs[0].progress = Some(1.0);
        assert_eq!(visible_jobs(&mut jobs, 10_000)[0].percent, Some(99));
        // Unknown length: no percent, no eta.
        jobs[0].progress = None;
        let v = &visible_jobs(&mut jobs, 10_000)[0];
        assert_eq!((v.percent, v.eta), (None, None));
        // Under 3 %: percent but no eta yet.
        jobs[0].done = 0;
        jobs[0].progress = Some(0.04);
        let v = &visible_jobs(&mut jobs, 10_000)[0];
        assert_eq!((v.percent, v.eta), (Some(2), None));
    }

    #[test]
    fn quick_jobs_never_show() {
        let mut jobs = vec![job(1, JobState::Running, 0)];
        assert!(visible_jobs(&mut jobs, 500).is_empty());
        jobs[0].state = JobState::Done;
        jobs[0].ended = Some(600);
        assert!(visible_jobs(&mut jobs, 700).is_empty());
    }

    #[test]
    fn slow_jobs_show_then_done_lingers_two_seconds() {
        let mut jobs = vec![job(1, JobState::Running, 0)];
        assert_eq!(visible_jobs(&mut jobs, 1000)[0].state, "running");
        jobs[0].state = JobState::Done;
        jobs[0].ended = Some(3000);
        assert_eq!(visible_jobs(&mut jobs, 4000)[0].state, "done");
        assert!(visible_jobs(&mut jobs, 5000).is_empty());
    }

    #[test]
    fn failed_jobs_show_at_once_and_queue_positions_count_from_one() {
        let mut jobs =
            vec![job(1, JobState::Failed(JobError::Engine), 0), job(2, JobState::Queued, 0), job(3, JobState::Queued, 0)];
        let v = visible_jobs(&mut jobs, 1500);
        assert_eq!(v[0].error, Some(JobError::Engine));
        assert_eq!((v[1].position, v[2].position), (1, 2));
        assert_eq!((v[0].folder.as_str(), v[0].total), ("out", 1));
    }

    #[test]
    fn prune_drops_stale_and_unseen_done_jobs() {
        let mut stale = job(1, JobState::Done, 0);
        stale.shown = true;
        stale.ended = Some(1000);
        let mut fresh = job(2, JobState::Done, 0);
        fresh.shown = true;
        fresh.ended = Some(2500);
        let mut unseen = job(3, JobState::Done, 0);
        unseen.ended = Some(2900);
        let mut jobs = vec![stale, fresh, unseen, job(4, JobState::Queued, 0), job(5, JobState::Failed(JobError::Write), 0)];
        prune(&mut jobs, 3000);
        assert_eq!(jobs.iter().map(|j| j.id).collect::<Vec<_>>(), vec![2, 4, 5]);
    }
}
