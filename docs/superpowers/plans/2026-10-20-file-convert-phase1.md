# Drag File Convert — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** While dragging an image or files card out of the panel, holding the convert key opens a format donut fixed at that point; dropping on a slice writes the converted image into the folder under that point, with progress on the card and a result toast.

**Architecture:** `start_drag` starts a key-watch thread. On key-down it looks up the folder under the cursor (`app/drop_target.rs`, platform code), computes the formats (`domain/convert`) and shows a new transparent `convert` window centred there. The donut (`src/app/convert/ConvertApp.tsx`) hit-tests Tauri `onDragDropEvent` positions and calls `convert_drop(format)`. A single worker thread (`app/convert.rs`) runs jobs through `domain/convert` (sips on macOS, the `image` crate, a tiny JPEG→PDF writer), publishes them to the panel as `convert://jobs`, and ends each job with a toast.

**Tech Stack:** Rust (Tauri 2.12, `image` 0.25, `sips`, `osascript`, `objc2-core-graphics`/`objc2-core-foundation` 0.3.2 on macOS, `windows` 0.62 on Windows), React 19 + Tailwind 4/daisyUI 5.

**Spec:** `docs/superpowers/specs/2026-10-20-file-convert-design.md` (this plan covers phase ① only; phase ② FFmpeg and ③ rhwp/LibreOffice get their own plans).

## Global Constraints

- Branch `feature/file-convert`. Commit messages `<prefix>(<scope>): <subject>` ([commit-messages](../../agents/workflow/commit-messages.md)).
- **Dependencies (approved by the user 2026-10-20).** This set only, nothing else:
  - macOS: `image` add feature `png`; add `objc2-core-graphics = "0.3.2"` and `objc2-core-foundation = "0.3.2"` (both already in `Cargo.lock` through tao), only the features used.
  - Windows: `windows` 0.62.2 add features `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_Shell`, `Win32_UI_Shell_Common`, `Win32_System_Com`, `Win32_System_Ole`, `Win32_System_Variant` as needed.
- Only image and files cards. Text and link cards never arm the key watch.
- Convert key setting `convertKey`: values `alt` | `ctrl` | `shift`; default `alt` on macOS (Option), `ctrl` on Windows. On/off setting `dragConvert`: `on` | `off`, default `on`.
- Phase 1 formats: source extensions `png jpg jpeg webp heic heif tif tiff bmp gif`; targets in donut order `png, jpg, webp, heic, tiff, gif, bmp, pdf`. `heic` target on macOS only. The source's own format is left out (jpg = jpeg, tif = tiff, heic = heif).
- Multi-file cards: formats every file supports; none (or any non-image file) → toast "unsupported", no donut.
- No folder under the key-down point → no donut, "not a folder" toast, at most once per drag. The folder is looked up once per key-down.
- Release then press again in the same drag → new lookup, donut at the new point.
- A drop that lands inside the open donut window never counts as a copy: no `confirm_copy` (no touch, no sound, no "Copied" toast). Panel hides as it does today (pinned panel stays).
- Output name: source stem (image clips: the dragged file name stem, see `drag_file_name`) + new extension; on conflict `name (1).ext`, `name (2).ext`, … Write to a temp dir, then move into the folder. The source is never written.
- Card progress appears only for jobs queued or running ≥ 1 s; done shows ~2 s; failed stays until retried or dismissed. A toast ends every job.
- macOS Finder windows: Automation permission is never prompted mid-drag. Not granted → Finder windows count as "not a folder" (toast hints at Settings). The desktop works without it.
- Colours only in `src/app/globals.css`, both themes. Every user string in `src/shared/i18n/{en,ko}.ts` (exact copy below). Code and comments in English.
- Done = `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` pass, and the app checked in light/dark, ko/en.

## Review Focus

- A Finder window whose folder has no POSIX path (e.g. Recents, a search) → "not a folder" toast, never a crash or a write into `/`. Test in Task 4 (`parse_finder_windows` skips lines without an absolute path).
- The source file of a files card deleted between drag start and drop → job fails with `missing`, toast "Couldn't convert", no partial file left in the target folder. Test in Task 3.
- Target folder not writable (read-only volume, permission) → job fails with `write`, temp output removed. Test in Task 3.
- Name conflict with 2+ existing numbered files (`a.png`, `a (1).png`) → `a (2).png`, and the existing files are untouched. Test in Task 2.
- The key pressed while the cursor is over Vee's own pinned panel → "not a folder" (own windows are not folders), never a write into the app's directory. Test in Task 4 (`pick_hit` with an own-pid window on top).

---

## File Map

| File | Responsibility |
| --- | --- |
| `src-tauri/src/domain/convert/mod.rs` (new) | public API: `formats_for`, `unique_path`, `run` (one job), re-exports |
| `src-tauri/src/domain/convert/model.rs` (new) | `Format`, `JobState`, `JobError`, `JobView`, `visible_jobs` |
| `src-tauri/src/domain/convert/image.rs` (new) | image engine per platform + `jpeg_to_pdf` |
| `src-tauri/src/domain/settings/{mod,model,commands}.rs` | `dragConvert`, `convertKey` keys |
| `src-tauri/src/app/drop_target.rs` (new) | key state, folder under a point, Finder access (platform code + pure helpers) |
| `src-tauri/src/app/convert.rs` (new) | drag session, key watch, overlay window, job worker, events, toasts, `commands` |
| `src-tauri/src/app/window.rs` | `ToastPayload.convert`, `finish_drag` skip, hooks into `app::convert` |
| `src-tauri/tauri.conf.json`, `capabilities/default.json`, `Info.plist` (new), `Entitlements.plist` (new) | `convert` window, Automation usage string + entitlement |
| `src/utils/donut.ts` (+ `.test.ts`) (new) | slice geometry and hit test |
| `src/app/convert/ConvertApp.tsx` (new) | the donut window |
| `src/domain/convert/{apis,queries,components,store}` (new) | DTOs, `call()` wrappers, `useConvertJobs` |
| `src/app/panel/_components/Card.tsx`, `PanelApp.tsx` | job overlay on cards |
| `src/app/toast/ToastApp.tsx` | convert toasts |
| `src/app/settings/_components/{Tabs,ConvertSection}.tsx`, `SettingsApp.tsx` | new "Convert" tab |
| `src/shared/i18n/{en,ko}.ts`, `src/app/globals.css`, `src/app/main.tsx` | copy, tokens, window routing |

---

### Task 1: Spike — windows and drops during an OS drag (throwaway)

Proves the design's riskiest assumption before anything is built. Nothing from this task is committed.

**Files:** temporary edits to `src-tauri/src/app/window.rs`, `src-tauri/tauri.conf.json`, `src/app/main.tsx`.

- [ ] **Step 1:** Add a temporary `convert` window to `tauri.conf.json` (copy the `toast` entry: `create: false`, transparent, `alwaysOnTop`, `focus: false`, `visibleOnAllWorkspaces`, `skipTaskbar`, 260×260, `dragDropEnabled: true`) and to `capabilities/default.json`. In `main.tsx` render a plain red 260×260 div for label `convert` that logs every `getCurrentWebview().onDragDropEvent` payload with `console.log`.
- [ ] **Step 2:** In `window::start_drag`, after `drag::start_drag` returns, spawn a thread that sleeps 1 s, then calls `set_position` on the `convert` window at `app.cursor_position()` (through `cursor_point`) and `show()`.
- [ ] **Step 3:** `yarn tauri dev` on macOS. Drag an image card, wait 1 s, drop on the red square. Record: (a) the window appeared mid-drag, (b) the webview logged `enter`/`over`/`drop` with positions, (c) `finish_drag` got `Dropped`, and which comes first, the JS `drop` event or `finish_drag`, (d) the `_cursor` value `finish_drag` receives compared to the window's `outer_position()` (same coordinate space?).
- [ ] **Step 4:** Same on Windows. Also record whether `WindowFromPoint` at the cursor (log its root class name in the spike thread) returns the drag image window.
- [ ] **Step 5:** Report the findings (a)–(d) to the user in 5 lines and keep them for the PR description. Revert only the files the spike touched: `git checkout -- src-tauri/src/app/window.rs src-tauri/tauri.conf.json src-tauri/capabilities/default.json src/app/main.tsx`. If Windows fails (b) or (a): stop and tell the user; the fix is a vendored `drag` patch running `DoDragDrop` on its own STA thread (record in `vendor/drag/VEE_PATCH.md`) and becomes a new task before Task 6. If no Windows machine is available, say so to the user and continue on macOS; the PR must state Windows was not run.

---

### Task 2: Convert core — formats and output names

**Files:**
- Create: `src-tauri/src/domain/convert/mod.rs`, `src-tauri/src/domain/convert/model.rs`
- Modify: `src-tauri/src/domain/mod.rs` (`pub mod convert;`)

**Interfaces:**
- Produces:
  - `pub enum Format { Png, Jpg, Webp, Heic, Tiff, Gif, Bmp, Pdf }` — `serde(rename_all = "lowercase")`, `Copy, Eq, Debug`; `fn ext(self) -> &'static str` (`"png"`, `"jpg"`, `"webp"`, `"heic"`, `"tiff"`, `"gif"`, `"bmp"`, `"pdf"`); `fn from_ext(ext: &str) -> Option<Format>` (case-insensitive, `jpeg`→Jpg, `tif`→Tiff, `heif`→Heic, `pdf` → None as a source); `const TARGETS: [Format; 8]` in donut order.
  - `pub fn formats_for(paths: &[PathBuf], heic: bool) -> Vec<Format>` — targets common to all sources, in `TARGETS` order, minus every source's own format; empty if `paths` is empty or any path has no image extension.
  - `pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf`.

- [ ] **Step 1: Write the failing tests** (in `mod.rs` `#[cfg(test)] mod tests`)

```rust
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
```

- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib convert` → FAIL (module missing).
- [ ] **Step 3:** Implement `model.rs` (`Format`) and `mod.rs` (`//!` doc line, `formats_for`, `unique_path`). No `tauri` imports.
- [ ] **Step 4:** Run the same command → PASS. Run the layering check from [rust-modules](../../agents/conventions/rust-modules.md#dependency-direction) → empty.
- [ ] **Step 5: Commit** `feat(convert): add image formats and output naming`

---

### Task 3: Image engine and one-job runner

**Files:**
- Create: `src-tauri/src/domain/convert/image.rs`
- Modify: `src-tauri/src/domain/convert/{mod,model}.rs`, `src-tauri/Cargo.toml` (macOS `image` feature `png` — approved set only)

**Interfaces:**
- Consumes: `Format`, `unique_path` (Task 2).
- Produces:
  - `pub enum JobError { Missing, Engine, Write }` — `serde(rename_all = "camelCase")`, `Copy, Debug, PartialEq`.
  - `pub fn jpeg_to_pdf(jpeg: &[u8]) -> Option<Vec<u8>>` (in `image.rs`, re-exported for tests) — one page, page size = pixel size in points, image as a `DCTDecode` XObject; `None` if no SOF0/SOF1/SOF2 marker.
  - `pub fn convert_image(src: &Path, to: Format, out: &Path) -> Result<(), JobError>` (in `image.rs`).
  - `pub fn run(src: &Path, stem: &str, to: Format, dir: &Path, tmp: &Path) -> Result<PathBuf, JobError>` (in `mod.rs`) — converts into `tmp`, then moves to `unique_path(dir, stem, to.ext())` (rename, falling back to copy + remove across volumes); returns the final path. Source missing → `Missing`; engine failure → `Engine`; move failure → `Write`. The temp file is removed on every error.

- [ ] **Step 1: Write the failing tests** (`image.rs` and `mod.rs` tests)

```rust
// image.rs — TINY_JPEG: a 2×1 baseline JPEG as a const byte array (generate once, paste as base64 + decode).
#[test]
fn pdf_wraps_the_jpeg_at_its_pixel_size() {
    let pdf = jpeg_to_pdf(&tiny_jpeg()).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(text.contains("/MediaBox [0 0 2 1]"));
    assert!(text.contains("/Filter /DCTDecode"));
    assert!(text.trim_end().ends_with("%%EOF"));
}
#[test]
fn pdf_rejects_non_jpeg() { assert!(jpeg_to_pdf(b"not a jpeg").is_none()); }

#[test]
fn png_converts_to_every_target() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("p.png");
    std::fs::write(&src, tiny_png()).unwrap(); // 1×1 RGBA PNG const
    let magic: &[(Format, &[u8])] = &[(Format::Jpg, b"\xFF\xD8"), (Format::Webp, b"RIFF"), (Format::Tiff, b"II"),
        (Format::Gif, b"GIF8"), (Format::Bmp, b"BM"), (Format::Pdf, b"%PDF")];
    for (to, head) in magic {
        let out = dir.path().join(format!("o.{}", to.ext()));
        convert_image(&src, *to, &out).unwrap();
        assert!(std::fs::read(&out).unwrap().starts_with(head), "{to:?}");
    }
}
// TIFF magic may be b"MM" from sips: accept either.

// mod.rs
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
```

- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib convert` → FAIL.
- [ ] **Step 3:** Implement. Engines:
  - macOS: `sips -s format <png|jpeg|heic|tiff|gif|bmp> <src> --out <out>` (`std::process::Command`, non-zero exit → `Engine`). `Webp`: sips → temp PNG → `image` decode → `image::codecs::webp::WebPEncoder::new_lossless`. `Pdf`: sips → temp JPEG (`-s formatOptions 90`) → `jpeg_to_pdf`.
  - Windows: `image::open` → `Jpg`/`Pdf` go through `to_rgb8()` (JPEG has no alpha), JPEG quality 90; `Pdf` = encoded JPEG → `jpeg_to_pdf`; other targets `save_with_format`. `Heic` never reaches Windows (`formats_for(.., heic=false)`).
  - `// ponytail:` lossless-only WebP (the `image` crate has no lossy encoder); revisit if files are too large.
- [ ] **Step 4:** Run tests → PASS (on macOS this runs `sips`; that's expected).
- [ ] **Step 5: Commit** `feat(convert): convert images with sips and the image crate`

---

### Task 4: Settings keys, key state and the folder under the cursor

**Files:**
- Modify: `src-tauri/src/domain/settings/{mod,model,commands}.rs`, `src/domain/settings/apis/settings.dto.ts`
- Create: `src-tauri/src/app/drop_target.rs`; Modify: `src-tauri/src/app/mod.rs`, `src-tauri/Cargo.toml` (approved macOS/Windows deps)

**Interfaces:**
- Produces:
  - settings: `get(conn, "dragConvert")` default `"on"`; `get(conn, "convertKey")` default `"alt"` on macOS, `"ctrl"` on Windows (`cfg!`); `validate` accepts `dragConvert` ∈ {on, off}, `convertKey` ∈ {alt, ctrl, shift}; `SettingsDto` gains `drag_convert`, `convert_key`; TS `Settings` gains `dragConvert: 'on' | 'off'`, `convertKey: 'alt' | 'ctrl' | 'shift'`, `SettingKey` gains both.
  - `drop_target`:
    - `pub fn key_down(key: &str) -> bool` — macOS `CGEventSourceFlagsState(CombinedSessionState)` masks alt `0x80000`, ctrl `0x40000`, shift `0x20000`; Windows `GetAsyncKeyState(VK_MENU|VK_CONTROL|VK_SHIFT) & 0x8000`.
    - `pub enum DropFolder { Folder(PathBuf), NotFolder, FinderBlocked }`
    - `pub fn folder_at(point: (f64, f64)) -> DropFolder` — `point` in the window-positioning space (`window::cursor_point` output).
    - Pure helpers (both platforms compile them, tested everywhere): `pub struct WinInfo { pub pid: i32, pub layer: i64, pub bounds: (f64, f64, f64, f64) /* x, y, w, h */, pub finder: bool }`; `pub enum Hit { Desktop, Finder, Other }`; `pub fn pick_hit(windows: &[WinInfo], point: (f64, f64), own_pid: i32) -> Hit` (front-to-back; only `layer == 0`; first window containing the point: own pid → Other, `finder` → Finder, else Other; none → Desktop); `pub fn parse_finder_windows(out: &str) -> Vec<((f64, f64, f64, f64), PathBuf)>` (lines `left,top,right,bottom\t/abs/path/`; skips lines without an absolute path; bounds turned into x, y, w, h).
    - `pub enum FinderAccess { Granted, Denied, Unknown }` + `pub fn finder_access(ask: bool) -> Option<FinderAccess>` (None on Windows).

- [ ] **Step 1: Write the failing tests**

```rust
// settings/mod.rs
#[test]
fn convert_settings_default_and_validate() {
    let conn = db::open_in_memory().unwrap();
    assert_eq!(get(&conn, "dragConvert"), "on");
    assert_eq!(get(&conn, "convertKey"), if cfg!(target_os = "macos") { "alt" } else { "ctrl" });
    for v in ["alt", "ctrl", "shift"] { assert!(validate("convertKey", v).is_ok()); }
    assert!(validate("convertKey", "cmd").is_err());
    assert!(validate("dragConvert", "off").is_ok());
    assert!(validate("dragConvert", "yes").is_err());
}

// app/drop_target.rs
fn w(pid: i32, layer: i64, b: (f64, f64, f64, f64), finder: bool) -> WinInfo { WinInfo { pid, layer, bounds: b, finder } }
#[test]
fn nothing_under_the_point_is_the_desktop() {
    assert!(matches!(pick_hit(&[w(9, 0, (0., 0., 10., 10.), false)], (50., 50.), 1), Hit::Desktop));
}
#[test]
fn front_window_wins_and_other_layers_are_ignored() {
    let ws = [w(5, 25, (0., 0., 100., 100.), false), w(7, 0, (0., 0., 100., 100.), true), w(8, 0, (0., 0., 100., 100.), false)];
    assert!(matches!(pick_hit(&ws, (10., 10.), 1), Hit::Finder));
}
#[test]
fn own_windows_are_not_folders() {
    let ws = [w(1, 0, (0., 0., 100., 100.), false), w(7, 0, (0., 0., 100., 100.), true)];
    assert!(matches!(pick_hit(&ws, (10., 10.), 1), Hit::Other));
}
#[test]
fn finder_lines_without_a_path_are_skipped() {
    let out = "10,20,410,320\t/Users/me/Downloads/\n0,0,5,5\t\n0,0,5,5\tmissing value\n";
    assert_eq!(parse_finder_windows(out), vec![((10., 20., 400., 300.), PathBuf::from("/Users/me/Downloads/"))]);
}
```

- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib` → FAIL.
- [ ] **Step 3:** Implement settings keys (Rust + TS DTO) and `drop_target.rs`:
  - macOS `folder_at`: `CGWindowListCopyWindowInfo(OptionOnScreenOnly | ExcludeDesktopElements, kCGNullWindowID)` → `WinInfo` from `kCGWindowOwnerPID`, `kCGWindowLayer`, `kCGWindowBounds`; `finder` = `NSRunningApplication::runningApplicationWithProcessIdentifier(pid).bundleIdentifier() == "com.apple.finder"`. `Desktop` → `NSHomeDirectory()/Desktop` (use `dirs`-free `std::env::home_dir`). `Finder` → `finder_access(false)`; not `Granted` → `FinderBlocked`; else run `osascript -e <script>` (below), `parse_finder_windows`, first bounds containing the point → `Folder`, none → `NotFolder`. `Other` → `NotFolder`.
  - Finder script (Finder lists windows front to back):
    ```applescript
    tell application "Finder"
      set out to ""
      repeat with w in (every Finder window)
        try
          set b to bounds of w
          set out to out & (item 1 of b) & "," & (item 2 of b) & "," & (item 3 of b) & "," & (item 4 of b) & tab & (POSIX path of (target of w as alias)) & linefeed
        end try
      end repeat
      return out
    end tell
    ```
    `// ponytail: osascript process per lookup (~100 ms); move to NSAppleScript on a dedicated thread if it feels slow.`
  - macOS `finder_access(ask)`: `AECreateDesc(typeApplicationBundleID 'bund', "com.apple.finder")` + `AEDeterminePermissionToAutomateTarget(&desc, typeWildCard, typeWildCard, ask)` via `extern "C"` linked to the `CoreServices` framework; `0` → Granted, `-1743` → Denied, anything else (`-1744`, `-600`) → Unknown. Call with `ask = true` only from the settings command, off the main thread.
  - Windows `folder_at`: `WindowFromPoint` → `GetAncestor(GA_ROOT)` → class name. `Progman`/`WorkerW` → `SHGetKnownFolderPath(FOLDERID_Desktop)`. `CabinetWClass` → `IShellWindows` (CoInitialize on the calling thread), match the item whose HWND is the root, pick the active tab (the first `ShellTabWindowClass` child via `FindWindowExW`, compared with `IShellBrowser::GetWindow` reached through `IServiceProvider::QueryService(SID_STopLevelBrowser)`), then `QueryActiveShellView` → `IFolderView::GetFolder(IPersistFolder2)` → `GetCurFolder` → `SHGetPathFromIDListW`. Own process root (`GetWindowThreadProcessId`) → `NotFolder`. Anything else → `NotFolder`.
- [ ] **Step 4:** Run tests → PASS. Add an `#[ignore]`d smoke test `folder_at_desktop_corner` that prints `folder_at` for the cursor; run it by hand once per platform with the cursor on the desktop and on a Finder/Explorer window.
- [ ] **Step 5: Commit** `feat(convert): find the folder under the cursor and add convert settings`

---

### Task 5: Job list model

**Files:** Modify `src-tauri/src/domain/convert/{mod,model}.rs`

**Interfaces:**
- Consumes: `Format`, `JobError` (Tasks 2–3).
- Produces (`model.rs`, re-exported):
  - `pub enum JobState { Queued, Running, Done, Failed(JobError) }`
  - `pub struct Job { pub id: u64, pub clip_id: i64, pub format: Format, pub files: Vec<PathBuf>, pub stem: String, pub dir: PathBuf, pub done: usize, pub state: JobState, pub since: i64 /* ms when the job was queued */, pub ended: Option<i64>, pub shown: bool }`
  - `#[derive(Serialize)] #[serde(rename_all = "camelCase")] pub struct JobView { pub id: u64, pub clip_id: i64, pub format: Format, pub state: &'static str /* "queued" | "running" | "done" | "failed" */, pub position: usize /* 1-based among queued, 0 otherwise */, pub done: usize, pub total: usize, pub folder: String /* last path component of dir */, pub error: Option<JobError> }`
  - `pub fn visible_jobs(jobs: &mut [Job], now: i64) -> Vec<JobView>` — sets `shown` once a job has been queued/running ≥ `SHOW_AFTER_MS = 1000`; failed jobs always shown; done jobs shown only if already `shown` and `now - ended < DONE_MS = 2000`; returns views of shown jobs.
  - `pub fn prune(jobs: &mut Vec<Job>, now: i64)` — drops done jobs older than `DONE_MS` and done jobs never shown.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn quick_jobs_never_show() {
    let mut jobs = vec![job(1, JobState::Running, 0)];
    assert!(visible_jobs(&mut jobs, 500).is_empty());
    jobs[0].state = JobState::Done; jobs[0].ended = Some(600);
    assert!(visible_jobs(&mut jobs, 700).is_empty());
}
#[test]
fn slow_jobs_show_then_done_lingers_two_seconds() {
    let mut jobs = vec![job(1, JobState::Running, 0)];
    assert_eq!(visible_jobs(&mut jobs, 1000)[0].state, "running");
    jobs[0].state = JobState::Done; jobs[0].ended = Some(3000);
    assert_eq!(visible_jobs(&mut jobs, 4000)[0].state, "done");
    assert!(visible_jobs(&mut jobs, 5000).is_empty());
}
#[test]
fn failed_jobs_show_at_once_and_queue_positions_count_from_one() {
    let mut jobs = vec![job(1, JobState::Failed(JobError::Engine), 0), job(2, JobState::Queued, 0), job(3, JobState::Queued, 0)];
    let v = visible_jobs(&mut jobs, 1500);
    assert_eq!(v[0].error, Some(JobError::Engine));
    assert_eq!((v[1].position, v[2].position), (1, 2));
}
```

- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** Run → PASS.
- [ ] **Step 5: Commit** `feat(convert): track convert jobs for the panel`

---

### Task 6: Drag session, overlay window and worker

**Files:**
- Create: `src-tauri/src/app/convert.rs`
- Modify: `src-tauri/src/app/{mod,window}.rs`, `src-tauri/src/lib.rs` (handlers, spawn worker), `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src/domain/clip/apis/clip.dto.ts` (`ToastPayload`)

**Interfaces:**
- Consumes: `formats_for`, `run`, `Job`, `visible_jobs`, `prune` (Tasks 2–5); `drop_target::{key_down, folder_at, DropFolder}` (Task 4); `window::{cursor_point, place, show_toast, Rect}` (make the needed ones `pub(crate)`).
- Produces:
  - `pub const CONVERT: &str = "convert";` window label. Config: copy of `toast` (create false, transparent, decorations false, alwaysOnTop, skipTaskbar, focus/focusable false, visibleOnAllWorkspaces, shadow false), 260×260, `dragDropEnabled: true`. Added to capabilities `windows`.
  - `pub fn begin(app: &AppHandle, clip_id: i64, files: Vec<PathBuf>, stem: String)` — called from `window::start_drag` for image (staged PNG path) and files clips; no-op if `dragConvert` is off. Stores the session and spawns the key watch: every 30 ms while `state.dragging`, read `key_down(convertKey)`; on press → `open_at(cursor)`; on release → `close()`.
  - `open_at`: formats empty → toast `Unsupported` (once per drag); `folder_at` → `NotFolder`/`FinderBlocked` → toast `NotFolder { finderBlocked }` (once per drag); `Folder(dir)` → save `dir` in the session, emit `convert://open` `{ formats, name }` to `convert`, centre the window on the point (size scaled like `show_toast` does), `show()`.
  - `pub fn dropped_on_overlay(app: &AppHandle, cursor: (f64, f64)) -> bool` — overlay visible and its outer rect contains `cursor` (space confirmed in Task 1). Always hides the overlay.
  - `ToastPayload` gains `pub convert: Option<ConvertNote>`; `#[serde(tag = "type", rename_all = "camelCase")] pub enum ConvertNote { Saved { format: Format, count: usize, folder: String }, Failed { name: String }, NotFolder { finder_blocked: bool }, Unsupported }` (`rename_all_fields = "camelCase"`). Existing constructors set `convert: None`.
  - Events: `convert://open` (to `convert`), `convert://close` (to `convert`), `convert://jobs` `JobView[]` (to `panel`, after every change and every 500 ms while any job is shown).
  - `commands` submodule: `convert_drop(format: String) -> AppResult<()>` (parses `Format`, takes the session; no session or format not in its list → `InvalidInput`; queues one `Job` for all files), `list_convert_jobs() -> Vec<JobView>`, `retry_convert(id: u64)` (failed → queued again, same dir/format), `dismiss_convert(id: u64)` (removes a queued or failed job).
  - Worker: one thread, `mpsc` of job ids; runs each file through `convert::run` with `tmp = temp_dir()/vee-convert`; updates `done`; first error → `Failed(e)` (remaining files skipped). End → toast `Saved { format, count: done, folder }` or `Failed { name: first failing file name }`. `// ponytail: one worker for all images; a second worker if batches feel slow.`

- [ ] **Step 1:** `window::finish_drag` gets the drag callback's cursor; on `Dropped`, `if convert::dropped_on_overlay(app, cursor)` → same panel handling as today but **skip `confirm_copy`**. Both platforms.
- [ ] **Step 2:** Implement `app/convert.rs`, config, capabilities, `ToastPayload`/TS `ToastPayload` (`convert: ConvertNote | null` with the tagged union mirrored).
- [ ] **Step 3:** `cargo test --manifest-path src-tauri/Cargo.toml --lib` and `yarn typecheck` → PASS (no new unit tests: Tauri-facing code, verified in Task 9).
- [ ] **Step 4: Commit** `feat(convert): open the convert overlay while dragging and run jobs`

---

### Task 7: Donut window

**Files:**
- Create: `src/utils/donut.ts`, `src/utils/donut.test.ts`, `src/app/convert/ConvertApp.tsx`, `src/domain/convert/apis/{convert.ts,convert.dto.ts}`, `src/domain/convert/{queries,components,store}/.gitkeep`
- Modify: `src/app/main.tsx` (label `convert` → `ConvertApp`), `src/app/globals.css` (transparent body for `data-window='convert'`, `--vee-donut` light `rgba(255,255,255,0.92)` / dark `rgba(31,32,39,0.92)` exposed as `bg-donut`/`fill-donut`), `src/shared/i18n/{en,ko}.ts`

**Interfaces:**
- Consumes: `convert://open` `{ formats: Format[]; name: string }`, `convert://close`, command `convert_drop`.
- Produces:
  - `export const DONUT = { size: 260, outer: 112, inner: 44 } as const;`
  - `export function sliceAt(x: number, y: number, count: number): number | null` — `x, y` CSS px from the window's top-left; centre `(130, 130)`; slices clockwise from 12 o'clock, equal angles; `null` inside `inner`, outside `outer`, or `count === 0`.
  - `export function slicePath(i: number, count: number, gap?: number): string` — SVG path in a `-130..130` viewBox (mockup geometry, `gap` 0.03 rad default).
  - `convert.dto.ts`: `type Format = 'png' | 'jpg' | 'webp' | 'heic' | 'tiff' | 'gif' | 'bmp' | 'pdf'`; `interface ConvertJob { id: number; clipId: number; format: Format; state: 'queued' | 'running' | 'done' | 'failed'; position: number; done: number; total: number; folder: string; error: 'missing' | 'engine' | 'write' | null }`; `type ConvertNote = …` (Task 6).
  - `convert.ts`: `convertDrop(format: Format)`, `listConvertJobs()`, `retryConvert(id: number)`, `dismissConvert(id: number)`.

- [ ] **Step 1: Write the failing test** (`donut.test.ts`, `node:test`)

```ts
test('slices run clockwise from the top', () => {
  assert.equal(sliceAt(130, 130 - 80, 6), 0);   // straight up
  assert.equal(sliceAt(130 + 80, 130, 6), 1);   // right: 90° falls in slice 1 of 6 (60°–120°)
  assert.equal(sliceAt(130, 130 + 80, 6), 3);   // down
});
test('centre, outside and empty donuts hit nothing', () => {
  assert.equal(sliceAt(130, 130, 6), null);
  assert.equal(sliceAt(130 + 120, 130, 6), null);
  assert.equal(sliceAt(130, 50, 0), null);
});
```

- [ ] **Step 2:** `yarn test` → FAIL. **Step 3:** Implement `donut.ts` (only `import type` with `@/`). **Step 4:** `yarn test` → PASS.
- [ ] **Step 5:** Implement `ConvertApp.tsx` (sections per [react-sections](../../agents/conventions/react-sections.md)): listens `convert://open` / `convert://close`; `getCurrentWebview().onDragDropEvent`: `enter`/`over` → `sliceAt(pos.x / devicePixelRatio, pos.y / devicePixelRatio)` → highlighted index; `leave` → none; `drop` → index ≠ null → `convertDrop(formats[i])`. SVG ring: slice `fill-donut stroke-base-300`, hot slice `fill-primary`, labels `format.toUpperCase()` 12px bold (`fill-base-content`, hot `fill-primary-content`); centre disc `bg-card shadow-card` with the file name (truncate) and `t.convert.dropHint`. `aria-hidden` on the SVG; the window is pointer-only.
- [ ] **Step 6:** `yarn typecheck && yarn lint` → PASS. **Commit** `feat(convert): add the format donut window`

---

### Task 8: Card progress, toasts and the Convert settings tab

**Files:**
- Create: `src/domain/convert/queries/convert.tsx` (`useConvertJobs`), `src/app/settings/_components/ConvertSection.tsx`, `src-tauri/Info.plist`, `src-tauri/Entitlements.plist`
- Modify: `src/app/panel/{PanelApp.tsx,_components/Card.tsx}`, `src/app/toast/ToastApp.tsx`, `src/app/settings/{SettingsApp.tsx,_components/Tabs.tsx}`, `src/app/globals.css`, `src/shared/i18n/{en,ko}.ts`, `src-tauri/src/app/drop_target.rs` (commands), `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json` (`bundle.macOS.entitlements`), `src/domain/convert/apis/convert.ts`

**Interfaces:**
- Consumes: `ConvertJob`, `ConvertNote`, the commands from Tasks 6–7.
- Produces:
  - `useConvertJobs(): Map<number, ConvertJob>` keyed by `clipId` — seeds from `listConvertJobs()` on mount and on `panel://opened`, replaced by every `convert://jobs` payload (`queryClient.setQueryData(['convert', 'jobs'], …)`).
  - `Card` prop `job?: ConvertJob`.
  - Rust commands in `drop_target.rs`: `get_finder_access() -> Option<String>` (`"granted" | "denied" | "unknown"`, None on Windows), `request_finder_access()` (runs `finder_access(true)` on a spawned thread, then emits `settings://changed`), `open_automation_settings()` (opens `x-apple.systempreferences:com.apple.preference.security?Privacy_Automation`). TS wrappers in `convert.ts`: `getFinderAccess`, `requestFinderAccess`, `openAutomationSettings`.
  - Tokens (`globals.css` `@theme inline`): `--color-job-veil: rgba(16, 17, 20, 0.72)`, `--color-job-failed: color-mix(in srgb, var(--color-error) 78%, transparent)`; keyframes + `animate-job-indeterminate` (35 % bar sliding −100 % → 300 %, 1.2 s, `motion-reduce` static).
  - `SettingsTab` gains `'convert'`; order `general, panel, shortcuts, convert, history`; icon path `M4 7h13l-3-3M20 17H7l3 3`.

- [ ] **Step 1: Card overlay** (mockup §3). Over the card body: `queued` → veil + `t.convert.queued(position)`, ✕ (`aria-label={t.convert.dismiss}`) → `dismissConvert`; `running` → veil + `t.convert.converting(FMT)`, bar: `total > 1` → width `done/total` and `${done}/${total}`, else indeterminate; `done` → `border-success` + chip `t.convert.saved(FMT)`, foot shows `folder`; `failed` → `bg-job-failed` + `t.convert.failed`, `t.convert.errors[error]`, button `t.convert.retry` → `retryConvert`, ✕ → `dismissConvert`. Keep `card`, `data-id`, `ghost` hooks. Buttons stop propagation (no select/copy/drag).
- [ ] **Step 2: Toasts.** `ToastApp`: when `p.convert` is set — `saved` → ok tile (image/files icon + check), title `count > 1 ? t.convert.savedMany(count, FMT) : t.convert.savedOne(FMT)`, summary `folder`; `failed` → red tile, `t.convert.failedTitle`, summary `name`; `notFolder` → red tile, `t.convert.notFolder`, summary `finderBlocked ? t.convert.finderBlockedHint : null`; `unsupported` → red tile, `t.convert.unsupported`.
- [ ] **Step 3: Settings tab** (mockup §4, phase 1 rows only): `SettingSection` with `Row` `dragConvert` (`Switch`), `Row` `convertKey` (`Segmented`; macOS options `alt ⌥ Option`, `ctrl ⌃ Control`, `shift ⇧ Shift`; Windows `ctrl Ctrl`, `alt Alt`, `shift Shift` — labels from `t.settings.convert.keys`, `isMac` as in `ShortcutRecorder`), and macOS-only `Row` Finder access: status (`granted` → ok badge `t.settings.convert.allowed`; `denied` → `Button` `t.settings.convert.openSystemSettings`; `unknown` → `Button` `t.settings.convert.allow` → `requestFinderAccess`). Rows dim when `dragConvert` is off.
- [ ] **Step 4: macOS plist.** `src-tauri/Info.plist` with `NSAppleEventsUsageDescription` = "Vee asks Finder which folder is open so it can save converted files there."; `src-tauri/Entitlements.plist` with `com.apple.security.automation.apple-events` = true; `bundle.macOS.entitlements: "Entitlements.plist"`. Check `docs/agents/workflow/release.md` signing still works (note in PR if not verified by a signed build).
- [ ] **Step 5: i18n** — add exactly:

| key | en | ko |
| --- | --- | --- |
| `settings.tabs.convert` | Convert | 변환 |
| `settings.convert.dragConvert` | Convert while dragging | 드래그 변환 |
| `settings.convert.dragConvertHint` | Hold the key while dragging a card to pick a format | 카드를 끄는 중에 키를 누르면 포맷 선택 창이 열려요 |
| `settings.convert.key` | Convert key | 변환 키 |
| `settings.convert.keyHint` | The key you hold while dragging | 드래그 중에 누르는 키 |
| `settings.convert.keys` | `{ mac: { alt: '⌥ Option', ctrl: '⌃ Control', shift: '⇧ Shift' }, win: { ctrl: 'Ctrl', alt: 'Alt', shift: 'Shift' } }` | same |
| `settings.convert.finderAccess` | Finder window access | Finder 창 접근 |
| `settings.convert.finderAccessHint` | Needed to convert into an open Finder window | 열린 Finder 창 위에서도 변환하려면 필요해요 |
| `settings.convert.allowed` | Allowed | 허용됨 |
| `settings.convert.allow` | Allow | 허용 |
| `settings.convert.openSystemSettings` | Open System Settings | 시스템 설정 열기 |
| `convert.dropHint` | Drop on a format | 놓을 포맷 선택 |
| `convert.converting(f)` | `` `To ${f}…` `` | `` `${f} 변환 중` `` |
| `convert.queued(n)` | `` `Waiting · #${n}` `` | `` `대기 중 · ${n}번째` `` |
| `convert.saved(f)` | `` `✓ ${f} saved` `` | `` `✓ ${f} 저장됨` `` |
| `convert.failed` | Couldn't convert | 변환 실패 |
| `convert.retry` | Retry | 다시 |
| `convert.dismiss` | Dismiss | 닫기 |
| `convert.errors` | `{ missing: 'The original is gone', engine: "This file can't be converted", write: "Couldn't save in this folder" }` | `{ missing: '원본이 없어요', engine: '이 파일은 변환할 수 없어요', write: '이 폴더에 저장할 수 없어요' }` |
| `convert.savedOne(f)` | `` `Saved as ${f}` `` | `` `${f} 파일로 저장했어요` `` |
| `convert.savedMany(n, f)` | `` `Saved ${n} files as ${f}` `` | `` `${n}개 파일을 ${f} 파일로 저장했어요` `` |
| `convert.failedTitle` | Couldn't convert | 변환하지 못했어요 |
| `convert.notFolder` | Convert only over a folder | 폴더 위에서만 변환할 수 있어요 |
| `convert.finderBlockedHint` | Allow Finder access in Settings → Convert | 설정 → 변환에서 Finder 접근을 허용해 주세요 |
| `convert.unsupported` | These files can't be converted | 이 파일은 변환할 수 없어요 |

- [ ] **Step 6:** `yarn typecheck && yarn lint && yarn test` → PASS. **Commit** `feat(convert): show convert progress, toasts and settings`

---

### Task 9: Docs, verification and the running app

**Files:** Modify `docs/agents/{overview,architecture,structure}.md`, `docs/agents/conventions/tauri-commands.md` (events table: `convert://open`, `convert://close`, `convert://jobs`; window list), `docs/agents/configuration.md` (Info.plist, entitlements).

- [ ] **Step 1:** Update the docs above (five windows; `domain/convert`; `app/convert.rs`, `app/drop_target.rs`; new events and commands).
- [ ] **Step 2:** Run all four: `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` → all PASS (paste counts in the PR).
- [ ] **Step 3:** `yarn tauri dev` (separate identifier if Vee is installed, see [build-run](../../agents/build-run.md)). Check by hand, light **and** dark, 한국어 **and** English:
  1. Image card → desktop, Option → donut at the press point, stays put while the cursor moves; drop on JPG → `… .jpg` on the desktop, toast "JPG 파일로 저장했어요 · Desktop"; the panel closes; no "Copied" toast, no copy sound.
  2. Same over an open Finder window (after allowing access in Settings) → file in that folder; before allowing → "폴더 위에서만…" toast with the Settings hint.
  3. Over Safari → toast, no donut; drag continues and a normal drop still copies.
  4. Release Option mid-drag → donut closes; drop → normal copy. Press again elsewhere → donut at the new point.
  5. Files card with 3 large images → card shows waiting/progress `1/3`, done chip, then back; one toast for the batch.
  6. Files card with a `.docx` → "이 파일은 변환할 수 없어요".
  7. Name conflict → `name (1).ext`.
  8. Settings → Convert: switch off → Option does nothing; change key to Shift → works with Shift.
  9. Windows: 1, 3, 4, 7, 8 with Ctrl, desktop and an Explorer tab (Windows 11 with 2 tabs: the active tab's folder).
- [ ] **Step 4: Commit** `docs: document drag file convert`, then open the PR per [pr](../../agents/workflow/pr.md) (`feat(convert): …` title), listing anything not run (signed build, Windows).
