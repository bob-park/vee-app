# Image Editor, Light Panel and Settings Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the panel readable on light/dark pages, stop the settings previews shrinking on macOS Sonoma, and add an image editor window (crop, rotate, resize, background removal, upscale) opened from the convert donut.

**Architecture:** Pixel work runs in Rust. A pure `domain/edit` renders an ordered list of ops with the `image` crate; `app/ai.rs` runs the two ONNX models through `ort` (ONNX Runtime loaded at runtime from the downloaded engine dir); `app/editor.rs` owns the new `editor` window, its session, an AI-result cache, and the copy-to-history path. The frontend `editor` screen holds the op stack, previews through a `render_edit` command, and compares AI results with daisyUI `diff`. Models download through the existing engines system as two new engines.

**Tech Stack:** Tauri 2, Rust (`image` 0.25, `ort` 2.0 rc with `load-dynamic`), React 19 + TypeScript, Tailwind 4 + daisyUI 5, React Query, `node:test`.

**Spec:** `docs/superpowers/specs/2026-10-29-image-editor-and-light-panel-design.md`

## Global Constraints

- Work on the current branch `feature/settings-titlebar-panel-file-stack`. Commit per task, `<prefix>: <subject>` (`docs/agents/workflow/commit-messages.md`), ending with the session's attribution lines.
- Colors only in `src/app/globals.css`, both `light` and `dark` blocks. No hex/rgb in components.
- Every user-visible string in `src/shared/i18n/en.ts` and `ko.ts`.
- `domain/**` and `shared/**` never `use tauri`. Commands stay thin (`docs/agents/conventions/tauri-commands.md`). Errors are `AppError` (`docs/agents/conventions/error-handling.md`).
- Dependency changes: only `ort` is approved. Any other new crate or npm package → ask the user first.
- Model licenses: commercial use allowed (MIT/Apache/BSD). No RMBG-1.4/2.0.
- Everything runs on device; no image leaves the machine.
- Upscale: 2× / 4×, default 2×; output long side capped at 4096 px (downscale with Lanczos3 when over).
- Rotation is ±90° only. Resize keeps aspect ratio by default.
- Copy writes PNG to the clipboard, inserts a new history clip directly (the watcher ignores Vee's own writes via `suppress_watcher`), leaves the original clip untouched, then hides the editor.
- Before each commit that touches UI: look at it in `yarn tauri dev` in light/dark and ko/en (installed Vee present → separate identifier, see `docs/agents/build-run.md`).
- Done = `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` all pass.

## Review Focus

1. **Huge source image** (e.g. a 6016×3384 Retina screenshot): editor opens without freezing, previews are downsized, 4× upscale is capped to 4096 long side, background removal doesn't exhaust memory. → tests in Task 3 (`upscale_size_caps_long_side`) and Task 6 (`preview_is_downsized`).
2. **Invalid crop or resize input** (zero, negative, larger than image, empty field): never panics; crop is clamped into the image, resize below 1 px or above 16384 px is rejected with `AppError::InvalidInput`. → Task 3 tests.
3. **Source clip deleted or retention-pruned while the editor is open:** the editor keeps its own decoded copy, so copy still works. → Task 6 `session_keeps_source_after_clip_delete`.
4. **Model missing, removed or corrupt mid-session:** the AI tab shows the "download in Settings" prompt instead of an error loop; a load failure surfaces as `AppError::Other` and a toast, not a crash. → Task 5 `missing_model_is_reported`.
5. **Second edit opened while one is open:** the new drop replaces the session and the window reloads; no mixed state from the old image. → Task 6 `open_replaces_session`.

---

## File Structure

| Path | Responsibility |
| ---- | -------------- |
| `src/app/globals.css` | new `--vee-panel` token, daisyUI `diff` enabled |
| `src/app/panel/PanelApp.tsx`, `_components/Card.tsx` | use `bg-panel`, stronger watermark |
| `src/app/settings/_components/ThemePicker.tsx`, `PositionPicker.tsx` | fixed-size previews |
| `src-tauri/src/domain/edit/{mod.rs,model.rs}` | `Op`, validation, pure geometric render, upscale fallback, size math |
| `src-tauri/src/app/ai.rs` | `ort` init from engine dir, `remove_background`, `upscale` |
| `src-tauri/src/app/editor.rs` | editor window, session, AI cache, commands (`get_edit_source`, `render_edit`, `copy_edit`, `close_editor`) |
| `src-tauri/src/app/engines.rs` | engines `bgremove`, `upscale`; pinned releases; new hosts |
| `src-tauri/src/domain/convert/{model.rs,mod.rs}`, `app/convert.rs` | `Format::Edit` slice; drop on it opens the editor |
| `src-tauri/tauri.conf.json`, `capabilities/default.json`, `lib.rs` | `editor` window, registration |
| `src/domain/edit/{apis,queries,components,store}` | DTOs, `call()` wrappers |
| `src/utils/edit.ts` (+ `.test.ts`) | pure op-stack / crop / resize / zoom math |
| `src/app/editor/EditorApp.tsx`, `_components/*` | the editor screen |
| `src/shared/components/RotateIcon.tsx` | two-tone rotate icon |
| `src/app/settings/_components/ConvertSection.tsx` | two model `EngineSection`s |
| `src/shared/i18n/{en,ko}.ts` | all strings |

---

### Task 1: Panel stands out on light and dark pages

**Files:**
- Modify: `src/app/globals.css` (light `[data-theme]` block ~line 100, dark ~line 130, `@theme inline`)
- Modify: `src/app/panel/PanelApp.tsx:531`
- Modify: `src/app/panel/_components/Card.tsx:355`

**Interfaces:**
- Produces: utility class `bg-panel` (token `--vee-panel` → `--color-panel`).

- [ ] **Step 1:** Add `--vee-panel` to both theme blocks — light `#eeedf6`, dark `#1a1b22` — and expose it as `--color-panel: var(--vee-panel)` in `@theme inline`. Leave `base-100` alone (settings and what's-new use it).
- [ ] **Step 2:** In `PanelApp.tsx:531` swap `bg-base-100` for `bg-panel`.
- [ ] **Step 3:** In `Card.tsx:355` change the watermark opacity `opacity-16 … dark:opacity-22` to `opacity-45 [:root[data-theme=dark]_&]:opacity-50`. Shape, size and mask stay.
- [ ] **Step 4:** `yarn tauri dev`. Open the panel over a white web page and a black one, in light and dark. Expected: panel edge visible on both; app icons recognisable; glass cards still read. Tune the two token values (not the recipe) if an edge disappears; update `docs/design/aurora-glass.md` §1 table with the new token.
- [ ] **Step 5:** `yarn typecheck && yarn lint`, then commit `style(panel): lift the panel off light and dark pages`.

### Task 2: Settings previews keep their size on macOS Sonoma

**Files:**
- Modify: `src/app/settings/_components/ThemePicker.tsx`
- Modify: `src/app/settings/_components/PositionPicker.tsx`

- [ ] **Step 1:** Use superpowers:systematic-debugging. If a Sonoma machine/VM is available, run `yarn tauri dev` there and inspect the computed sizes of the radiogroup, buttons and preview spans in Web Inspector to confirm which box collapses. If not, reason from the markup: `ThemePicker` root is a shrinkable flex item in a `justify-between` row; the preview span has height but no width; `PositionPicker`'s preview gets its height only from `aspect-[16/10]` on a stretched flex child of a `<button>`.
- [ ] **Step 2:** Make sizes explicit so no WebKit can shrink them:
  - `ThemePicker`: root `flex shrink-0 gap-2`; button `w-[74px] shrink-0`; preview span `block h-12 w-full`.
  - `PositionPicker`: button `w-[84px]`; preview span `block w-full h-[52px]` (84 × 10/16 ≈ 52, replaces `aspect-[16/10]`).
- [ ] **Step 3:** On macOS 27, compare a screenshot of Settings › General and › Panel before and after. Expected: identical pixels. If Sonoma is available, expected there: same size as on 27.
- [ ] **Step 4:** Commit `fix(settings): keep theme and position previews full size on older webkit`.

### Task 3: Pure edit ops in Rust

**Files:**
- Create: `src-tauri/src/domain/edit/mod.rs`, `src-tauri/src/domain/edit/model.rs`
- Modify: `src-tauri/src/domain/mod.rs` (`pub mod edit;`)

**Interfaces:**
- Produces (in `domain::edit`):
  - `#[serde(tag = "type", rename_all = "camelCase")] pub enum Op { Crop { x: u32, y: u32, w: u32, h: u32 }, Rotate { quarter: i8 /* -1 | 1 */ }, Resize { w: u32, h: u32 }, RemoveBg, Upscale { scale: u8 /* 2 | 4 */ } }`
  - `pub const MAX_SIDE: u32 = 16384; pub const UPSCALE_CAP: u32 = 4096; pub const PREVIEW_SIDE: u32 = 2048;`
  - `pub fn apply_geometric(img: DynamicImage, op: &Op) -> AppResult<DynamicImage>` — Crop (clamped into bounds; empty after clamp → `InvalidInput`), Rotate (`rotate90`/`rotate270`), Resize (`resize_exact`, Lanczos3; `w`/`h` 0 or > `MAX_SIDE` → `InvalidInput`). AI ops → `InvalidInput("not geometric")`.
  - `pub fn upscale_size(w: u32, h: u32, scale: u8) -> (u32, u32)` — `w*scale, h*scale`, scaled down proportionally so the long side ≤ `UPSCALE_CAP`.
  - `pub fn upscale_fallback(img: &DynamicImage, scale: u8) -> DynamicImage` — Lanczos3 to `upscale_size`, then `imageops::unsharpen(sigma 1.0, threshold 2)`.
  - `pub fn fit(w: u32, h: u32, side: u32) -> (u32, u32)` — downsize so long side ≤ `side`, never upsize.
  - `pub fn to_png(img: &DynamicImage) -> AppResult<Vec<u8>>`.
- The `image` dependency is per-target in `Cargo.toml`; both targets already have `png`. Do not add features.

- [ ] **Step 1: Write the failing tests** (`#[cfg(test)] mod tests` in `mod.rs`):

```rust
#[test] fn crop_is_clamped_into_the_image() { /* 100×50, Crop{x:80,y:40,w:50,h:50} → 20×10 */ }
#[test] fn crop_outside_the_image_is_rejected() { /* Crop{x:200,..} → Err(InvalidInput) */ }
#[test] fn rotate_swaps_sides() { /* 100×50, quarter 1 → 50×100; quarter -1 → 50×100, pixel (0,0) lands where rotate270 puts it */ }
#[test] fn resize_rejects_zero_and_huge() { /* w 0 → Err; w 16385 → Err; 40×20 → Ok 40×20 */ }
#[test] fn upscale_size_caps_long_side() { assert_eq!(upscale_size(420,180,2),(840,360)); assert_eq!(upscale_size(3000,1000,4),(4096,1365)); }
#[test] fn fit_never_upsizes() { assert_eq!(fit(100,50,2048),(100,50)); assert_eq!(fit(6016,3384,2048),(2048,1152)); }
#[test] fn upscale_fallback_hits_target_size() { /* 10×5, scale 2 → 20×10 */ }
#[test] fn op_serde_matches_the_wire() { /* {"type":"rotate","quarter":-1} and {"type":"removeBg"} round-trip */ }
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --lib domain::edit` → FAIL (module missing).
- [ ] **Step 3:** Implement the interfaces above.
- [ ] **Step 4:** Same command → PASS. Run the tauri-import grep from `rust-modules.md` → empty.
- [ ] **Step 5:** Commit `feat(edit): add the image edit ops`.

### Task 4: Pick and pin the models (spike, then engines)

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `ort`)
- Modify: `src-tauri/src/app/engines.rs`
- Create: `src-tauri/src/app/ai.rs` (spike only in this task: an `#[ignore]` benchmark test)

**Interfaces:**
- Produces: `EngineId::{Bgremove, Upscale}` (serde `"bgremove"`, `"upscale"`), `EngineId::ALL` of length 4, `SPECS` entries, `pub fn model_dir(app: &AppHandle, id: EngineId) -> Option<PathBuf>` (installed dir or `None`), and the pinned constants below. Frontend `EngineId` in `convert.dto.ts` gains the two values (done in Task 9).

- [ ] **Step 1: Add `ort`.** Check the current 2.0 release with `cargo search ort` and the `/pykeio/ort` docs via ctx7 (API differs between rcs). Add under `[dependencies]`: `ort = { version = "=<that version>", default-features = false, features = ["std", "load-dynamic"] }` (add `"ndarray"` only if the chosen version needs it for tensors). Note the ONNX Runtime version that `ort` release targets.
- [ ] **Step 2: Spike the candidates.** Write `app::ai::tests::bench_models` (`#[ignore]`), modelled on `app::ocr::tests::bench`: reads `VEE_AI_BENCH_DIR` (product photos + screenshots), runs each candidate, writes outputs next to the inputs, prints ms per image.
  - Background removal candidates (from `https://github.com/danielgatis/rembg/releases/download/v0.0.0/`): `isnet-general-use.onnx` (Apache-2.0), `BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx` (MIT). Pick on product-photo edge quality, then size.
  - Upscale candidates: Real-ESRGAN (BSD-3) ONNX exports — `realesr-general-x4v3` and `RealESRGAN_x4plus`. Run on 12–13 px Korean and English UI screenshots at 2× and 4×.
  - **Upscale gate (spec):** a model passes only if no glyph is blurred into another or changed, judged side by side against the Lanczos fallback (`domain::edit::upscale_fallback`). Show the outputs to the user and record the verdict in the PR description.
  - If no upscale model passes: drop `EngineId::Upscale` from this plan (skip its parts in Tasks 5, 8, 9) — the Upscale tab uses `upscale_fallback` and needs no download.
- [ ] **Step 3: Write the failing tests** in `engines.rs` tests:

```rust
#[test] fn model_releases_are_valid() { for id in [EngineId::Bgremove, EngineId::Upscale] { assert!(pinned(id).unwrap().valid()); } }
#[test] fn model_release_has_runtime_and_model() { /* assets: one Archive (ONNX Runtime) + one Raw { name: "model.onnx" } */ }
#[test] fn engine_ids_round_trip() { assert_eq!(EngineId::parse("bgremove").unwrap(), EngineId::Bgremove); }
```

- [ ] **Step 4: Implement.**
  - `SPECS`: `bgremove` / `upscale`, `binaries: [<ORT lib>, "model.onnx"]` where `<ORT lib>` is the real (not symlink) file name in the ORT archive: macOS `libonnxruntime.<ver>.dylib` from `onnxruntime-osx-arm64-<ver>.tgz`, Windows `onnxruntime.dll` from `onnxruntime-win-x64-<ver>.zip`.
  - `fn pinned(id) -> Option<Release>`: constant `Release { version: "1", assets: [ORT archive, model Raw] }` with URL, sha256, size you measured in the spike. `resolve()` returns `pinned(id)` for the model engines (no network lookup). Bump `version` when a pinned file changes; that drives the existing update badge.
  - `SOURCES` += `"https://github.com/microsoft/onnxruntime/releases/download/"`, `"https://github.com/danielgatis/rembg/releases/download/"`, and the upscale model's host (a `huggingface.co/<repo>/resolve/<commit>/` prefix pinned to a commit if that is where it lives).
  - Make sure `AssetKind::Raw` installs without an executable bit requirement and drop its `cfg_attr(not(test), allow(dead_code))`.
  - Both model engines download their own copy of ORT (~30 MB each). `// ponytail: ORT shipped per model engine; share one runtime engine if a third model arrives.`
- [ ] **Step 5:** `cargo test … --lib app::engines` → PASS.
- [ ] **Step 6:** Commit `feat(engines): add background removal and upscale model downloads` (Cargo.toml + lock included).

### Task 5: Run the models

**Files:**
- Modify: `src-tauri/src/app/ai.rs`
- Modify: `src-tauri/src/app/mod.rs`

**Interfaces:**
- Consumes: `engines::model_dir`, `SPECS[id].binaries`, `domain::edit::{upscale_size, UPSCALE_CAP}`.
- Produces:
  - `pub fn remove_background(app: &AppHandle, img: &DynamicImage) -> AppResult<DynamicImage>` — RGBA, alpha from the model mask resized back to the source size.
  - `pub fn upscale(app: &AppHandle, img: &DynamicImage, scale: u8) -> AppResult<DynamicImage>` — model output (×4) resized with Lanczos3 to `upscale_size(w, h, scale)`. If `EngineId::Upscale` was dropped in Task 4, this calls `domain::edit::upscale_fallback` and needs no model.
  - `AppError::Other("model missing")` when the engine isn't installed (frontend checks status first; this is the backstop).

- [ ] **Step 1: Write the failing tests:** `missing_model_is_reported` (a temp app-data dir without the engine → `remove_background` errors, no panic) — test the plain helper `fn session_paths(dir: &Path, id: EngineId) -> AppResult<(PathBuf, PathBuf)>` rather than the `AppHandle` wrapper. Also `mask_alpha_is_resized_to_source` on a fake 4×4 mask → 8×8 alpha, testing the pure `fn apply_mask(img: &RgbaImage, mask: &GrayImage) -> RgbaImage`.
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3: Implement.**
  - `ort::init_from(<ORT lib path>)` once per process (`OnceLock`). Whichever model engine is first supplies the dylib; both pin the same ORT version.
  - Sessions cached in a `Mutex<HashMap<EngineId, Session>>`, dropped when `engines::remove` runs for that id.
  - Pre/post-processing (input size, normalization, NCHW layout) follows the chosen model's card; keep it in two small pure functions (`fn to_input(…) -> Array`, `fn from_output(…) -> Image`) with a test each on a 2×2 image.
  - Upscale runs in 128-px tiles with 16-px overlap so a 4096-side output fits in memory.
- [ ] **Step 4:** Run → PASS. Run the Task 4 bench with the pinned models: both produce the spike's outputs.
- [ ] **Step 5:** Commit `feat(ai): run background removal and upscaling on device`.

### Task 6: Editor window, session and copy-to-history

**Files:**
- Create: `src-tauri/src/app/editor.rs`
- Modify: `src-tauri/src/app/mod.rs`, `src-tauri/src/lib.rs` (manage state, register commands, `CloseRequested` → `editor::close`), `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/app/window.rs` (`follow_active_space` list += `EDITOR`; make `show_toast`/`ToastPayload::copied` reachable)

**Interfaces:**
- Consumes: `domain::edit::*`, `ai::{remove_background, upscale}`, `clip::upsert`, `NewClip::Image`, `window::show_toast`, `AppState::suppress_watcher`.
- Produces:
  - `pub const EDITOR: &str = "editor";`
  - `pub fn open(app: &AppHandle, source: PathBuf) -> AppResult<()>` — decodes the PNG into the session (replacing any open one), emits `editor://opened`, shows and focuses the window.
  - Commands (camelCase on the wire):
    - `get_edit_source() -> EditSourceDto { width: u32, height: u32, dataUrl: String }` (preview fitted to `PREVIEW_SIDE`).
    - `async render_edit(ops: Vec<Op>) -> AppResult<RenderDto { width, height, dataUrl }>` — full-res render, PNG of `fit(…, PREVIEW_SIDE)` returned; `width/height` are full-res.
    - `async copy_edit(ops: Vec<Op>) -> AppResult<()>` — full-res render → `clip::upsert(NewClip::Image { png, thumb_png, width, height }, app_id: None, now)` → write PNG to clipboard (suppressed like `write_clipboard`) → emit `clips://changed` → copy sound + `ToastPayload::copied` → hide editor.
    - `close_editor()` — drops the session, hides.
  - Window config: label `editor`, `create: false`, `visible: false`, `titleBarStyle: "Overlay"`, `hiddenTitle: true`, `width: 760`, `height: 600`, `minWidth: 560`, `minHeight: 440`, `resizable: true`, `center: true`. Add `"editor"` to the capability's `windows`.
- Render pipeline: walk `ops`; geometric ops call `apply_geometric`; AI ops look up `cache: HashMap<u64, Arc<DynamicImage>>` keyed by a hash of (`session id`, ops prefix including this op) and call `ai::*` on a miss. AI calls run in `tauri::async_runtime::spawn_blocking`. `// ponytail: cache only grows within one session; cleared on open/close.`

- [ ] **Step 1: Write the failing tests** on the plain parts (`fn render(session: &Session, ops: &[Op], ai: impl Fn(&DynamicImage, &Op) -> AppResult<DynamicImage>) -> AppResult<DynamicImage>` takes the AI step as a closure so tests pass a fake):

```rust
#[test] fn geometric_ops_apply_in_order() { /* 100×50: Crop 50×50 → Rotate 1 → Resize 20×20 → 20×20 */ }
#[test] fn ai_result_is_cached_for_the_same_prefix() { /* fake ai counts calls; render twice → 1 call; append Rotate → still 1 */ }
#[test] fn changed_prefix_reruns_ai() { /* different Crop before RemoveBg → 2 calls */ }
#[test] fn preview_is_downsized() { /* 6016×3384 → preview 2048×1152, dto width/height 6016×3384 */ }
#[test] fn open_replaces_session() { /* second open clears cache and source */ }
#[test] fn session_keeps_source_after_clip_delete() { /* source decoded into memory; deleting the file doesn't break render */ }
```

- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(editor): add the editor window and copy to history`.

### Task 7: "Edit" slice on the donut

**Files:**
- Modify: `src-tauri/src/domain/convert/model.rs`, `src-tauri/src/domain/convert/mod.rs` (`formats_for`), `src-tauri/src/app/convert.rs` (`open_at`, `drop_on`), `src/domain/convert/apis/convert.dto.ts`, `src/utils/formatLabel.ts` (+ test), `src/app/convert/ConvertApp.tsx`

**Interfaces:**
- Produces: `Format::Edit` (serde `"edit"`), first slice for a single image source. TS `Format` gains `'edit'`.

- [ ] **Step 1: Write the failing tests:**
  - Rust `formats_for`: one PNG → slices start with `Edit` then `IMAGE_TARGETS` (minus own); two PNGs → no `Edit`; a video → no `Edit`.
  - Rust `take_drop` with `"edit"` → returns the session without needing a folder.
  - TS `formatLabel.test.ts`: `formatLabel('edit')` is not used for display — instead add `isEdit(f: Format): boolean` and test it.
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3: Implement.**
  - `formats_for`: prepend `Slice { format: Format::Edit, needs: None }` when `paths.len() == 1` and the path is an image source.
  - `open_at`: when the cursor isn't over a folder (`NotFolder` / `FinderBlocked`) and the session has an `Edit` slice, open the donut with **only** the `Edit` slice instead of the "convert only over a folder" toast. Over a folder, all slices as today.
  - `drop_on("edit")`: don't queue a job; call `editor::open(app, s.files[0].clone())`.
  - `ConvertApp`: the `edit` slice shows the localized label `t.convert.edit` (pencil glyph optional) instead of `formatLabel`.
- [ ] **Step 4:** Run tests → PASS. `yarn tauri dev`: drag an image card with the convert key over a folder (edit + formats) and over the desktop/a browser (edit only); drop on edit → editor opens.
- [ ] **Step 5:** Commit `feat(convert): open the editor from the donut`.

### Task 8: Editor screen — crop, rotate, resize, copy

**Files:**
- Create: `src/domain/edit/apis/edit.dto.ts`, `src/domain/edit/apis/edit.ts`, `src/domain/edit/{queries,components,store}/.gitkeep`
- Create: `src/utils/edit.ts`, `src/utils/edit.test.ts`
- Create: `src/app/editor/EditorApp.tsx`, `src/app/editor/_components/{CropBox.tsx,RotatePanel.tsx,ResizeForm.tsx,Footer.tsx}`
- Create: `src/shared/components/RotateIcon.tsx`
- Modify: `src/app/main.tsx` (route `editor`), `src/shared/i18n/{en,ko}.ts`

**Interfaces:**
- `edit.dto.ts`: `type EditOp = { type: 'crop'; x; y; w; h } | { type: 'rotate'; quarter: -1 | 1 } | { type: 'resize'; w; h } | { type: 'removeBg' } | { type: 'upscale'; scale: 2 | 4 }`; `interface EditImage { width: number; height: number; dataUrl: string }`.
- `edit.ts`: `getEditSource(): Promise<EditImage>`, `renderEdit(ops): Promise<EditImage>`, `copyEdit(ops): Promise<void>`, `closeEditor(): Promise<void>`.
- `utils/edit.ts` (pure, `import type` only):
  - `cropFromDrag(start: Pt, end: Pt, view: Size, image: Size): Rect` — view px → image px, normalised, clamped.
  - `resizeKeep(prev: Size, next: Partial<Size>, locked: boolean): Size` — rounds, min 1.
  - `sizeAfter(base: Size, ops: EditOp[]): Size` — mirrors Rust for the footer's `a × b → c × d` (upscale uses the same cap rule).
- `RotateIcon({ dir: 'ccw' | 'cw' })`: the two-tone icon from `rotate-icons-v2.html` option 3 (rounded square with `fill-current opacity-22` + 1.7 stroke, arrow over the top), `currentColor` only.

- [ ] **Step 1: Write the failing tests** (`src/utils/edit.test.ts`):

```ts
test('cropFromDrag maps view to image px and clamps', () => {
  assert.deepEqual(cropFromDrag({ x: 10, y: 10 }, { x: 60, y: 35 }, { w: 100, h: 50 }, { w: 1000, h: 500 }), { x: 100, y: 100, w: 500, h: 250 });
  assert.deepEqual(cropFromDrag({ x: 90, y: 40 }, { x: 200, y: 99 }, { w: 100, h: 50 }, { w: 1000, h: 500 }), { x: 900, y: 400, w: 100, h: 100 });
});
test('resizeKeep follows the ratio when locked', () => {
  assert.deepEqual(resizeKeep({ w: 1280, h: 720 }, { w: 800 }, true), { w: 800, h: 450 });
  assert.deepEqual(resizeKeep({ w: 1280, h: 720 }, { w: 800 }, false), { w: 800, h: 720 });
});
test('sizeAfter mirrors rotate and the upscale cap', () => {
  assert.deepEqual(sizeAfter({ w: 420, h: 180 }, [{ type: 'upscale', scale: 2 }]), { w: 840, h: 360 });
  assert.deepEqual(sizeAfter({ w: 3000, h: 1000 }, [{ type: 'upscale', scale: 4 }]), { w: 4096, h: 1365 });
  assert.deepEqual(sizeAfter({ w: 100, h: 50 }, [{ type: 'rotate', quarter: 1 }]), { w: 50, h: 100 });
});
```

- [ ] **Step 2:** `yarn test` → FAIL. **Step 3:** Implement `utils/edit.ts` → PASS.
- [ ] **Step 4: Build the screen** (section order per `react-sections.md`; layout per `editor-layout.html` A):
  - Top: `<Aurora>` + glass tab bar reusing the settings `Segmented` look (promote `Segmented` to `src/shared/components/` first if imported from another screen): crop / rotate / resize / removeBg / upscale.
  - State: `ops: EditOp[]` (applied), `pending: EditOp | null` (the current tab's unapplied op), `preview: EditImage`. Every change to `ops` calls `renderEdit(ops)`.
  - Crop: `CropBox` drag + 8 handles over the preview; outside dimmed; "Apply" appends the crop.
  - Rotate: two buttons `RotateIcon` + "-90°" / "+90°" (`rotate-icons.html` B) that append immediately.
  - Resize: two number inputs + lock toggle (default on) + "Apply". Invalid (empty, < 1, > 16384) disables Apply with an inline hint.
  - Footer: `sizeAfter` text left; Cancel / Copy right (Copy uses `from-primary to-sky`). Copy sends `[...ops, pending]` when a pending op exists.
  - Keys: Esc = cancel, ⌘/Ctrl+Enter = copy, ⌘/Ctrl+Z = pop the last op.
  - `editor://opened` → refetch source, reset `ops`.
  - Errors from `copyEdit`/`renderEdit` → toast line in the footer via `errorText`.
- [ ] **Step 5:** `yarn tauri dev`: crop a screenshot, rotate both ways, resize 800 px wide, copy, paste into another app; new card on top of the panel; original still there. Light/dark, ko/en.
- [ ] **Step 6:** `yarn typecheck && yarn lint && yarn test`, commit `feat(editor): crop, rotate and resize`.

### Task 9: AI tabs and model downloads in Settings

**Files:**
- Create: `src/app/editor/_components/{CompareView.tsx,MiniMap.tsx,ModelPrompt.tsx}`
- Modify: `src/app/editor/EditorApp.tsx`, `src/app/globals.css` (remove `diff` from daisyUI `exclude`), `src/utils/edit.ts` (+ test), `src/domain/convert/apis/convert.dto.ts` (`EngineId` += `'bgremove' | 'upscale'`), `src/app/settings/_components/ConvertSection.tsx`, `src/shared/i18n/{en,ko}.ts`, a "go to convert tab" path for settings (reuse `open_settings`; add an optional tab argument only if `open_settings` can't already land on a tab)

**Interfaces:**
- `utils/edit.ts` += `clampPan(pan: Pt, zoomed: Size, view: Size): Pt` and `viewportRect(pan: Pt, zoomed: Size, view: Size, map: Size): Rect` (for the mini-map).
- `CompareView({ before: string; after: string; zoom: 'fit' | 1 })` — daisyUI `diff` (`diff`, `diff-item-1`, `diff-item-2`, `diff-resizer`; check the daisyUI 5 docs via ctx7), both items share one `transform` for pan/zoom; labels "Original" / "Result"; checkerboard behind the result (a `bg-checker` utility in `globals.css`, both themes).

- [ ] **Step 1: Write the failing tests** for `clampPan` (can't pan past edges; fit → always 0,0) and `viewportRect` (half-size view → half-size rect).
- [ ] **Step 2:** `yarn test` → FAIL. **Step 3:** Implement → PASS.
- [ ] **Step 4: Build the tabs.**
  - Entering Remove background / Upscale: check `useEngineStatus(id)`. Not installed → `ModelPrompt` ("Download the model in Settings" + button opening Settings › Convert). Installed → `pending = { type: 'removeBg' }` / `{ type: 'upscale', scale: 2 }` and `renderEdit([...ops, pending])`; spinner overlay meanwhile, tabs and Copy disabled.
  - Upscale: 2× / 4× segmented (default 2×) + Fit / 100% (`upscale-diff.html` B) + `MiniMap` bottom-right while at 100%; drag to pan.
  - Remove background: `CompareView` only (`bg-remove.html` B).
  - "Apply" appends `pending` to `ops`.
  - If upscale fell back to Lanczos (Task 4), Upscale has no model prompt.
- [ ] **Step 5: Settings.** `<EngineSection engine="bgremove" index={3} run={run} />` and `engine="upscale"` (if kept), with i18n `caption`, `title`, `missingHint(mb)`, `removeConfirm` in both languages, plus `t.convert.edit`, every editor string, `needsModel`.
- [ ] **Step 6:** `yarn tauri dev`: download both models in Settings (progress, cancel, remove, re-download); remove background on a product photo; upscale a small screenshot 2× and 4×, compare at 100%, pan; copy and paste. Remove a model while the editor is open → the tab shows the prompt. Light/dark, ko/en.
- [ ] **Step 7:** Commit `feat(editor): background removal and upscale with compare view`.

### Task 10: Docs and final verification

**Files:**
- Modify: `docs/agents/overview.md` (six windows), `docs/agents/architecture.md` (editor window + AI engines), `docs/design/aurora-glass.md` (§6 editor window layout), `AGENTS.md` Foundations line ("its five windows" → six)

- [ ] **Step 1:** Update the docs above in the existing style.
- [ ] **Step 2:** Use superpowers:verification-before-completion. Run `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` → all pass. Re-run the Review Focus list by hand in `yarn tauri dev` (huge screenshot, bad inputs, deleted source, model removed, second drop).
- [ ] **Step 3:** If Windows isn't buildable here, say so for the PR (`rust-modules.md` platform rule).
- [ ] **Step 4:** Commit `docs: describe the editor window and model engines`.
