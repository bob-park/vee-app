//! Convert domain: which image or video formats a dragged card can become, and where the output goes.

mod image;
mod model;
mod video;

use self::image::convert_image;
pub use model::{Caps, Engine, Format, Job, JobError, JobState, JobView, Quality, Slice, DONE_MS, SHOW_AFTER_MS};
pub use video::Run;
use model::{IMAGE_TARGETS, VIDEO_TARGETS};

use std::path::{Path, PathBuf};

/// Slices every source can become, in target order, minus each source's own format.
/// All images or all videos; empty when `paths` is empty, mixes the two, or holds anything else.
/// Video slices need FFmpeg while `caps.ffmpeg` is false; image slices never need anything.
pub fn formats_for(paths: &[PathBuf], caps: Caps) -> Vec<Slice> {
    if !paths.is_empty() && paths.iter().all(|p| is_video(p)) {
        let own: Vec<Format> = paths.iter().filter_map(|p| ext_of(p).and_then(video_container)).collect();
        let needs = (!caps.ffmpeg).then_some(Engine::Ffmpeg);
        return VIDEO_TARGETS.into_iter().filter(|f| !own.contains(f)).map(|format| Slice { format, needs }).collect();
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
        views.push(JobView {
            id: job.id,
            clip_id: job.clip_id,
            format: job.format,
            state,
            position,
            done: job.done,
            total: job.files.len(),
            folder: job.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            error,
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

    /// `run` for an image source: no FFmpeg, never cancelled.
    fn run_image(src: &Path, stem: &str, to: Format, dir: &Path, tmp: &Path) -> Result<PathBuf, JobError> {
        let cancel = AtomicBool::new(false);
        run(src, stem, to, dir, tmp, &mut Run { ffmpeg: None, quality: Quality::Normal, cancel: &cancel, progress: &mut |_| {} })
    }

    #[test]
    fn typescript_files_are_not_videos() {
        assert!(formats_for(&[PathBuf::from("a.ts")], Caps { heic: true, ffmpeg: true }).is_empty());
        assert!(is_video(Path::new("a.MTS")) && is_video(Path::new("a.m2ts")));
    }

    /// Image-rule formats, with FFmpeg missing (image slices never need it).
    fn formats(paths: &[PathBuf], heic: bool) -> Vec<Format> {
        let s = formats_for(paths, Caps { heic, ffmpeg: false });
        assert!(s.iter().all(|s| s.needs.is_none()));
        s.iter().map(|s| s.format).collect()
    }

    #[test]
    fn video_sources_get_video_slices_without_their_own_container() {
        let s = formats_for(&[PathBuf::from("a.MOV")], Caps { heic: true, ffmpeg: true });
        let f: Vec<Format> = s.iter().map(|s| s.format).collect();
        assert_eq!(f, vec![Format::Mp4, Format::Webm, Format::Gif, Format::Mp3, Format::Mp4720]);
        assert!(s.iter().all(|s| s.needs.is_none()));
    }

    #[test]
    fn mp4_keeps_720p() {
        let f: Vec<Format> =
            formats_for(&[PathBuf::from("a.m4v")], Caps { heic: true, ffmpeg: true }).iter().map(|s| s.format).collect();
        assert!(!f.contains(&Format::Mp4) && f.contains(&Format::Mp4720));
    }

    #[test]
    fn video_slices_need_ffmpeg_when_missing() {
        let s = formats_for(&[PathBuf::from("a.mkv")], Caps { heic: true, ffmpeg: false });
        assert!(!s.is_empty() && s.iter().all(|s| s.needs == Some(Engine::Ffmpeg)));
    }

    #[test]
    fn images_and_videos_together_are_unsupported() {
        assert!(formats_for(&[PathBuf::from("a.png"), PathBuf::from("b.mp4")], Caps { heic: true, ffmpeg: true }).is_empty());
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
        Job {
            id,
            clip_id: 1,
            format: Format::Png,
            files: vec![PathBuf::from("/tmp/out/a.jpg")],
            dir: PathBuf::from("/tmp/out"),
            done: 0,
            state,
            since,
            ended: None,
            shown: false,
        }
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
