//! The user's LibreOffice: found at its default install path, never downloaded.

use crate::domain::convert::{command, output};
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `soffice --version` is given up after this long.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

/// A LibreOffice that was found; mirrors TS `LibreOffice`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LibreOffice {
    pub version: String,
}

/// Where LibreOffice's `soffice` is installed, if it is.
pub fn soffice() -> Option<PathBuf> {
    candidates().into_iter().find(|p| p.is_file())
}

#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    vec![PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice")]
}

#[cfg(windows)]
fn candidates() -> Vec<PathBuf> {
    windows_candidates(std::env::var_os("ProgramFiles"), std::env::var_os("ProgramFiles(x86)"))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn candidates() -> Vec<PathBuf> {
    Vec::new()
}

/// `<Program Files>\LibreOffice\program\soffice.exe`, 64-bit first; unset variables are skipped.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn windows_candidates(program_files: Option<OsString>, program_files_x86: Option<OsString>) -> Vec<PathBuf> {
    [program_files, program_files_x86]
        .into_iter()
        .flatten()
        .map(|root| PathBuf::from(root).join("LibreOffice").join("program").join("soffice.exe"))
        .collect()
}

/// The version `soffice --version` prints, or `None` if it fails, hangs or prints something else.
pub fn libreoffice_version(soffice: &Path) -> Option<String> {
    parse_version(&output(command(soffice).arg("--version"), VERSION_TIMEOUT)?)
}

/// `LibreOffice 26.8.1.1 bf6ef…` (first line) → `26.8.1.1`.
fn parse_version(out: &str) -> Option<String> {
    let mut words = out.lines().next()?.split_whitespace();
    let version = (words.next()? == "LibreOffice").then(|| words.next()).flatten()?;
    let valid = version.starts_with(|c: char| c.is_ascii_digit()) && version.chars().all(|c| c.is_ascii_digit() || c == '.');
    valid.then(|| version.to_owned())
}

pub mod commands {
    use super::{LibreOffice, libreoffice_version, soffice};

    /// The installed LibreOffice and its version; `None` when it isn't found or won't say its version.
    /// Async so the up-to-10 s `soffice --version` never blocks the main thread.
    #[tauri::command(async)]
    pub fn get_libreoffice() -> Option<LibreOffice> {
        libreoffice_version(&soffice()?).map(|version| LibreOffice { version })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_first_line_only() {
        assert_eq!(parse_version("LibreOffice 26.8.1.1 bf6ef2662401c7d6dce671ab4deb26ee0af3a7c1\n").as_deref(), Some("26.8.1.1"));
        assert_eq!(parse_version("LibreOffice 7.6.4.1 e19e193f88cd6c0525a17fb7a176ed8e6a3e2aa1\r\n").as_deref(), Some("7.6.4.1"));
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("OpenOffice 4.1.15"), None);
        assert_eq!(parse_version("LibreOffice"), None);
        assert_eq!(parse_version("LibreOffice dev"), None);
        assert_eq!(parse_version("warning: something\nLibreOffice 26.8.1.1 x"), None);
    }

    #[test]
    fn windows_paths_64_bit_first_and_skip_unset_vars() {
        let pf = |s: &str| Some(OsString::from(s));
        let exe = |root: &str| PathBuf::from(root).join("LibreOffice").join("program").join("soffice.exe");
        assert_eq!(
            windows_candidates(pf(r"C:\Program Files"), pf(r"C:\Program Files (x86)")),
            vec![exe(r"C:\Program Files"), exe(r"C:\Program Files (x86)")]
        );
        assert_eq!(windows_candidates(None, pf(r"D:\PF86")), vec![exe(r"D:\PF86")]);
        assert!(windows_candidates(None, None).is_empty());
    }

    #[test]
    #[ignore = "needs LibreOffice at its default path"]
    fn finds_the_installed_libreoffice() {
        let found = commands::get_libreoffice();
        println!("LibreOffice: {found:?}");
        assert!(found.is_some_and(|lo| lo.version.split('.').count() >= 2));
    }
}
