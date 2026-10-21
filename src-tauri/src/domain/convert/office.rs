//! Document conversion through LibreOffice (`soffice --headless`).

use super::process::{command, exec};
use super::{Format, JobError, unique_path};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// LibreOffice is killed after this long; a hung one would stall the single worker.
pub const OFFICE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// The `--convert-to` value for `to`. `None` for a target LibreOffice isn't used for.
fn filter(to: Format) -> Option<&'static str> {
    Some(match to {
        Format::Pdf => "pdf",
        Format::Docx => "docx:MS Word 2007 XML",
        Format::Odt => "odt",
        Format::Xlsx => "xlsx:Calc MS Excel 2007 XML",
        Format::Ods => "ods",
        Format::Pptx => "pptx:Impress MS PowerPoint 2007 XML",
        Format::Odp => "odp",
        Format::Txt => "txt:Text (encoded):UTF8",
        Format::Rtf => "rtf:Rich Text Format",
        Format::Html => "html:XHTML Writer File:UTF8",
        _ => return None,
    })
}

/// `soffice` arguments converting `src` to `to` into `outdir`, with its own `profile` dir.
pub fn soffice_args(src: &Path, to: Format, outdir: &Path, profile: &Path) -> Option<Vec<OsString>> {
    let mut a: Vec<OsString> = ["--headless", "--norestore"].map(OsString::from).into();
    a.push(format!("-env:UserInstallation={}", file_url(profile)).into());
    a.extend(["--convert-to".into(), filter(to)?.into(), "--outdir".into(), outdir.into(), src.into()]);
    Some(a)
}

/// Where LibreOffice writes `src` converted to `to`: `<outdir>/<src stem>.<ext>` (`a.b.docx` → `a.b.pdf`).
pub fn lo_output(outdir: &Path, src: &Path, to: Format) -> PathBuf {
    let mut name = src.file_stem().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(to.ext());
    outdir.join(name)
}

/// An absolute path as a `file:///` URL, UTF-8 percent-encoded: `/a b` → `file:///a%20b`, `C:\a` → `file:///C:/a`.
pub fn file_url(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let mut url = String::from(if path.starts_with('/') { "file://" } else { "file:///" });
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// Converts `src` to `to` with LibreOffice into a fresh dir under `tmp`, then moves it to `out`.
/// That dir never outlives the call; the caller removes `out` on any error.
pub fn convert_lo(
    src: &Path,
    to: Format,
    out: &Path,
    tmp: &Path,
    soffice: &Path,
    profile: &Path,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<(), JobError> {
    // A dir of its own, so the expected name can't meet another file.
    let outdir = unique_path(tmp, "soffice", "out");
    let args = soffice_args(src, to, &outdir, profile).ok_or(JobError::Engine)?;
    std::fs::create_dir(&outdir).map_err(|_| JobError::Write)?;
    let result = exec(command(soffice).args(args), timeout, cancel, "soffice").and_then(|()| {
        // LibreOffice exits 0 even when it couldn't load the file, so the output is the real check.
        std::fs::rename(lo_output(&outdir, src, to), out).map_err(|e| {
            log::warn!("convert: soffice wrote no {}: {e}", to.ext());
            JobError::Engine
        })
    });
    let _ = std::fs::remove_dir_all(&outdir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(a: Vec<OsString>) -> Vec<String> {
        a.iter().map(|s| s.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn soffice_args_use_their_own_profile_and_filter() {
        let a = strings(soffice_args(Path::new("/i/a.docx"), Format::Docx, Path::new("/t/o"), Path::new("/p/lo")).unwrap());
        assert_eq!(
            a,
            ["--headless", "--norestore", "-env:UserInstallation=file:///p/lo", "--convert-to", "docx:MS Word 2007 XML", "--outdir", "/t/o", "/i/a.docx"]
        );
        let f = |to| soffice_args(Path::new("i"), to, Path::new("o"), Path::new("/p")).map(|a| strings(a)[4].clone());
        assert_eq!(f(Format::Xlsx).unwrap(), "xlsx:Calc MS Excel 2007 XML");
        assert_eq!(f(Format::Pptx).unwrap(), "pptx:Impress MS PowerPoint 2007 XML");
        for (to, v) in [(Format::Pdf, "pdf"), (Format::Odt, "odt"), (Format::Ods, "ods"), (Format::Odp, "odp")] {
            assert_eq!(f(to).unwrap(), v);
        }
        assert_eq!(f(Format::Txt).unwrap(), "txt:Text (encoded):UTF8");
        assert_eq!(f(Format::Rtf).unwrap(), "rtf:Rich Text Format");
        assert_eq!(f(Format::Html).unwrap(), "html:XHTML Writer File:UTF8");
        assert_eq!(f(Format::Png), None);
    }

    #[test]
    fn file_urls_for_both_platforms() {
        assert_eq!(file_url(Path::new("/Users/me/Library/Application Support/vee/lo-profile")), "file:///Users/me/Library/Application%20Support/vee/lo-profile");
        assert_eq!(file_url(Path::new(r"C:\Users\박\AppData\Roaming\vee\lo-profile")), "file:///C:/Users/%EB%B0%95/AppData/Roaming/vee/lo-profile");
    }

    #[test]
    fn lo_output_keeps_every_dot_of_the_stem() {
        assert_eq!(lo_output(Path::new("/t/o"), Path::new("/i/a.b.docx"), Format::Pdf), Path::new("/t/o/a.b.pdf"));
        assert_eq!(lo_output(Path::new("/t/o"), Path::new("/i/s.XLS"), Format::Xlsx), Path::new("/t/o/s.xlsx"));
    }
}
