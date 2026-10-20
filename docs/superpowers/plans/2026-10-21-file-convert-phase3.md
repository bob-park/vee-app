# Drag File Convert — Phase 3 (HWP + documents) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** HWP/HWPX files convert through the donut with rhwp (downloaded in Settings like FFmpeg), and office documents convert with the user's installed LibreOffice (or macOS `textutil` for plain formats), with the same card progress, toasts and settings patterns as phases 1–2.

**Architecture:** `app/engines.rs` is generalised from FFmpeg-only to a small table of downloadable engines (`ffmpeg`, `rhwp`) sharing one download/verify/install path; rhwp's assets are archives, so a one-step `tar -xf` unpack comes back for archive assets only. `domain/convert` gains document/HWP families, `Engine::{Rhwp, LibreOffice}` slice needs, and two runners (`hwp.rs`, `office.rs`) with timeouts. LibreOffice is detected, never downloaded.

**Tech Stack:** Rust (Tauri 2.12, `sha2`), system `curl` + `tar`, rhwp v0.8.7 CLI, LibreOffice `soffice --headless`, macOS `textutil`; React 19.

**Spec:** `docs/superpowers/specs/2026-10-20-file-convert-design.md` (phase ③). Phase 1/2 plans' Global Constraints still hold.

## Global Constraints

- Branch `feature/file-convert`. No dependency changes.
- **rhwp** v0.8.7 (`https://github.com/edwardkim/rhwp/releases/download/v0.8.7/`), version string `0.8.7`:
  - macOS arm64: `rhwp-v0.8.7-macos-aarch64.tar.gz` size `12941874` sha256 `6ed46aa63f6d10c447842999f73abc59cf9586c5645fefc5e763bd6a6d43662d` → contains `rhwp/rhwp`.
  - Windows x64: `rhwp-v0.8.7-windows-x86_64.zip` size `19796112` sha256 `f7ac326e98f595ae8fecd8da7a614ee9a37db77bdf938782a8e07c9da473ec6f` → contains `rhwp.exe` (find by name below the unpack dir).
  - Installed in `<app data>/engines/rhwp/` (`rhwp`/`rhwp.exe` + `meta.json`), same safety rules as FFmpeg (verify before touching dest, swap into place, recover, per-identifier temp dir). Unpack: `tar -xf <archive> -C <dir>` (macOS bsdtar reads .tar.gz; Windows 10 1803+ `tar.exe` reads .zip), CREATE_NO_WINDOW on Windows.
- rhwp commands (verified on macOS): `rhwp export-pdf <in> -o <out.pdf>`; `rhwp export-hwpx <in.hwp> <out.hwpx>`; `rhwp convert <in.hwpx> <out.hwp>`. Exit 0 = success.
- Windows rhwp needs the MSVC runtime (`VCRUNTIME140.dll`, `MSVCP140.dll`); a launch failing with exit code `0xC0000135` (`-1073741515`) maps to `JobError::Runtime` → toast text `t.convert.errors.runtime`.
- **LibreOffice** detection (no download): macOS `/Applications/LibreOffice.app/Contents/MacOS/soffice`; Windows `%ProgramFiles%\LibreOffice\program\soffice.exe`, then `%ProgramFiles(x86)%\…`. Version from `soffice --version` (first line, e.g. `LibreOffice 26.8.1.1 …` → `26.8.1.1`), cached until the Convert tab re-checks.
  - Run: `soffice --headless --norestore -env:UserInstallation=file:///<app data>/lo-profile --convert-to <filter> --outdir <tmp> <in>`; output is `<tmp>/<in stem>.<ext>`; then the usual move + numbering.
  - Filters: `pdf`, `docx` (`docx:"MS Word 2007 XML"`), `odt`, `xlsx` (`xlsx:"Calc MS Excel 2007 XML"`), `ods`, `pptx` (`pptx:"Impress MS PowerPoint 2007 XML"`), `odp`.
  - Timeout 5 min → kill → `Engine`. One LibreOffice job at a time (the single worker already ensures it).
- **textutil** (macOS only, always available): `textutil -convert <txt|rtf|html|docx> -output <out> <in>` for sources `docx doc rtf rtfd html htm txt odt`.
- Families and targets (donut order; the source's own format left out; mixed families in one card → unsupported):
  - HWP `hwp hwpx` → `pdf, hwpx, hwp` (`hwpx` only from `.hwp`, `hwp` only from `.hwpx`); needs `rhwp`.
  - Text documents `docx doc odt rtf` → `pdf, docx, odt` (needs `libreoffice`) then on macOS `txt, rtf, html` (textutil, never dimmed).
  - Plain text `txt html htm` (macOS only) → `docx, rtf, txt, html` (textutil). Windows: unsupported.
  - Spreadsheets `xlsx xls ods` → `pdf, xlsx, ods` (needs `libreoffice`).
  - Presentations `pptx ppt odp` → `pdf, pptx, odp` (needs `libreoffice`).
  - `Format` gains `Hwp, Hwpx, Docx, Odt, Rtf, Txt, Html, Xlsx, Ods, Pptx, Odp` (serde = ext; `Pdf` is shared with images).
- `Slice.needs`: `ffmpeg | rhwp | libreoffice`. Missing engine → dimmed with `t.convert.needsRhwp` / `t.convert.needsLibreOffice`.
- No progress for rhwp/LibreOffice/textutil → spinning ring (phase-2 UI). Not cancellable.
- Settings → Convert:
  - **rhwp** section reusing the FFmpeg section component (missing / downloading with % + MB + speed / installed with version, size, installed date; no location shown; error line).
  - **LibreOffice** row: found → ok badge + version; not found → button `t.settings.convert.libreoffice.install` opening `https://www.libreoffice.org/download/download-libreoffice/`. Re-checked when the Convert tab mounts (not on focus).
- Engine commands/events become generic (this branch is unmerged, so the phase-2 names are replaced, not kept): `get_engine_status(engine)`, `download_engine(engine)`, `cancel_engine_download(engine)`, `remove_engine(engine)` with `engine` = `"ffmpeg" | "rhwp"`; event `engines://status` payload `{ engine, status }`; `get_libreoffice() -> Option<{ version: String }>`.
- Strings only in i18n (copy in Task 5). Colours only in globals.css.

## Review Focus

- A `.hwp` that rhwp can't open (encrypted, damaged) → `Engine` toast, no file in the folder, temp cleaned. Test in Task 3 with a fake rhwp script exiting non-zero.
- LibreOffice already open by the user → our run uses its own profile and still converts; a LibreOffice that hangs → killed at 5 min. Test the timeout with a fake soffice script (sleep) and a shortened timeout constant in the test.
- LibreOffice writes `<stem>.<ext>` into `--outdir` — a source named `a.b.docx` gives `a.b.pdf`; the runner must look for that exact name, never "the only file in tmp". Test in Task 3.
- Removing FFmpeg must not touch rhwp's dir and vice versa; a download of one engine doesn't block the other's status. Test in Task 1.
- Windows-only `.txt` source → unsupported toast, not a dimmed donut. Test in Task 2.

---

### Task 1: Generic engine install (ffmpeg + rhwp)

**Files:** `src-tauri/src/app/engines.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/app/convert.rs` (callers), `src/domain/convert/apis/{convert.ts,convert.dto.ts}`, `src/domain/convert/queries/engines.tsx`, `src/app/settings/_components/ConvertSection.tsx` (call sites only).

**Interfaces:**
- `pub enum EngineId { Ffmpeg, Rhwp }` (serde lowercase, `parse(&str)`); `struct EngineSpec { id, version: &str, dir_name: &str, binaries: &[&str], assets: fn() -> &'static [Asset] }`; `Asset` gains `kind: AssetKind { Raw { name }, Archive }` (Archive → `tar -xf` into a work dir, then binaries found by file name).
- `pub fn engine_bin(app, id) -> Option<Vec<PathBuf>>` (all binaries present); `engines::ffmpeg(app)` stays as a thin wrapper.
- Status per engine: the existing `FfmpegStatus` renamed `EngineStatus` (same shape incl. `downloadSize`, `error`); `Stage` gains `Unpack` again, `InstallError` gains `Unpack` again (archive assets only).
- Commands/event per Global Constraints; TS: `getEngineStatus(engine)`, `downloadEngine(engine)`, `cancelEngineDownload(engine)`, `removeEngine(engine)`, `useEngineStatus(engine)` listening to `engines://status` filtered by engine.
- One download at a time **per engine**; temp dir `vee-<engine>-download-<identifier>`.

- [ ] Failing tests: `install_from` with an Archive asset (test unpack via a real `tar` of a fixture dir) installs the binary; removing one engine's dir leaves the other; status JSON shape `{ engine: "rhwp", status: { state: "missing", error: null, downloadSize: … } }`. → implement → all phase-2 engine tests still pass → **Commit** `refactor(convert): install FFmpeg and rhwp through one engine table`.
- [ ] Ignored real-install test for rhwp (tempdir), run once on macOS; `rhwp --help` exits 0.

### Task 2: Document and HWP formats

**Files:** `src-tauri/src/domain/convert/{mod,model}.rs`, `src-tauri/src/app/convert.rs` (Caps), `src/domain/convert/apis/convert.dto.ts`, `src/utils/` (`formatLabel` stays uppercase for the new ones).

- `Caps` gains `rhwp: bool`, `libreoffice: bool`, `textutil: bool` (`cfg!(target_os = "macos")`).
- `formats_for` implements the family table exactly; `Engine` gains `Rhwp`, `LibreOffice`.
- [ ] Failing tests per family (own format left out; hwp↔hwpx rule; textutil slices never dimmed; LibreOffice slices dimmed when missing; Windows `.txt` → empty; mixed → empty). → implement → **Commit** `feat(convert): add document and HWP formats`.

### Task 3: rhwp, LibreOffice and textutil runners

**Files:** create `src-tauri/src/domain/convert/{hwp.rs,office.rs}`; modify `mod.rs` (`run` dispatch by source family), `model.rs` (`JobError::Runtime`), `src-tauri/src/app/convert.rs` (`Run` gets `rhwp: Option<PathBuf>`, `soffice: Option<PathBuf>`, `lo_profile: PathBuf`).

- Pure + tested: `rhwp_args(src, to, out) -> Vec<OsString>`; `soffice_args(src, to, outdir, profile) -> Vec<OsString>`; `textutil_args(src, to, out)`; `lo_output(outdir, src, to) -> PathBuf` (`<stem>.<ext>`, `a.b.docx` → `a.b.pdf`).
- Runner: spawn with the shared `command()` (CREATE_NO_WINDOW), wait with a deadline (`OFFICE_TIMEOUT = 5 min`, rhwp `2 min`, textutil `1 min`), kill on timeout → `Engine`; exit `-1073741515` → `Runtime`; output moved like images; temp cleaned on every error.
- [ ] Failing tests (args, `lo_output`, fake rhwp/soffice scripts for failure + timeout + success-with-dotted-name) → implement.
- [ ] Ignored smoke test `converts_documents`: with `VEE_RHWP_DIR` and `VEE_HWP_SAMPLE` (any `.hwpx` the developer has; no sample is committed), convert it to pdf and hwp, then the hwp to hwpx and pdf; with LibreOffice at its default path, make a `.docx` with `textutil` from a short text and convert it to pdf/odt, and a `.txt` → docx via textutil. Assert non-empty outputs with the right magic bytes (`%PDF`, `PK`, `\xD0\xCF\x11\xE0`). Run once on macOS and report.
- **Commit** `feat(convert): convert HWP with rhwp and documents with LibreOffice`.

### Task 4: LibreOffice detection

**Files:** `src-tauri/src/app/engines.rs` (or new `app/office.rs` if it reads better), `src-tauri/src/lib.rs`.

- `pub fn soffice() -> Option<PathBuf>` (paths per Global Constraints); `pub fn libreoffice_version(path) -> Option<String>` (parse first line; pure parser tested with `"LibreOffice 26.8.1.1 bf6ef…"` → `26.8.1.1`); command `get_libreoffice()`; `Caps.libreoffice = soffice().is_some()` at drag start.
- [ ] Tests for the parser and the Windows path list builder → implement → **Commit** `feat(convert): find the user's LibreOffice`.

### Task 5: Settings sections, donut reasons, i18n

**Files:** `ConvertSection.tsx` (extract the FFmpeg section into an `EngineSection({ engine })` used for both), `src/domain/convert/queries/engines.tsx` (`useLibreOffice`), `ConvertApp.tsx` (reason per `needs`), `src/shared/i18n/{en,ko}.ts`.

| key | en | ko |
| --- | --- | --- |
| `convert.needsRhwp` | Needs rhwp | rhwp 필요 |
| `convert.needsLibreOffice` | Needs LibreOffice | LibreOffice 필요 |
| `convert.errors.runtime` | Needs the Microsoft Visual C++ runtime | Microsoft Visual C++ 런타임이 필요해요 |
| `settings.convert.rhwp.caption` | rhwp · HWP conversion | rhwp · HWP 변환 |
| `settings.convert.rhwp.title` | rhwp | rhwp |
| `settings.convert.rhwp.missingHint(mb)` | `` `Needed to convert HWP and HWPX. About ${mb} MB` `` | `` `HWP·HWPX 변환에 필요해요. 약 ${mb}MB` `` |
| `settings.convert.libreoffice.caption` | LibreOffice · Document conversion | LibreOffice · 문서 변환 |
| `settings.convert.libreoffice.title` | LibreOffice | LibreOffice |
| `settings.convert.libreoffice.found` | Found | 찾음 |
| `settings.convert.libreoffice.missing` | Not found | 찾을 수 없음 |
| `settings.convert.libreoffice.hint` | Needed for PDF, DOCX, XLSX and PPTX. Vee finds it once it's installed. | PDF·DOCX·XLSX·PPTX 변환에 필요해요. 설치하면 자동으로 찾아요. |
| `settings.convert.libreoffice.install` | Open download page | 설치 페이지 열기 |
| `settings.convert.libreoffice.version(v)` | `` `Version ${v}` `` | `` `버전 ${v}` `` |

  (The FFmpeg section's existing keys stay; the shared component takes its copy per engine.)
- [ ] `yarn test/typecheck/lint` → **Commit** `feat(convert): manage rhwp and show LibreOffice in settings`.

### Task 6: Docs, checks, running app

- Docs: architecture (engine table, rhwp, LibreOffice, textutil), configuration (rhwp dir, lo-profile), tauri-commands (generic engine commands, `engines://status`, `get_libreoffice`), overview (network: user-started downloads of FFmpeg and rhwp), README (HWP/문서 변환), testing (new ignored tests).
- Four checks; manual pass with the user (macOS: rhwp download, HWP→PDF/HWPX, HWPX→HWP; DOCX→PDF/ODT/TXT; XLSX→PDF; PPTX→PDF; LibreOffice missing → dimmed slices + settings button; Windows: rhwp download + HWP→PDF, VC++ runtime message if missing).
- **Commit** `docs: document HWP and document conversion`.
