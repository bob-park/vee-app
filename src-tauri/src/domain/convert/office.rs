//! Document conversion: LibreOffice (`soffice --headless`) and macOS `textutil`.

use super::process::{command, exec};
use super::{Format, JobError, unique_path};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// LibreOffice is killed after this long; a hung one would stall the single worker.
pub const OFFICE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// `textutil` is killed after this long.
pub const TEXTUTIL_TIMEOUT: Duration = Duration::from_secs(60);

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

/// `textutil` arguments converting `src` to `to`, writing `out`. `None` for a target it isn't used for.
pub fn textutil_args(src: &Path, to: Format, out: &Path) -> Option<Vec<OsString>> {
    let format = match to {
        Format::Txt => "txt",
        Format::Rtf => "rtf",
        Format::Html => "html",
        Format::Docx => "docx",
        _ => return None,
    };
    // `-noload`: HTML never fetches its linked images or CSS, so a local conversion stays offline.
    Some(vec!["-noload".into(), "-convert".into(), format.into(), "-output".into(), out.into(), src.into()])
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

/// Converts `src` to `to` with macOS `textutil`, writing `out`. The caller removes `out` on any error.
pub fn convert_textutil(src: &Path, to: Format, out: &Path, timeout: Duration, cancel: &AtomicBool) -> Result<(), JobError> {
    let args = textutil_args(src, to, out).ok_or(JobError::Engine)?;
    exec(command(Path::new("/usr/bin/textutil")).args(args), timeout, cancel, "textutil")?;
    if out.is_file() { Ok(()) } else { Err(JobError::Engine) }
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
        assert_eq!(f(Format::Txt), None);
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

    #[test]
    fn textutil_args_per_target() {
        let a = |to| textutil_args(Path::new("/i/a.rtf"), to, Path::new("/o/a.x")).map(strings);
        assert_eq!(a(Format::Txt).unwrap(), ["-noload", "-convert", "txt", "-output", "/o/a.x", "/i/a.rtf"]);
        for (to, v) in [(Format::Rtf, "rtf"), (Format::Html, "html"), (Format::Docx, "docx")] {
            assert_eq!(a(to).unwrap()[2], v);
        }
        assert_eq!(a(Format::Pdf), None);
    }
}
