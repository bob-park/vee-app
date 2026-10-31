//! HWP and HWPX conversion through the rhwp CLI.

use super::process::{command, exec};
use super::{Format, JobError};
use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// rhwp is killed after this long; a hung one would stall the single worker.
pub const RHWP_TIMEOUT: Duration = Duration::from_secs(120);

/// rhwp's arguments converting `src` to `to`, writing `out`. `None` for a target rhwp doesn't write.
pub fn rhwp_args(src: &Path, to: Format, out: &Path) -> Option<Vec<OsString>> {
    let (src, out) = (src.as_os_str().to_owned(), out.as_os_str().to_owned());
    Some(match to {
        Format::Pdf => vec!["export-pdf".into(), src, "-o".into(), out],
        Format::Hwpx => vec!["export-hwpx".into(), src, out],
        Format::Hwp => vec!["convert".into(), src, out],
        _ => return None,
    })
}

/// Converts `src` to `to` with `rhwp`, writing `out`. The caller removes `out` on any error.
pub fn convert(src: &Path, to: Format, out: &Path, rhwp: &Path, timeout: Duration, cancel: &AtomicBool) -> Result<(), JobError> {
    let args = rhwp_args(src, to, out).ok_or(JobError::Engine)?;
    exec(command(rhwp).args(args), timeout, cancel, "rhwp")?;
    if out.is_file() { Ok(()) } else { Err(JobError::Engine) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(to: Format) -> Option<Vec<String>> {
        rhwp_args(Path::new("/i/a.hwp"), to, Path::new("/o/a.x")).map(|a| a.iter().map(|s| s.to_string_lossy().into_owned()).collect())
    }

    #[test]
    fn rhwp_args_per_target() {
        assert_eq!(args(Format::Pdf).unwrap(), ["export-pdf", "/i/a.hwp", "-o", "/o/a.x"]);
        assert_eq!(args(Format::Hwpx).unwrap(), ["export-hwpx", "/i/a.hwp", "/o/a.x"]);
        assert_eq!(args(Format::Hwp).unwrap(), ["convert", "/i/a.hwp", "/o/a.x"]);
        assert_eq!(args(Format::Docx), None);
    }
}
