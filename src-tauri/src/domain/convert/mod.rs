//! Convert domain: which image formats a dragged card can become, and where the output goes.

mod image;
mod model;

use self::image::convert_image;
pub use model::{Format, JobError, TARGETS};

use std::path::{Path, PathBuf};

/// Targets every source can become, in `TARGETS` order, minus each source's own format.
/// Empty when `paths` is empty or any path is not a supported image. `heic` says whether HEIC output is available.
pub fn formats_for(paths: &[PathBuf], heic: bool) -> Vec<Format> {
    let sources: Option<Vec<Format>> =
        paths.iter().map(|p| p.extension().and_then(|e| e.to_str()).and_then(Format::from_ext)).collect();
    match sources {
        Some(sources) if !sources.is_empty() => {
            TARGETS.into_iter().filter(|f| (heic || *f != Format::Heic) && !sources.contains(f)).collect()
        }
        _ => Vec::new(),
    }
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
/// The source is never touched and the temp file never outlives the call.
pub fn run(src: &Path, stem: &str, to: Format, dir: &Path, tmp: &Path) -> Result<PathBuf, JobError> {
    if !src.is_file() {
        return Err(JobError::Missing);
    }
    let staged = unique_path(tmp, stem, to.ext());
    let result = convert_image(src, to, &staged).and_then(|()| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::image::tests::tiny_png;

    #[test]
    fn formats_leave_out_the_source_format() {
        let f = formats_for(&[PathBuf::from("/a/shot.PNG")], true);
        assert_eq!(f, vec![Format::Jpg, Format::Webp, Format::Heic, Format::Tiff, Format::Gif, Format::Bmp, Format::Pdf]);
    }

    #[test]
    fn heic_target_only_when_supported() {
        assert!(!formats_for(&[PathBuf::from("a.png")], false).contains(&Format::Heic));
    }

    #[test]
    fn mixed_images_drop_every_source_format() {
        let f = formats_for(&[PathBuf::from("a.jpeg"), PathBuf::from("b.tif")], true);
        assert!(!f.contains(&Format::Jpg) && !f.contains(&Format::Tiff));
        assert!(f.contains(&Format::Png));
    }

    #[test]
    fn any_non_image_means_no_formats() {
        assert!(formats_for(&[PathBuf::from("a.png"), PathBuf::from("b.docx")], true).is_empty());
        assert!(formats_for(&[PathBuf::from("noext")], true).is_empty());
        assert!(formats_for(&[], true).is_empty());
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
        let out = run(&src, "p", Format::Jpg, dst.path(), tmp.path()).unwrap();
        assert_eq!(out, dst.path().join("p (1).jpg"));
        assert_eq!(std::fs::read(dst.path().join("p.jpg")).unwrap(), b"old");
        assert_eq!(std::fs::read(&src).unwrap(), tiny_png());
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn run_reports_a_missing_source() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(run(&d.path().join("gone.png"), "gone", Format::Jpg, d.path(), d.path()), Err(JobError::Missing));
    }

    #[test]
    fn run_reports_an_unwritable_folder_and_cleans_up() {
        let (src_dir, tmp) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = src_dir.path().join("p.png");
        std::fs::write(&src, tiny_png()).unwrap();
        let missing_dir = src_dir.path().join("no/such/dir");
        assert_eq!(run(&src, "p", Format::Jpg, &missing_dir, tmp.path()), Err(JobError::Write));
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }
}
