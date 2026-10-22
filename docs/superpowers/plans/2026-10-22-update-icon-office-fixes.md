# Update Re-check, Gear Icon, Office Fonts, Image Icons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Install the newest release even if a newer one shipped after the download; center the panel's gear icon and fix its wobbly spin; make Korean office → PDF conversions close to the original; replace BMP/TIFF image targets with ICO/ICNS.

**Architecture:** The updater keeps checking while an update is ready (quietly, without the `Checking` state) and re-checks once under an async lock right before installing. A version comparator keeps only strictly newer releases. The gear becomes an inline SVG. Before every `soffice` run, Vee rewrites `registrymodifications.xcu` in its private LibreOffice profile with a ko-KR locale and a font replacement table. ICO/ICNS are written by hand as PNG-payload containers with the `image` crate. There are no new dependencies.

**Tech Stack:** Rust (Tauri 2.12, tauri-plugin-updater 2.13, image 0.25), React 19 + TypeScript, Tailwind 4 / daisyUI 5, `node:test`.

**Spec:** `docs/superpowers/specs/2026-10-22-update-icon-office-fixes-design.md`

## Global Constraints

- Branch `feature/update-icon-office-fixes` (already created). Commit prefixes follow `docs/agents/workflow/commit-messages.md`.
- No dependency changes in `package.json` or `src-tauri/Cargo.toml`. If one becomes necessary, stop and ask the user.
- Code and comments are in English. Every user-visible string lives in `src/shared/i18n/{en,ko}.ts`.
- Done means all four pass: `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint`.
- UI changes are checked in `yarn tauri dev`, in light and dark and in ko and en (see `docs/agents/build-run.md` when the installed Vee is running).
- Verify LibreOffice config keys against real LibreOffice output, not memory.

## Review Focus

1. **Offline when "Restart" is pressed:** the re-check fails, and the downloaded update still installs. → `settle` test in Task 1.
2. **A release is pulled** (GitHub's latest is older than the downloaded one): keep the downloaded one. → `is_newer` test in Task 1.
3. **Non-square or tiny (1×1) source image:** a valid ICO/ICNS with every size, padded transparent. → Task 4 tests.
4. **A LibreOffice profile left by 1.2.0** with LibreOffice's own rewritten xcu: overwritten before the next run. → `write_profile` test in Task 5.
5. **Mixed-format image drop** (PNG + JPG + BMP): ICO and ICNS are offered and BMP/TIFF are not. → `formats_for` test in Task 4.

---

### Task 1: Updater keeps checking and re-checks before install (Rust)

**Files:**
- Modify: `src-tauri/src/app/updater.rs`
- Modify: `src-tauri/src/app/tray.rs:50-61` (the `"update"` arm)

**Interfaces:**
- Produces:
  - `pub async fn install_and_restart(app: &AppHandle) -> AppResult<()>`
  - `#[tauri::command] pub async fn install_update_and_restart(app: AppHandle) -> AppResult<()>`, same command name, so the frontend `installUpdate()` is unchanged.
  - `pub async fn check(app: &AppHandle) -> UpdateStatus`, same signature.

- [ ] **Step 1: Write failing tests** in the `tests` module of `updater.rs`:

```rust
#[test]
fn only_strictly_newer_versions_replace_the_download() {
    assert!(is_newer("1.3.0", "1.2.0"));
    assert!(is_newer("1.10.0", "1.9.9"));
    assert!(!is_newer("1.2.0", "1.2.0"));
    assert!(!is_newer("1.1.0", "1.2.0")); // pulled release
    assert!(!is_newer("garbage", "1.2.0"));
}

#[test]
fn a_ready_update_survives_a_failed_or_empty_check() {
    let ready = |v: &str| UpdateStatus::Ready { version: v.into() };
    assert_eq!(settle(Some("1.2.0"), Ok(Some("1.3.0".into())), 7), ready("1.3.0"));
    assert_eq!(settle(Some("1.2.0"), Ok(None), 7), ready("1.2.0"));
    assert_eq!(settle(Some("1.2.0"), Err(()), 7), ready("1.2.0")); // offline
    assert_eq!(settle(None, Ok(None), 7), UpdateStatus::UpToDate { checked_at: 7 });
    assert_eq!(settle(None, Err(()), 7), UpdateStatus::Failed { checked_at: 7 });
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib updater`
Expected: compile errors, because `is_newer` and `settle` are not defined.

- [ ] **Step 3: Implement**
  - `fn is_newer(candidate: &str, than: &str) -> bool`: compare `major.minor.patch` as a `(u64, u64, u64)` tuple. If either side fails to parse, return false. Vee has no prereleases.
  - `fn settle(ready: Option<&str>, found: Result<Option<String>, ()>, now: i64) -> UpdateStatus`. The table is the test above.
  - `UpdateState` gains `busy: tauri::async_runtime::Mutex<()>`. It serializes check, download and install.
  - `fetch(app, floor: Option<String>)`: build with `app.updater_builder().version_comparator(move |current, r| match &floor { Some(f) => is_newer(&r.version.to_string(), f), None => r.version > current }).build()?.check()`.
  - Shared `async fn refresh(app: &AppHandle) -> UpdateStatus`. The caller holds `busy`.
    - Read the ready version. Only when there is none, call `set_status(Checking)`.
    - Then `fetch(app, ready)`. On a new download, swap `pending` and call `whats_new::save`.
    - Then `set_status(settle(...))`.
  - `check()`: return early only when the status is `Checking`. Otherwise lock `busy` and call `refresh`.
  - `install_and_restart()`: lock `busy` and call `refresh`, ignoring its result. Then install `pending` as today and call `app.restart()`.
  - The command becomes async.
  - Update the module doc line to say the updater keeps checking after a download.

- [ ] **Step 4: Tray.** In the `"update"` arm, when an update is ready, call `tauri::async_runtime::spawn` on `install_and_restart(&app)` and log the error. Keep the existing `else` branch.

- [ ] **Step 5: Run the tests and confirm they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib updater`
Expected: PASS, along with the existing `status_serializes_like_the_typescript_union`.

- [ ] **Step 6: Commit** `fix(updater): re-check before installing so the newest release wins`

---

### Task 2: "Checking for the latest" wait state in the panel and Settings

**Files:**
- Modify: `src/shared/i18n/en.ts`, `src/shared/i18n/ko.ts` (top level, next to `restart`)
- Modify: `src/app/panel/PanelApp.tsx` (around `installUpdate`, lines 318-326 and 91-92)
- Modify: `src/app/panel/_components/Toolbar.tsx` (the update popover, lines 201-232)
- Modify: `src/app/settings/SettingsApp.tsx` (the update row, around line 57)

**Interfaces:**
- Consumes: `installUpdate()` from `@/domain/updater/apis/updater` (unchanged). It now resolves only on failure, because success restarts the app.
- Produces: i18n key `updateInstalling`. en: `'Getting the latest version…'`. ko: `'최신 버전 확인 중…'`.

- [ ] **Step 1: Add `updateInstalling`** to both dictionaries.
- [ ] **Step 2: Panel**
  - Add `const [updateInstalling, setUpdateInstalling] = useState(false)`.
  - `installUpdate` sets it to true, and its `.catch` sets failed and resets it to false.
  - `resetView` and `cancelUpdate` reset it as well.
  - Pass it to `Toolbar` as the prop `updateInstalling: boolean`.
  - In the popover, while it is true:
    - the hint line shows `t.updateInstalling`;
    - the Restart `ConfirmButton` is `disabled`, so add a `disabled` prop to `ConfirmButton` if it doesn't forward one;
    - Cancel stays enabled.
- [ ] **Step 3: Settings.** Give the update row a local `installing` state.
  - While it is true, the button is `disabled` and shows `t.updateInstalling`.
  - On rejection, the row shows `t.updateFailed`, since the panel already uses that key.
- [ ] **Step 4: Verify**

Run: `yarn typecheck && yarn lint && yarn test`
Expected: all pass.

- [ ] **Step 5: Commit** `feat(updater): show a wait state while the latest version is fetched`

---

### Task 3: Gear icon as a centered SVG

**Files:**
- Modify: `src/app/panel/_components/Toolbar.tsx:260-269`

- [ ] **Step 1: Swap the glyph.** Replace the `⚙︎` `<span>` with an inline `<svg>` that uses the pin icon's attributes exactly:
  - `viewBox="0 0 24 24"`, `fill="none"`, `stroke="currentColor"`, `strokeWidth="2"`, `strokeLinecap="round"`, `strokeLinejoin="round"`, `aria-hidden`;
  - `className="block size-4.5 transition-transform duration-300 motion-safe:group-hover:rotate-45"`.
  - Paths: Lucide `settings` (ISC), copied from lucide.dev. It is symmetric around (12, 12).
  - `Tip` stays a sibling, so it does not rotate.
- [ ] **Step 2: Check in `yarn tauri dev`.**
  - The gear sits dead center, and its button is the same size as the pin button.
  - Hovering rotates it in place with no wobble.
  - Check light/dark and ko/en.
- [ ] **Step 3: Verify**

Run: `yarn lint && yarn typecheck`
Expected: PASS

- [ ] **Step 4: Commit** `fix(panel): center the settings gear and spin it in place`

---

### Task 4: ICO/ICNS image targets replace BMP/TIFF

**Files:**
- Create: `src-tauri/src/domain/convert/icon.rs`
- Modify: `src-tauri/src/domain/convert/model.rs` (`Format`, `ext`, `IMAGE_TARGETS`)
- Modify: `src-tauri/src/domain/convert/image.rs` (both `convert_image`s)
- Modify: `src-tauri/src/domain/convert/mod.rs` (declare `mod icon;`, fix the target test at line ~584)
- Modify: `src/domain/convert/apis/convert.dto.ts` (`Format` union)

**Interfaces:**
- Produces:
  - `Format::Ico` / `Format::Icns`, with serde and `ext` values `"ico"` / `"icns"`.
  - `pub fn write_icon(img: ::image::DynamicImage, to: Format, out: &Path) -> Result<(), JobError>` in `icon.rs`.
  - Pure helpers `fn ico_bytes(src: &RgbaImage) -> Result<Vec<u8>, JobError>` and `fn icns_bytes(src: &RgbaImage) -> Result<Vec<u8>, JobError>`.

- [ ] **Step 1: Write failing tests**

`icon.rs` tests:

```rust
fn img(w: u32, h: u32) -> RgbaImage { RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 255])) }

#[test]
fn ico_holds_every_size_with_256_written_as_zero() {
    let b = ico_bytes(&img(300, 120)).unwrap(); // non-square
    assert_eq!(&b[..6], &[0, 0, 1, 0, 7, 0]); // reserved, type 1, 7 images
    let widths: Vec<u8> = (0..7).map(|i| b[6 + i * 16]).collect();
    assert_eq!(widths, [16, 24, 32, 48, 64, 128, 0]);
    let first = u32::from_le_bytes(b[18..22].try_into().unwrap()) as usize; // entry 0 offset
    assert_eq!(&b[first..first + 8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn icns_lists_png_blocks_and_its_own_length() {
    let b = icns_bytes(&img(1, 1)).unwrap(); // tiny
    assert_eq!(&b[..4], b"icns");
    assert_eq!(u32::from_be_bytes(b[4..8].try_into().unwrap()) as usize, b.len());
    let mut types = vec![];
    let mut at = 8;
    while at < b.len() {
        types.push(std::str::from_utf8(&b[at..at + 4]).unwrap().to_owned());
        at += u32::from_be_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
    }
    assert_eq!(types, ["icp4", "icp5", "ic07", "ic08", "ic09", "ic10", "ic11", "ic12", "ic13", "ic14"]);
}

#[test]
fn padding_is_transparent_and_centered() {
    let sq = square(&img(4, 2));
    assert_eq!(sq.dimensions(), (4, 4));
    assert_eq!(sq.get_pixel(0, 0)[3], 0);
    assert_eq!(sq.get_pixel(0, 1)[3], 255);
}
```

In the `mod.rs` tests, update the existing assertion at ~584 to the new order. Then add:

```rust
#[test]
fn mixed_images_offer_icons_but_not_bmp_or_tiff() {
    let f = formats(&["a.png", "b.jpg", "c.bmp"]); // use the module's existing helper/caps
    assert!(f.contains(&Format::Ico) && f.contains(&Format::Icns));
    assert!(!f.contains(&Format::Bmp) && !f.contains(&Format::Tiff));
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib convert`
Expected: compile errors (`Format::Ico` and `icon.rs` items are missing).

- [ ] **Step 3: Implement**
  - `IMAGE_TARGETS: [Format; 8] = [Png, Jpg, Webp, Heic, Ico, Icns, Gif, Pdf]`. Keep `Tiff`/`Bmp` in `Format` and `from_ext`, since they are still sources.
  - Remove the Windows `Format::Tiff`/`Bmp` encode arms. The macOS `sips` arms for them become unreachable, so remove those too.
  - `icon.rs`:
    - `fn square(src: &RgbaImage) -> RgbaImage` makes a transparent canvas of side `max(w, h)` with `src` centered.
    - Each size is `imageops::resize(.., FilterType::Lanczos3)`, then PNG-encoded with `PngEncoder`.
    - ICO: `ICONDIR` (6 bytes) + 16-byte `ICONDIRENTRY` per size (w, h, 0 colors, 0, planes 1, bpp 32, size LE, offset LE) + PNG data, sizes `[16, 24, 32, 48, 64, 128, 256]`. The 256 entry writes width and height 0.
    - ICNS: `b"icns"` + total length BE. Each block is OSType + (8 + data) length BE + PNG, in order `icp4 16, icp5 32, ic07 128, ic08 256, ic09 512, ic10 1024, ic11 32, ic12 64, ic13 256, ic14 512`. Resize once per distinct pixel size.
  - `image.rs`:
    - macOS: `Format::Ico | Format::Icns` runs `sips` to a `vee-tmp.png` (same shape as the Webp arm), then `open_upright` and `icon::write_icon`, and removes the temp file.
    - Windows: `Format::Ico | Format::Icns => return icon::write_icon(img, to, out)`.
  - TS `Format` union: replace `'tiff'`/`'bmp'` with `'ico'`/`'icns'`. If the type checker shows `tiff`/`bmp` are used elsewhere, keep them in the union.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib convert && yarn typecheck`
Expected: PASS

- [ ] **Step 5: Manual check (macOS)**
  - Drop a JPG and a PNG on the convert donut. ICO and ICNS slices show, and BMP and TIFF are gone.
  - Make an `.icns` and check that Finder Quick Look shows it.
  - `file out.ico` reports `MS Windows icon resource - 7 icons`.

- [ ] **Step 6: Commit** `feat(convert): ICO and ICNS image targets in place of BMP and TIFF`

---

### Task 5: Korean-friendly LibreOffice profile

**Files:**
- Modify: `src-tauri/src/domain/convert/office.rs`

**Interfaces:**
- Produces:
  - `pub fn registry_xcu(macos: bool) -> String`
  - `pub fn write_profile(profile: &Path, macos: bool) -> std::io::Result<()>`, which writes `<profile>/user/registrymodifications.xcu`. `convert_lo` calls it before `exec`; on error it logs and goes on.

- [ ] **Step 1: Confirm the real keys.**
  - Run `soffice -env:UserInstallation=file:///<scratchpad>/lo-probe` with the GUI.
  - In Tools ▸ Options:
    - Language Settings: set the locale to Korean and the Asian default to Korean.
    - Fonts: Replacement Table on, with one `Malgun Gothic → Apple SD Gothic Neo` row, "Always" on and "Screen only" off.
  - Quit, then read `<scratchpad>/lo-probe/user/registrymodifications.xcu`. Copy the exact `<item oor:path=…>` lines for the locale and for `/org.openoffice.Office.Common/Font/Substitution` (`Replacement`, `FontPairs/_N` with `ReplaceFont`, `SubstituteFont`, `Always`, `OnScreenOnly`).

- [ ] **Step 2: Build Korean samples and baselines** in the scratchpad, not the repo.
  - Make one each of docx, pptx and xlsx with mixed Korean/English paragraphs, a table and several pages, in 맑은 고딕, 굴림, 돋움, 바탕 and 궁서.
  - Export each to PDF from Word, PowerPoint and Excel on this Mac; these are the baselines.
  - Convert each with today's `soffice` and the empty profile.
  - Note the page counts, any □, and the line breaks.

- [ ] **Step 3: Write failing tests**

```rust
#[test]
fn registry_sets_korean_and_replaces_ms_korean_fonts_on_macos() {
    let x = registry_xcu(true);
    assert!(x.contains("ko-KR"));
    for f in ["Malgun Gothic", "맑은 고딕", "Gulim", "굴림", "Dotum", "돋움", "Batang", "바탕", "Gungsuh", "궁서"] {
        assert!(x.contains(&format!("<value>{f}</value>")), "{f}");
    }
}

#[test]
fn windows_registry_sets_korean_without_font_pairs() {
    let x = registry_xcu(false);
    assert!(x.contains("ko-KR") && !x.contains("FontPairs"));
}

#[test]
fn write_profile_overwrites_what_libreoffice_left() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("user/registrymodifications.xcu");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "old").unwrap();
    write_profile(dir.path(), true).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), registry_xcu(true));
}
```

- [ ] **Step 4: Run them and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib office`
Expected: compile errors (the functions are missing).

- [ ] **Step 5: Implement.**
  - `registry_xcu` builds the file from the Step 1 lines:
    - the locale is always ko-KR;
    - on macOS only, a `Replacement = true` item and one `FontPairs/_N` node per pair (`Always` true, `OnScreenOnly` false).
  - Pairs, each with its Korean and English name:
    - 맑은 고딕 / Malgun Gothic, 굴림 / Gulim, 새굴림 / New Gulim, 돋움 / Dotum → Apple SD Gothic Neo
    - 바탕 / Batang, 궁서 / Gungsuh → AppleMyungjo
  - Call `write_profile(profile, cfg!(target_os = "macos"))` at the top of `convert_lo`.
  - Add a one-line comment saying LibreOffice rewrites the file on exit, which is why it is written every run.

- [ ] **Step 6: Run the tests and confirm they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib office`
Expected: PASS

- [ ] **Step 7: Compare and tune.**
  - Re-convert the Step 2 samples with the new profile, through the app (`yarn tauri dev`) or `soffice` with a copied profile.
  - Check against the baselines: there must be no □ and the page counts must match; then compare line breaks by eye.
  - If AppleMyungjo or Apple SD Gothic Neo shifts lines more than another installed font (for example 나눔고딕, 나눔명조), switch the target. Note the result (fonts tried, page counts) in the commit body.
  - If □ remains even with replacement on, find the cause (missing CJK fallback, locale key not taking effect) before going further.
  - Font metrics differ, so line positions won't match the original exactly.

- [ ] **Step 8: Commit** `fix(convert): Korean locale and font replacements for LibreOffice`

---

### Task 6: Whole-branch verification

- [ ] **Step 1:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib && yarn test && yarn typecheck && yarn lint`. Everything passes.
- [ ] **Step 2:** In `yarn tauri dev`, check the gear (Task 3), the ICO/ICNS slices (Task 4) and one Korean docx → PDF (Task 5), in light/dark and ko/en.
- [ ] **Step 3: Updater.**
  - The updater is off in debug builds, so the re-check path cannot be run locally.
  - Re-read `updater.rs` against the spec's anti-goals:
    - no `Checking` while ready;
    - an offline re-check still installs;
    - every swap of `pending` happens under `busy`.
  - Note in the PR that it needs a real check against two consecutive releases.
