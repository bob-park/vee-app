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

/// MS Korean fonts and their stand-ins on macOS, which ships none of them.
const MAC_FONT_PAIRS: [(&str, &str); 12] = [
    ("맑은 고딕", "Apple SD Gothic Neo"),
    ("Malgun Gothic", "Apple SD Gothic Neo"),
    ("굴림", "Apple SD Gothic Neo"),
    ("Gulim", "Apple SD Gothic Neo"),
    ("새굴림", "Apple SD Gothic Neo"),
    ("New Gulim", "Apple SD Gothic Neo"),
    ("돋움", "Apple SD Gothic Neo"),
    ("Dotum", "Apple SD Gothic Neo"),
    ("바탕", "AppleMyungjo"),
    ("Batang", "AppleMyungjo"),
    ("궁서", "AppleMyungjo"),
    ("Gungsuh", "AppleMyungjo"),
];

/// The profile's `registrymodifications.xcu`: Korean as the Asian-text language (not the system
/// locale, which sets paper size and number formats), and on macOS a font replacement
/// table applied to output too (`Always`), so Korean documents keep their metrics roughly.
pub fn registry_xcu(macos: bool) -> String {
    let prop = |name: &str, value: &str| format!(r#"<prop oor:name="{name}" oor:op="fuse"><value>{value}</value></prop>"#);
    let item = |path: &str, body: String| format!(r#"<item oor:path="{path}">{body}</item>"#);
    let mut items = vec![
        item("/org.openoffice.Office.Linguistic/General", prop("DefaultLocale_CJK", "ko-KR")),
        item("/org.openoffice.Office.Common/I18N/CJK", prop("CJKFont", "true")),
    ];
    if macos {
        items.push(item("/org.openoffice.Office.Common/Font/Substitution", prop("Replacement", "true")));
        for (i, (from, to)) in MAC_FONT_PAIRS.iter().enumerate() {
            let pair =
                [prop("Always", "true"), prop("OnScreenOnly", "false"), prop("ReplaceFont", from), prop("SubstituteFont", to)].concat();
            items.push(item(
                "/org.openoffice.Office.Common/Font/Substitution/FontPairs",
                format!(r#"<node oor:name="_{i}" oor:op="replace">{pair}</node>"#),
            ));
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<oor:items xmlns:oor=\"http://openoffice.org/2001/registry\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n{}\n</oor:items>\n",
        items.join("\n")
    )
}

/// A fontconfig file for headless `soffice` on macOS. Headless LibreOffice finds fonts through
/// fontconfig, whose bundled setup lists only LibreOffice's own fonts, so without this it sees no
/// system font and drops every Hangul glyph.
pub fn fonts_conf(soffice: &Path, cache: &Path) -> String {
    // `<app>/Contents/MacOS/soffice` → `<app>/Contents/Resources/fonts/truetype`.
    let bundled = soffice.parent().and_then(Path::parent).unwrap_or(Path::new("")).join("Resources/fonts/truetype");
    let (bundled, cache) = (bundled.display(), cache.display());
    format!(
        r#"<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig>
  <dir>/System/Library/Fonts</dir>
  <dir>/Library/Fonts</dir>
  <dir>~/Library/Fonts</dir>
  <dir>{bundled}</dir>
  <cachedir>{cache}</cachedir>
  <include ignore_missing="yes">{bundled}/fc_local.conf</include>
</fontconfig>
"#
    )
}

/// Writes the profile's settings, and on macOS its `fonts.conf`. LibreOffice rewrites the
/// `.xcu` on exit, so this runs before every conversion rather than once.
pub fn write_profile(profile: &Path, soffice: &Path, macos: bool) -> std::io::Result<()> {
    let user = profile.join("user");
    std::fs::create_dir_all(&user)?;
    std::fs::write(user.join("registrymodifications.xcu"), registry_xcu(macos))?;
    if macos {
        std::fs::write(profile.join("fonts.conf"), fonts_conf(soffice, &profile.join("fc-cache")))?;
    }
    Ok(())
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
    let macos = cfg!(target_os = "macos");
    if let Err(e) = write_profile(profile, soffice, macos) {
        log::warn!("convert: LibreOffice profile settings not written: {e}");
    }
    let mut cmd = command(soffice);
    if macos {
        cmd.env("FONTCONFIG_FILE", profile.join("fonts.conf"));
    }
    let result = exec(cmd.args(args), timeout, cancel, "soffice").and_then(|()| {
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
            [
                "--headless",
                "--norestore",
                "-env:UserInstallation=file:///p/lo",
                "--convert-to",
                "docx:MS Word 2007 XML",
                "--outdir",
                "/t/o",
                "/i/a.docx"
            ]
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
    fn registry_sets_korean_and_replaces_ms_korean_fonts_on_macos() {
        let x = registry_xcu(true);
        assert!(x.contains("ko-KR"));
        for f in ["Malgun Gothic", "맑은 고딕", "Gulim", "굴림", "Dotum", "돋움", "Batang", "바탕", "Gungsuh", "궁서"] {
            assert!(x.contains(&format!("<value>{f}</value>")), "{f}");
        }
    }

    #[test]
    fn registry_leaves_the_system_locale_alone() {
        // It picks the default paper size and number formats; ko-KR would turn Letter into A4 for everyone.
        assert!(!registry_xcu(true).contains("ooSetupSystemLocale"));
        assert!(!registry_xcu(false).contains("ooSetupSystemLocale"));
    }

    #[test]
    fn windows_registry_sets_korean_without_font_pairs() {
        let x = registry_xcu(false);
        assert!(x.contains("ko-KR") && !x.contains("FontPairs"));
    }

    #[test]
    fn fonts_conf_adds_the_mac_font_dirs_and_libreoffice_aliases() {
        let x = fonts_conf(Path::new("/Applications/LibreOffice.app/Contents/MacOS/soffice"), Path::new("/p/fc-cache"));
        for line in [
            "<dir>/System/Library/Fonts</dir>",
            "<dir>/Library/Fonts</dir>",
            "<dir>~/Library/Fonts</dir>",
            "<dir>/Applications/LibreOffice.app/Contents/Resources/fonts/truetype</dir>",
            "<cachedir>/p/fc-cache</cachedir>",
            "<include ignore_missing=\"yes\">/Applications/LibreOffice.app/Contents/Resources/fonts/truetype/fc_local.conf</include>",
        ] {
            assert!(x.contains(line), "{line}");
        }
    }

    #[test]
    fn write_profile_overwrites_what_libreoffice_left() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("user/registrymodifications.xcu");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "old").unwrap();
        let soffice = Path::new("/Applications/LibreOffice.app/Contents/MacOS/soffice");
        write_profile(dir.path(), soffice, true).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), registry_xcu(true));
        let conf = std::fs::read_to_string(dir.path().join("fonts.conf")).unwrap();
        assert_eq!(conf, fonts_conf(soffice, &dir.path().join("fc-cache")));
    }

    #[test]
    fn file_urls_for_both_platforms() {
        assert_eq!(
            file_url(Path::new("/Users/me/Library/Application Support/vee/lo-profile")),
            "file:///Users/me/Library/Application%20Support/vee/lo-profile"
        );
        assert_eq!(
            file_url(Path::new(r"C:\Users\박\AppData\Roaming\vee\lo-profile")),
            "file:///C:/Users/%EB%B0%95/AppData/Roaming/vee/lo-profile"
        );
    }

    #[test]
    fn lo_output_keeps_every_dot_of_the_stem() {
        assert_eq!(lo_output(Path::new("/t/o"), Path::new("/i/a.b.docx"), Format::Pdf), Path::new("/t/o/a.b.pdf"));
        assert_eq!(lo_output(Path::new("/t/o"), Path::new("/i/s.XLS"), Format::Xlsx), Path::new("/t/o/s.xlsx"));
    }
}
