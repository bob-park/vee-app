# Editor Subject Select + Brush Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two tools to the editor's Remove background tab: Subject (SAM click-to-select) and Brush (erase/restore). Both sit in a left glass tool rail next to the existing Auto (BiRefNet).

**Architecture:** Two new `Op` variants ride the existing ops pipeline (`app/editor.rs::render`). `brush` is a local alpha op in `domain/edit`. `subject` is an AI op in `app/ai.rs`. Its SAM image embedding is cached per session, keyed by the ops *before* it, so a click only re-runs the small decoder. SAM is a new pinned model engine (`EngineId::Sam`) that is downloaded on its own, separate from BiRefNet.

**Tech Stack:** Rust (`image` 0.25, `ort` 2.0.0-rc.15 with the bundled ONNX Runtime), Tauri 2, React + TypeScript, Tailwind 4 / daisyUI 5, `node:test`.

**Spec:** `docs/superpowers/specs/2026-10-31-editor-bg-brush-subject-design.md` (Korean). Read it first; this plan only adds the decisions the spec left to the builder.

## Global Constraints

- No new dependencies in `package.json` or `src-tauri/Cargo.toml`. Ask the user before adding one.
- Every user-visible string goes in `src/shared/i18n/en.ts` and `ko.ts`. Colors come only from existing tokens; no hex in components.
- Background-removal ops (`removeBg`, `subject`, `brush`) change **alpha only**. RGB is never touched.
- `subject` **replaces** alpha; it does not intersect with the alpha already there.
- Coordinates and radii on the wire are integer full-res px of the image the op applies to (`u32`), so `Op` stays `Eq + Hash`.
- Tool switching: a pending `subject`/`brush` with at least one point or stroke is **applied** when the user leaves the tool or tab. A pending `removeBg` (Auto preview) is **dropped**.
- ⌘Z: while a `subject`/`brush` is pending, it drops the last point or stroke. Otherwise it undoes one applied step, as today.
- SAM files (rembg `v0.0.0`; the MD5s match rembg's `SAM_CHECKSUMS`; SHA-256s measured 2026-10-31):
  - `sam_vit_b_01ec64.encoder.quant.onnx`: sha256 `181301ce3500a86c38450b56e2b71dab8008a13b1812a0d6cc812f22651e143a`, 108836115 bytes, installed as `encoder.onnx`
  - `sam_vit_b_01ec64.decoder.quant.onnx`: sha256 `b11465135b7c93a1fd65141a4e2a96f1ddf2e3dab09b5eeab640bdf5f10aadbe`, 8755366 bytes, installed as `decoder.onnx`
  - The spec preferred MobileSAM, but rembg never pairs MobileSAM with a decoder, so the plan pins vit_b quant (about 117 MB).
- **Memory (user requirement):** the app must stay light outside editing. After the editor closes, memory must return: the app plus WebContent RSS, a few seconds after close, is at most **+50 MB** over the pre-edit baseline. Subject results are **not** cached (only the ~4 MB embedding is). Closing the editor clears the frontend's images and edit state.
- Done means all four pass: `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint`. Run Node/Yarn through `mise exec --`.
- Commits use `<prefix>(<scope>): <subject>` (see `docs/agents/workflow/commit-messages.md`).

## Review Focus

1. **A large image (6000×4000) with a 2048 preview.** Brush and click coordinates must land at full-res px, not preview px. Test: `toImagePx` and `brushRadius` in Task 5.
2. **A click or stroke outside the image** (dragging off the edge) must clamp, not error. Tests: `toImagePx` clamp (Task 5) and `brush_clamps_outside_points` (Task 1).
3. **Subject after a crop, then undoing the crop.** The embedding must match the image it was computed on. Test: `subject_embedding_keyed_by_prefix` (Task 4).
4. **SAM removed in Settings while the Subject tool is open.** It must fall back to `ModelPrompt` and must not leave a pending op the next copy would fail on. Step in Task 7.
5. **Many Subject clicks on a 4000×3000 photo.** Memory must not grow per click. Test: `subject_results_not_cached` (Task 4).
6. **Copy (⌘Enter) with an unapplied brush or subject.** Copy includes the pending op, as today. Step in Task 7.

---

### Task 1: `subject` / `brush` ops and the brush alpha painter

**Files:**
- Modify: `src-tauri/src/domain/edit/model.rs`, `src-tauri/src/domain/edit/mod.rs` (tests live in its `mod tests`)

**Interfaces:**
- Produces (in `model.rs`, re-exported from `domain::edit`):
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
  pub struct SubjectPoint { pub x: u32, pub y: u32, pub keep: bool }
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub enum BrushMode { Erase, Restore }
  #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
  pub struct Stroke { pub mode: BrushMode, pub radius: u32, pub points: Vec<[u32; 2]> }
  // new Op variants:
  Subject { points: Vec<SubjectPoint> },
  Brush { strokes: Vec<Stroke> },
  ```
- Produces: `pub fn paint_strokes(img: DynamicImage, strokes: &[Stroke]) -> AppResult<DynamicImage>`. `apply_geometric` handles `Op::Brush` by calling it. `Op::Subject` joins `RemoveBg | Upscale` in the "not geometric" arm.

- [ ] **Step 1: Write the failing tests** in `domain/edit/mod.rs`. Start from a 20×20 opaque RGBA image where every pixel is `[10,20,30,255]`.
  - `op_serde_new_variants`: `{"type":"subject","points":[{"x":3,"y":4,"keep":false}]}` and `{"type":"brush","strokes":[{"mode":"restore","radius":5,"points":[[1,2]]}]}` both round-trip exactly.
  - `brush_erase_clears_alpha_keeps_rgb`: one erase stroke at `[[10,10]]` with radius 4. Pixel (10,10) becomes `[10,20,30,0]`, and (0,0) keeps alpha 255.
  - `brush_line_fills_between_points`: erase `[[2,10],[17,10]]` with radius 2. (10,10) has alpha 0, and (10,0) has 255.
  - `brush_restore_sets_alpha`: start from the same image with alpha 0, restore `[[10,10]]` with radius 4. (10,10) has alpha 255 and RGB unchanged.
  - `brush_clamps_outside_points`: erase `[[500,500]]` with radius 3 returns `Ok`, and the image is unchanged.
  - `brush_rejects_zero_radius`: radius 0 returns `Err(InvalidInput)`.
  - Extend `ai_ops_are_not_geometric` with `Op::Subject { points: vec![] }`.

- [ ] **Step 2: Run them and confirm they fail to compile or fail.**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib domain::edit`

- [ ] **Step 3: Implement `paint_strokes`.**
  - Coverage per pixel: `c = clamp((radius - d) / feather, 0, 1)`, where `d` is the distance to the stroke polyline (point-to-segment; a single point is a zero-length segment) and `feather = max(1, radius / 4)` as f32.
  - Erase: `a = a·(1−c)`. Restore: `a = a + (255−a)·c`.
  - Only visit each segment's bounding box (± radius) clipped to the image.
  - Reject `radius == 0` or `radius > MAX_SIDE`.
  - Update `apply_geometric`'s doc comment to say it also paints brush strokes.

- [ ] **Step 4: Run the tests and confirm they pass.** Same command. Expected: all `domain::edit` tests PASS.

- [ ] **Step 5: Commit.** `feat(editor): subject and brush edit ops`

### Task 2: SAM model engine

**Files:**
- Modify: `src-tauri/src/app/engines.rs`, `src-tauri/src/app/ai.rs` (`model_path`, `forget`)

**Interfaces:**
- Produces: `EngineId::Sam` (serde `"sam"`, `dir_name: "sam"`, `binaries: &["encoder.onnx", "decoder.onnx"]`); `EngineId::ALL` has length 4.
- Produces: `pinned(EngineId::Sam)` returns two `Raw` assets with the constants in Global Constraints, version `"1"`. Bgremove keeps `MODEL_VERSION` `"2"`, so the version becomes per engine.
- Produces: `fn model_paths(dir: &Path, id: EngineId) -> AppResult<Vec<PathBuf>>` replaces `model_path` (it returns every binary and errors `Other("model missing")` if any is absent). `forget` refuses `Bgremove` **and** `Sam` while `RUN` is held.

- [ ] **Step 1: Write the failing tests.**
  - `engines`: `EngineId::parse("sam") == Ok(Sam)`. `pinned(Sam)` is `Some`, `valid()`, has 2 assets, and `size() == 108836115 + 8755366` (on macOS/Windows only, the same cfg as `pinned`).
  - `ai`: extend `forget_refuses_while_running` so `forget(EngineId::Sam)` errors while held.

- [ ] **Step 2: Run and confirm FAIL.** `cargo test --manifest-path src-tauri/Cargo.toml --lib engines ai::tests::forget`

- [ ] **Step 3: Implement.** Add the enum variant, the `SPECS` row, the `pinned` arm, `resolve(Sam) => pinned(id)`, and the `forget` match. Fix every exhaustive match the compiler flags.

- [ ] **Step 4: Run all lib tests and confirm PASS.** `cargo test --manifest-path src-tauri/Cargo.toml --lib`

- [ ] **Step 5: Commit.** `feat(engines): sam model engine`

### Task 3: SAM inference in `ai.rs`

**Files:**
- Modify: `src-tauri/src/app/ai.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct SamEmbedding { data: Vec<f32>, shape: Vec<usize>, scale: f32, w: u32, h: u32 }
  pub fn sam_embed(app: &AppHandle, img: &DynamicImage) -> AppResult<SamEmbedding>
  pub fn sam_cut(app: &AppHandle, img: &DynamicImage, emb: &SamEmbedding, points: &[SubjectPoint]) -> AppResult<DynamicImage>
  ```
- Private pure helpers (tested):
  - `fn sam_input(img: &DynamicImage) -> (Vec<f32>, f32)`
  - `fn sam_prompt(points: &[SubjectPoint], scale: f32) -> (Vec<f32>, Vec<f32>)`
  - `fn sam_mask(logits: &[f32], scale: f32, w: u32, h: u32) -> GrayImage`

Pre- and post-processing follow rembg `sessions/sam.py`:
- **Encoder input:** HWC f32 `[684, 1024, 3]` of raw 0–255 RGB, with no batch dim and no normalization (the exported encoder does that itself). The image is scaled by `scale = min(1024/w, 684/h)`, placed top-left, and zero-padded.
- **Decoder inputs** (by name):
  - `image_embeddings`: the encoder's first output
  - `point_coords`: `[1, n+1, 2]`, which is each point × `scale` plus a `(0,0)` pad
  - `point_labels`: `[1, n+1]`, with `keep` → 1, `!keep` → 0, and the pad → −1
  - `mask_input`: zeros `[1,1,256,256]`
  - `has_mask_input`: `[0]`
  - `orig_im_size`: `[684, 1024]`
- **Output:** the first output is `[1, k, 684, 1024]` logits. OR the `k` masks: take the per-pixel max logit. Crop to `round(w·scale) × round(h·scale)`, resize bilinearly to `w × h`, and set alpha = `sigmoid(logit)·255`.
- **Running:** take the `RUN` lock and the shared `ENV` exactly like `cut`. Load the encoder per `sam_embed` and the decoder per `sam_cut`; neither session is kept. `sam_cut` errors `InvalidInput` on empty `points`, and on an `emb` whose `w,h` differ from `img`.

- [ ] **Step 1: Write the failing tests.**
  - `sam_input_scales_and_pads`:
    - A 2048×1368 image → scale 0.5 and `len == 684*1024*3`.
    - A 1024×100 solid `[200,0,0]` image → scale 1.0; pixel (0,0) is `[200,0,0]`, and row 200, col 0 is `[0,0,0]`.
  - `sam_prompt_scales_and_pads`: points `[(100,50,keep),(10,20,!keep)]` at scale 0.5 → coords `[50,25, 5,10, 0,0]` and labels `[1,0,-1]`.
  - `sam_mask_left_half`: logits that are +10 for x < 512 and −10 elsewhere, with scale 1.0 and w=1024, h=684 → (10,10) is 255 and (1000,10) is 0.
  - An `#[ignore]` test `sam_real_model`, the same shape as `installs_pinned_bgremove`: download, embed one bench image, cut with one centre point, and write the result PNG to `VEE_AI_OUT`.

- [ ] **Step 2: Run and confirm FAIL.** `cargo test --manifest-path src-tauri/Cargo.toml --lib ai::tests::sam`

- [ ] **Step 3: Implement** the helpers, then `sam_embed` and `sam_cut`. Reuse `apply_mask`; the mask is already the image size, so the resize there is a no-op.

- [ ] **Step 4: Run and confirm PASS**, then run the real model once:
  ```
  node scripts/fetch-onnxruntime.mjs
  cargo test --manifest-path src-tauri/Cargo.toml --lib sam_real_model -- --ignored --nocapture
  ```
  Expected: PASS, and the output PNG keeps the centre subject. Look at it.
  Also measure peak RSS with the existing `bench_models` approach (`ps` RSS before and after each run; extend it with a SAM embed + cut pass). Record the SAM peak and the RSS after the run in `docs/agents/architecture.md` (Task 8). The RSS after the run must be back near the pre-run value, because sessions are dropped.

- [ ] **Step 5: Commit.** `feat(editor): sam subject cut`

### Task 4: Render `subject` with a cached embedding

**Files:**
- Modify: `src-tauri/src/app/editor.rs`

**Interfaces:**
- Consumes: `ai::{sam_embed, sam_cut, SamEmbedding}`, `Op::Subject`.
- Changes `render`'s AI callback to `ai: impl Fn(&DynamicImage, &Op, u64) -> AppResult<DynamicImage>`, where the `u64` is `ai_key(session, &ops[..i])` (the key of the ops **before** the AI op).
- `Session` gains `embeds: Mutex<HashMap<u64, Arc<ai::SamEmbedding>>>`, cleared with the session like `cache`.
- `Op::Subject` is an AI op but is **not** put in `cache`: only its embedding is cached. A click re-runs just the decoder, so the cache doesn't grow by a full-res image per click.
- In `render_off_main`, `Op::Subject` does three things: look up `embeds[prefix_key]`; on a miss, call `sam_embed` and insert; then call `sam_cut`.

- [ ] **Step 1: Write the failing test** `subject_embedding_keyed_by_prefix`. Use a counting closure that counts distinct prefix keys it sees for `Subject`.
  - Render `[Subject{p1}]`, then `[Subject{p1,p2}]` → the prefix key is the same both times. The results differ, so the result cache misses each time.
  - Render `[Crop…, Subject{p1}]` → a new prefix key.
  - `subject_results_not_cached`: after rendering `[Subject{p1}]`, `[Subject{p1,p2}]` and `[Subject{p1,p2,p3}]`, `session.cache.len() == 0`, and `embeds.len() == 1`.
  - Update the existing `counting` helper to the 3-arg callback.

- [ ] **Step 2: Run and confirm FAIL.** `cargo test --manifest-path src-tauri/Cargo.toml --lib app::editor`

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run all lib tests and confirm PASS.**

- [ ] **Step 5: Commit.** `feat(editor): render subject with cached sam embedding`

### Task 5: Frontend types and pure helpers

**Files:**
- Modify: `src/domain/edit/apis/edit.dto.ts`, `src/utils/edit.ts`, `src/utils/edit.test.ts`, `src/domain/convert/apis/convert.dto.ts`

**Interfaces:**
- Produces in `edit.dto.ts`:
  ```ts
  export interface SubjectPoint { x: number; y: number; keep: boolean }
  export type BrushMode = 'erase' | 'restore';
  export interface Stroke { mode: BrushMode; radius: number; points: [number, number][] }
  // EditOp gains:
  | { type: 'subject'; points: SubjectPoint[] }
  | { type: 'brush'; strokes: Stroke[] }
  ```
- Produces in `utils/edit.ts`:
  - `toImagePx(p: Pt, view: Size, image: Size): Pt`: rounded and clamped to `[0, image-1]`. `cropFromDrag` keeps its own math.
  - `brushRadius(viewRadius: number, view: Size, image: Size): number`: `max(1, round(viewRadius * image.w / view.w))`.
  - `thinPoints(points: [number, number][], minGap: number): [number, number][]`: keeps the first and last points, and drops any point closer than `minGap` to the last kept one.
  - `sizeAfter` returns `s` unchanged for `subject` and `brush`.
- `EngineId` gains `'sam'`.

- [ ] **Step 1: Write the failing tests** in `edit.test.ts`:
  - `toImagePx({x:50,y:25},{w:100,h:50},{w:6000,h:3000})` → `{x:3000,y:1500}`, and `{x:-5,y:999}` → `{x:0,y:2999}`.
  - `brushRadius(12,{w:1000,h:500},{w:6000,h:3000})` → 72, and `brushRadius(0.1,…)` → 1.
  - `thinPoints([[0,0],[1,0],[2,0],[10,0],[11,0]], 4)` → `[[0,0],[10,0],[11,0]]`.
  - `sizeAfter({w:10,h:10},[{type:'brush',strokes:[]},{type:'subject',points:[]}])` → `{w:10,h:10}`.

- [ ] **Step 2: Run and confirm FAIL.** `mise exec -- yarn test`

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Confirm PASS.** `mise exec -- yarn test && mise exec -- yarn typecheck`

- [ ] **Step 5: Commit.** `feat(editor): subject and brush types and coordinate helpers`

### Task 6: SAM in Settings

**Files:**
- Modify: `src/app/settings/_components/ConvertSection.tsx`, `src/shared/i18n/en.ts`, `src/shared/i18n/ko.ts`

**Interfaces:**
- Consumes: `EngineId` `'sam'` (Task 5) and the Rust engine (Task 2).
- Produces: `t.settings.convert.sam` with the same shape as `bgremove`.

Add `<EngineSection engine="sam" index={4} run={run} />` right after `bgremove`, and shift the later `index` values by one. Copy:

| key | en | ko |
|---|---|---|
| caption | `Subject selection · AI model` | `피사체 선택 · AI 모델` |
| title | `SAM` | `SAM` |
| missingHint(mb) | `Needed to pick a subject by clicking in the editor. About ${mb} MB` / without mb: `Needed to pick a subject by clicking in the editor.` | `에디터에서 클릭으로 피사체를 고를 때 필요해요. 약 ${mb}MB` / without mb: `에디터에서 클릭으로 피사체를 고를 때 필요해요.` |
| removeConfirm | `Remove the model? Subject selection stops working until you download it again.` | Follow the tone of `bgremove.removeConfirm` in `ko.ts`. |

Match the existing `bgremove` ko phrasing style (read `ko.ts` first) and adjust the ko copy above to match it if it differs.

- [ ] **Step 1: Implement.**
- [ ] **Step 2: Run `yarn typecheck && yarn lint` and confirm PASS.** Then `yarn tauri dev` → Settings → Convert. The SAM section shows, Download works, Remove works. Check light/dark and ko/en.
- [ ] **Step 3: Commit.** `feat(settings): sam model section`

### Task 7: Remove background tool rail, Subject and Brush UI

**Files:**
- Create: `src/app/editor/_components/BgToolRail.tsx`, `src/app/editor/_components/SubjectLayer.tsx`, `src/app/editor/_components/BrushLayer.tsx`
- Modify: `src/app/editor/EditorApp.tsx`, `src/shared/i18n/en.ts`, `src/shared/i18n/ko.ts`

**Interfaces:**
- Consumes: Task 5 types and helpers, `useEngineStatus('sam')`, `Segmented`, `TOOL_BUTTON`, `ModelPrompt`.
- `export type BgTool = 'auto' | 'subject' | 'brush'` (in `BgToolRail.tsx`).
- `BgToolRail({ value, onChange, disabled }: { value: BgTool; onChange(v: BgTool): void; disabled: boolean })`:
  - a vertical `.glass` `role="radiogroup"` of three icon `role="radio"` buttons, each with `aria-label` and `title` from `t.editor.bgTools`
  - arrow keys move the selection (reuse `arrowStep` from `@/utils/radio` as `Segmented` does)
  - absolutely positioned at the stage's left edge, vertically centred
  - when the Remove background tab is active, the stage gets left padding the rail's width so the image isn't covered
- `SubjectLayer({ view, image, points, onAdd, disabled })`:
  - a click layer over the image; `onAdd({ ...toImagePx(...), keep: !e.altKey })`
  - draws each point as a small circle, `bg-success` for keep and `bg-error` for remove, with a `+`/`−` glyph
- `BrushLayer({ view, image, mode, viewRadius, onStroke, disabled })`:
  - a `<canvas>` sized to `view`, using pointer events with `setPointerCapture`
  - while dragging, it draws the live stroke at 35 % opacity in `--color-primary` (erase) or `--color-sky` (restore), read through `getComputedStyle`
  - on pointer-up, it calls `onStroke({ mode, radius: brushRadius(viewRadius, view, image), points: thinPoints(imagePts, radius / 4) })` and clears the canvas
  - the circle cursor follows the pointer (white 1.5px ring plus a dark dashed ring); the native cursor is hidden over the layer

`EditorApp` state and rules:
- `bgTool: BgTool` defaults to `'auto'` when entering the tab. `brushMode: BrushMode` defaults to `'erase'`. `brushSize` is the view radius, slider range 2–80, default 16. `[` / `]` change it by 2.
- **Live pending:**
  - Extend `aiPending` to also cover a non-empty `subject` and a non-empty `brush`. The existing result effect then renders `[...ops, pending]` for them.
  - The spinner shows for `removeBg`, `upscale` and the first `subject` render only, not for `brush`.
  - The tab `fieldset` is disabled only for the spinner cases.
- **Display:**
  - Subject with a result: the preview at `opacity-35`, the result on top, and the `SubjectLayer` dots.
  - Brush: the latest result (or the preview) plus the `BrushLayer`.
  - Auto: `CompareView`, unchanged.
- **Leaving a tool or tab** (`handleBgTool`, `handleTab`): a pending `subject`/`brush` with at least one item goes through `handleApply(pending)`. A pending `removeBg` is dropped. Auto then re-proposes `removeBg` through its existing effect.
- **⌘Z:** with a non-empty pending `subject`/`brush`, drop its last item (an empty list becomes `pending = null`). Otherwise `handleUndo`.
- **Bottom bar** (by tool, while the tab is `removeBg`):
  - auto: unchanged
  - subject: `ModelPrompt` if SAM isn't installed; otherwise hint + `subjectReset` (sets pending null) + Apply
  - brush: `Segmented` erase/restore + a size slider (`<input type="range">` with `aria-label`) + Apply
- **SAM removed while on Subject:** clear pending in an effect on `samReady`, the same way the existing `bgReady` effect does.
- **Copy:** unchanged; `all` already includes `pending`.
- **Release on close:** the window is only hidden on close, so `handleCancel` and every other path that closes the editor (a successful copy, if it closes) must reset `source`, `preview`, `result`, `ops`, `pending` and the brush/subject state to their initial values. The `BrushLayer` canvas must unmount. Listen for the close in one place: emit `editor://closed` from Rust `close()` and reset on it, so a close from Rust is covered too. Add the event name per `docs/agents/conventions/tauri-commands.md`.

i18n `editor` keys:

| key | en | ko |
|---|---|---|
| `bgTools.auto` | `Auto` | `자동` |
| `bgTools.subject` | `Select subject` | `피사체 선택` |
| `bgTools.brush` | `Brush` | `브러시` |
| `subjectHint(mac: boolean)` | `Click what to keep · ${mac ? '⌥' : 'Alt+'}click to remove` | `남길 대상을 클릭하세요 · ${mac ? '⌥' : 'Alt+'}클릭으로 빼기` |
| `subjectReset` | `Reset` | `초기화` |
| `brushModes.erase` / `brushModes.restore` | `Erase` / `Restore` | `지우기` / `복원` |
| `brushSize` | `Size` | `크기` |

- [ ] **Step 1: Build the three components and wire up `EditorApp`.**
- [ ] **Step 2: Run `yarn typecheck && yarn lint && yarn test` and confirm PASS.**
- [ ] **Step 3: Manual check in `yarn tauri dev`** (if Vee is installed, use the separate identifier per `docs/agents/build-run.md`). Use an image with two subjects:
  - Subject: click the second subject → only it stays. ⌥-click removes a region. ⌘Z drops the last point.
  - Switch to Brush → the subject step is applied. Restore over a hole → pixels come back. Erase works.
  - ⌘Enter copies with a pending stroke.
  - Remove SAM in Settings while on Subject → `ModelPrompt` shows.
  - Check all of the above in light/dark and ko/en.
  - Close the editor, reopen it with another image → no leftover state from the previous image.
- [ ] **Step 4: Commit.** `feat(editor): tool rail with subject select and brush`

### Task 8: Docs and final verification

**Files:**
- Modify: `docs/agents/architecture.md` (engines list `ffmpeg | rhwp | bgremove | sam`, the SAM pin, the new ops, the embedding cache), `docs/agents/overview.md` (network paragraph: SAM download), `docs/agents/testing.md` (`sam_real_model`)

- [ ] **Step 1: Update the docs.**
- [ ] **Step 2: Run all four checks** and confirm every one passes:
  ```
  cargo test --manifest-path src-tauri/Cargo.toml --lib
  mise exec -- yarn test
  mise exec -- yarn typecheck
  mise exec -- yarn lint
  ```
- [ ] **Step 3: Memory check (pass/fail).** In `yarn tauri build --no-bundle` (or a dev build if release isn't possible; say which one was used), sum the RSS of the Vee process and its WebContent process(es), e.g. `ps -o rss= -p <pids>`, at three points:
  1. Before opening the editor, after the app has been idle for 10 s.
  2. After using Auto, Subject (≥10 clicks) and Brush (≥10 strokes) on a ≥4000×3000 photo.
  3. 10 s after closing the editor.

  **Pass:** ③ − ① ≤ 50 MB. Record all three numbers in the PR description. If it fails, use superpowers:systematic-debugging; don't raise the bar.
- [ ] **Step 4: Commit.** `docs: subject select and brush in the editor`
