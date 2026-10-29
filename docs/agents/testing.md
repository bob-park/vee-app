---
title: Testing
scope: "**"
applies_to: writing tests, verifying a change before calling it done
related:
  - ./build-run.md
  - ./conventions/sqlite-migrations.md
  - ./conventions/module-aliases.md
---

# Testing

> Rust: `cargo test --manifest-path src-tauri/Cargo.toml --lib` (domain core, DB, migrations, errors, pure app-shell helpers). Frontend and scripts: `yarn test` (Node built-in `node:test`, no extra libraries). Plus `yarn typecheck` and `yarn lint`. All four must pass before a change is done.

| Command | Covers | Lives in |
| ------- | ------ | -------- |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | domain APIs, repositories, migrations and upgrades from every shipped version, `AppError` serialization, pure helpers in `app/` (drag images, watcher filtering, locale resolution, sounds, update status) | `#[cfg(test)] mod tests` at the bottom of each Rust file |
| `yarn test` | pure TS logic (`src/**/*.test.ts`) and scripts (`scripts/*.test.mjs`) | next to the file under test |
| `yarn typecheck` | types, including missing i18n keys | — |
| `yarn lint` | ESLint (bobpark config) over `src/` | — |

## What to test

- **Write the failing test first**, then the code (TDD).
- **Rust:**
  - Test through the domain's public API in `mod.rs`, against `db::open_in_memory()`.
  - Cover the rejection paths too: invalid input saves nothing, and unknown stored values fall back.
  - Pull decisions out of Tauri-dependent code into plain functions and test those (as `app/window.rs` does for drag file names and drag images).
- **First launch, reopen, upgrades:** use `tempfile::tempdir()` with `db::open(path)`. `shared/db.rs` keeps the shipped schemas in `legacy_schema` to build old databases.
- **Frontend:**
  - Extract decisions into pure functions in `src/utils/` (or `src/shared/i18n/locale.ts`) and test those, e.g. `resolveTheme`, `followSelection`, `parseNotes`, `resolveLocale`.
  - Components and hooks are not unit-tested; verify them in the running app.
- **Shell scripts:** run them against fixture copies in a temp dir. See `scripts/bump-version-up.test.mjs`.

## Constraints

- `node --test` runs TS by stripping types. It cannot resolve `@/`, so a tested file may use `@/…` only in `import type` lines ([module-aliases](./conventions/module-aliases.md)).
- **OCR benchmark:** `app::ocr::tests::bench` is `#[ignore]`d. Point `VEE_OCR_BENCH_DIR` at a folder of screenshots plus `expected.tsv` (`<file name>\t<word>,<word>,…`) and run `VEE_OCR_BENCH_DIR=… cargo test --manifest-path src-tauri/Cargo.toml --lib app::ocr::tests::bench -- --ignored --nocapture`. It prints time and missed words per file, then the Korean and other word hit rates and mean/max ms.
- **Background removal benchmark:** `app::ai::tests::bench_models` is `#[ignore]`d. It runs the real BiRefNet path on every `product-*.png` in `VEE_AI_BENCH_DIR`, with `VEE_AI_ORT` (the ONNX Runtime dylib) and `VEE_AI_MODEL` (the BiRefNet `.onnx`), and writes RGBA and on-magenta PNGs to `VEE_AI_OUT`: `VEE_AI_BENCH_DIR=… VEE_AI_ORT=…/libonnxruntime.dylib VEE_AI_MODEL=…/model.onnx VEE_AI_OUT=… cargo test --manifest-path src-tauri/Cargo.toml --lib app::ai::tests::bench_models -- --ignored --nocapture`. It prints size, time and RSS per image (the session is dropped after each run).
- **Tauri-dependent code** (`commands.rs`, the Tauri-facing parts of `app/*`) has no unit tests. Keep it thin and verify it in `yarn tauri dev`.
- **Engines:** ignored tests that need a real FFmpeg, rhwp, LibreOffice or the background removal model.
  - `app::engines::tests::installs_latest_ffmpeg` / `installs_latest_rhwp` look up the latest release and download it for real (network, ~56 MB / ~13 MB on macOS), then install it into a temp dir: `cargo test --manifest-path src-tauri/Cargo.toml --lib installs_latest_rhwp -- --ignored --nocapture`.
  - `app::engines::tests::installs_pinned_bgremove` downloads the pinned ONNX Runtime and BiRefNet model for real (~260 MB on macOS), installs them into a temp dir and loads the model with `ort`: `cargo test --manifest-path src-tauri/Cargo.toml --lib installs_pinned_bgremove -- --ignored --nocapture`.
  - `domain::convert::video::tests::converts_a_generated_clip` converts a generated clip to every video target at every quality, plus an odd-sized (321x241) source for mp4/mov/720p. Point `VEE_FFMPEG_DIR` at a folder with `ffmpeg` and `ffprobe` (it returns early without it): `VEE_FFMPEG_DIR=/opt/homebrew/bin cargo test --manifest-path src-tauri/Cargo.toml --lib converts_a_generated_clip -- --ignored --nocapture`.
  - `domain::convert::tests::converts_documents` (macOS): HWPX → PDF/HWP → HWPX/PDF with rhwp, DOCX → PDF/ODT and ODT → DOCX and TXT → DOCX with LibreOffice at its default path; checks the magic bytes. Point `VEE_RHWP_DIR` at a folder with `rhwp` and `VEE_HWP_SAMPLE` at any `.hwpx` (none is committed; it returns early without them): `VEE_RHWP_DIR=… VEE_HWP_SAMPLE=…/a.hwpx cargo test --manifest-path src-tauri/Cargo.toml --lib converts_documents -- --ignored --nocapture`.
- A smoke test that needs a desktop session (`front_app::reads_frontmost_app_and_icon`) is `#[ignore]`d. Run it by hand: `cargo test --manifest-path src-tauri/Cargo.toml -- --ignored reads_frontmost_app_and_icon`.
- **UI changes:** check them in the running app in light/dark and ko/en ([design](./design.md)).
