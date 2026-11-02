//! PDF to Markdown: page text through PDFium, run in a one-shot child (`vee-app --pdf-worker`)
//! so a PDF that crashes PDFium never takes the app down. The app process never loads PDFium.

use super::JobError;
use super::process::{command, exec};
use pdfium_render::prelude::Pdfium;
use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// The CLI flag that starts the worker instead of the app: `vee-app --pdf-worker <lib> <src> <out>`.
pub const FLAG: &str = "--pdf-worker";
/// The PDFium library's file name, as `scripts/fetch-pdfium.mjs` fetches and the bundle installs it.
pub const LIB: &str = if cfg!(windows) { "pdfium.dll" } else { "libpdfium.dylib" };
/// The worker is killed after this long; a hung one would stall the single convert worker.
const PDF_TIMEOUT: Duration = Duration::from_secs(120);

/// Converts `src` into Markdown at `out` in a `--pdf-worker` child of `exe`, loading PDFium from `lib`.
/// The caller removes `out` on any error.
pub fn convert(src: &Path, out: &Path, exe: &Path, lib: &Path, cancel: &AtomicBool) -> Result<(), JobError> {
    exec(command(exe).arg(FLAG).arg(lib).arg(src).arg(out), PDF_TIMEOUT, cancel, "pdf worker")?;
    if out.is_file() { Ok(()) } else { Err(JobError::Engine) }
}

/// The worker's body: `args` is `[lib, src, out]`. Writes `out` only on success; the reason for a
/// failure goes to stderr.
pub fn worker_main(args: &[OsString]) -> ExitCode {
    let [lib, src, out] = args else {
        eprintln!("usage: {FLAG} <lib> <src> <out>");
        return ExitCode::FAILURE;
    };
    let written = extract(Path::new(lib), Path::new(src)).and_then(|md| std::fs::write(out, md).map_err(|e| e.to_string()));
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pdf worker: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `src` as Markdown (`to_markdown`), with PDFium loaded from `lib`. PDFium binds once per process,
/// which the one-shot worker is. `Err` when PDFium can't load or open it, or when no page has text.
pub fn extract(lib: &Path, src: &Path) -> Result<String, String> {
    let bindings = Pdfium::bind_to_library(lib).map_err(|e| format!("load {}: {e}", lib.display()))?;
    extract_with(&Pdfium::new(bindings), src)
}

/// `src` as Markdown titled with its file stem. Password-protected PDFs fail to open.
fn extract_with(pdfium: &Pdfium, src: &Path) -> Result<String, String> {
    let doc = pdfium.load_pdf_from_file(src, None).map_err(|e| format!("open: {e}"))?;
    let pages = doc.pages().iter().map(|p| p.text().map(|t| t.all())).collect::<Result<Vec<_>, _>>().map_err(|e| format!("text: {e}"))?;
    let title = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    to_markdown(&title, &pages).ok_or_else(|| "no text".into())
}

/// `# title`, then `## Page N` and the page's text for every page, or `None` when no page has text
/// (a scanned PDF): an empty file would pass for a result. Line ends become `\n`, trailing spaces
/// go, and runs of blank lines shrink to one. An empty page keeps its heading so numbers match the PDF.
pub fn to_markdown(title: &str, pages: &[String]) -> Option<String> {
    let mut md = format!("# {title}\n");
    let mut any = false;
    for (i, page) in pages.iter().enumerate() {
        md.push_str(&format!("\n## Page {}\n", i + 1));
        let text = page.replace("\r\n", "\n").replace('\r', "\n");
        let mut body = String::new();
        let mut blank = false;
        for line in text.lines().map(str::trim_end) {
            if line.is_empty() {
                blank = !body.is_empty();
                continue;
            }
            if blank {
                body.push('\n');
                blank = false;
            }
            body.push_str(line);
            body.push('\n');
        }
        if !body.is_empty() {
            any = true;
            md.push('\n');
            md.push_str(&body);
        }
    }
    any.then_some(md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The PDFium `scripts/fetch-pdfium.mjs` put in the source tree, or `None` (test skipped).
    fn vendor_pdfium() -> Option<PathBuf> {
        let lib = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/pdfium").join(LIB);
        lib.is_file().then_some(lib).or_else(|| {
            eprintln!("skipped: {LIB} not fetched (node scripts/fetch-pdfium.mjs)");
            None
        })
    }

    fn testdata(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/domain/convert/testdata").join(name)
    }

    /// One test for everything that loads PDFium: it binds once per process.
    #[test]
    fn pdfium_extracts_hangul_byte_for_byte_and_rejects_non_pdf() {
        let Some(lib) = vendor_pdfium() else { return };
        let pdfium = Pdfium::new(Pdfium::bind_to_library(&lib).unwrap());
        assert_eq!(extract_with(&pdfium, &testdata("hangul.pdf")).unwrap(), include_str!("testdata/hangul.md"));
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("x.pdf");
        std::fs::write(&fake, b"not a pdf").unwrap();
        assert!(extract_with(&pdfium, &fake).is_err());
        assert!(extract_with(&pdfium, &dir.path().join("missing.pdf")).is_err());
    }

    #[test]
    fn worker_rejects_wrong_arguments() {
        assert_eq!(worker_main(&["only-one".into()]), ExitCode::FAILURE);
    }

    #[test]
    fn an_existing_md_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hangul.md"), "keep").unwrap();
        assert!(super::super::unique_path(dir.path(), "hangul", "md").ends_with("hangul (1).md"));
    }

    #[test]
    fn markdown_has_title_and_numbered_pages() {
        let md = to_markdown("보고서", &["첫 줄  \r\n둘째 줄".into(), "".into(), "끝".into()]).unwrap();
        assert_eq!(md, "# 보고서\n\n## Page 1\n\n첫 줄\n둘째 줄\n\n## Page 2\n\n## Page 3\n\n끝\n");
    }

    #[test]
    fn blank_runs_collapse_to_one() {
        let md = to_markdown("t", &["a\n\n\n\n b".into()]).unwrap();
        assert_eq!(md, "# t\n\n## Page 1\n\na\n\n b\n");
    }

    #[test]
    fn all_blank_pages_is_no_text() {
        assert_eq!(to_markdown("t", &[" \n".into(), "".into()]), None);
    }
}
