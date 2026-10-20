//! Convert domain: which image formats a dragged card can become, and where the output goes.

mod model;

pub use model::{Format, TARGETS};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
