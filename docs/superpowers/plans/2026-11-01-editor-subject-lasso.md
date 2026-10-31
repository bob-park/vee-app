# Subject Lasso Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In the editor's Subject tool, dragging draws a loop ("lasso") that picks the object inside it. A short press still adds a click point.

**Architecture:**
- The `subject` op gets an optional `lasso` polygon.
- The worker turns the lasso's bounding box into a SAM box prompt (labels 2/3), alongside the click points, and reuses the cached embedding.
- After decoding, the worker clears alpha outside the lasso grown by a margin, unless a keep point lies outside the lasso.
- The frontend tells a drag from a click by distance, keeps an ordered mark history so ⌘Z can undo, and draws the lasso.

**Tech Stack:** Rust (`image` 0.25, `ort` in the `--ai-worker` child), React + TypeScript, `node:test`.

**Spec:** `docs/superpowers/specs/2026-11-01-editor-subject-lasso-design.md` (Korean). It builds on `docs/superpowers/specs/2026-10-31-editor-bg-brush-subject-design.md`.

## Global Constraints

- No new dependencies.
- Strings only in `src/shared/i18n/{en,ko}.ts`. Colors only from existing tokens.
- Wire format: `{ type: 'subject', points: SubjectPoint[], lasso?: [number, number][] }`. In Rust this is `lasso: Option<Vec<[u32; 2]>>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`. The old `{type, points}` JSON must still parse.
- Coordinates are integer full-res px of the image the op applies to.
- `subject` is `InvalidInput` only when `points` is empty **and** `lasso` is absent. A lasso with fewer than 3 points is also `InvalidInput`.
- Gesture threshold: a press is a lasso if the pointer moved **≥ 8 view px** from the start **and** the path's bounding-box long side is **≥ 8 view px**. Otherwise it is a click.
- One lasso per op: a new lasso replaces the previous one, and points stay. ⌘Z removes the last added mark (a point or a lasso). Undoing a lasso brings back the previous lasso, if there was one.
- Box prompt: the lasso's bbox corners are passed as `(x0,y0)` with label 2 and `(x1,y1)` with label 3. Both are scaled by the same `scale` as the points. Order: click points, then the box corners, then the `(0,0)`/−1 pad. The embedding key is unchanged, so drawing a lasso never re-encodes.
- Clip: margin = `max(8, round(0.02 × max(w, h)))` image px. Alpha outside (lasso polygon dilated by the margin) becomes 0. Skip the clip if any `keep` point is outside the undilated polygon. Alpha only: RGB is untouched, and subject still replaces alpha.
- Hint copy:
  - en: `` `Circle or click what to keep · ${mac ? '⌥' : 'Alt+'}click to remove` ``
  - ko: `` `대상을 둘러 그리거나 클릭하세요 · ${mac ? '⌥' : 'Alt+'}클릭으로 빼기` ``
- Memory: the user's top requirement is unchanged. Nothing new is cached.
- SAFETY: never launch the vee-app binary, `yarn tauri dev` or `yarn tauri build` (the real data folder is `com.bobpark.vee`). The controller runs GUI checks with `--config '{"identifier":"com.bobpark.vee.dev"}'`.
- Done means all four pass:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `mise exec -- yarn test`
  - `mise exec -- yarn typecheck`
  - `mise exec -- yarn lint`

## Review Focus

1. **A lasso drawn partly off the image** (drag past the edge). Points clamp, and the clip still works. Test: `lasso_clip_clamped_polygon` (Task 1) and the `toImagePx` clamp (existing).
2. **A self-crossing or figure-8 lasso.** The even-odd fill must not panic and must give a sane mask. Test: `lasso_fill_self_crossing` (Task 1).
3. **Undo after lasso → point → lasso.** The marks come back in reverse order. Test: `undoMark restores previous lasso` (Task 2).
4. **A slightly shaky click (3–5 px).** It stays a click. Test: `classifyGesture` cases (Task 2).
5. **A large image (6000×4000) with a lasso of a few hundred points.** The clip stays O(W·H), with no per-pixel polygon test. Test: `lasso_clip_large_is_fast` (Task 1): a 6000×4000 clip with a 400-point lasso finishes in under 1 s in a debug build.

---

### Task 1: Rust — lasso on the subject op, box prompt, clip

**Files:**
- Modify:
  - `src-tauri/src/domain/edit/model.rs` (`Op::Subject`)
  - `src-tauri/src/app/editor.rs` (validation in `render`, pass `lasso` to `ai::subject`)
  - `src-tauri/src/app/ai.rs` (`sam_prompt`, `decode`, `subject`, `handle`)
  - `src-tauri/src/app/ai_worker.rs` (`Request::Subject` gains `lasso`)
- Fix every `Op::Subject { points }` pattern and constructor the compiler flags, tests included.

**Interfaces:**
- Produces:
  - `Op::Subject { points: Vec<SubjectPoint>, lasso: Option<Vec<[u32; 2]>> }`
  - `ai::subject(app, img, key: u64, points: &[SubjectPoint], lasso: Option<&[[u32; 2]]>) -> AppResult<DynamicImage>`
- Private pure helpers in `ai.rs`:
  - `fn lasso_box(lasso: &[[u32; 2]]) -> [u32; 4]`, returning `[x0, y0, x1, y1]`
  - `fn sam_prompt(points: &[SubjectPoint], bbox: Option<[u32; 4]>, scale: f32) -> (Vec<f32>, Vec<f32>)`
  - `fn lasso_mask(lasso: &[[u32; 2]], w: u32, h: u32, margin: u32) -> GrayImage`: an even-odd scanline fill, then a separable square dilation by `margin` (sliding-window max, O(W·H)). 255 means keep.
  - `fn point_in_lasso(lasso: &[[u32; 2]], x: u32, y: u32) -> bool` (even-odd)
  - `fn clip_margin(w: u32, h: u32) -> u32`, equal to `max(8, round(0.02 * max(w, h)))`

- [ ] **Step 1: Write the failing tests.**
  - `model`/`domain::edit`:
    - `subject_lasso_serde`: `{"type":"subject","points":[],"lasso":[[1,2],[3,4],[5,6]]}` round-trips.
    - The old `{"type":"subject","points":[{"x":1,"y":2,"keep":true}]}` parses with `lasso == None`, and serializes back with no `lasso` key.
  - `editor`:
    - `subject_needs_points_or_lasso`: `Subject{points:[], lasso:None}` is InvalidInput.
    - `Subject{points:[], lasso:Some(3 pts)}` calls the AI callback.
    - `Subject{points:[], lasso:Some(2 pts)}` is InvalidInput.
  - `ai`:
    - `sam_prompt_with_box`: points `[(100,50,keep)]`, bbox `[10,20,30,40]`, scale 0.5 → coords `[50,25, 5,10, 15,20, 0,0]`, labels `[1,2,3,-1]`. With `bbox: None`, the output is unchanged from today.
    - `lasso_box_min_max`: `[[5,9],[2,7],[8,1]]` → `[2,1,8,9]`.
    - `lasso_clip_square`: 100×100, lasso `[[20,20],[80,20],[80,80],[20,80]]`, margin 8. `lasso_mask` is 255 at (50,50) and (85,50), and 0 at (5,5) and (95,50).
    - `lasso_fill_self_crossing`: the figure-8 `[[10,10],[90,90],[90,10],[10,90]]` on 100×100 doesn't panic. The two lobes, e.g. (50,20) and (50,80), are 255, and (5,50) is 0.
    - `lasso_clip_clamped_polygon`: a lasso whose points sit at the image edge (x=0 or x=w−1) gives no panic and keeps the pixels inside.
    - `clip_skipped_for_outside_keep`: in a `decode`-level helper (factor the "apply mask, then clip unless a keep point is outside" step into `fn finish_cut(img: RgbaImage, mask: &GrayImage, points, lasso) -> RgbaImage` and test that), a keep point at (95,50) outside the square lasso leaves alpha at (95,50) equal to the mask's.
    - `lasso_clip_large_is_fast`: `lasso_mask` on 6000×4000 with a 400-point circle finishes in under 1 s.
  - `ai_worker`: update the `Request::Subject` round-trip and constructors with `lasso`.
- [ ] **Step 2: Run and confirm FAIL.** `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- [ ] **Step 3: Implement.**
  - `decode` builds the prompt with `lasso.map(lasso_box)`.
  - After `apply_mask`, it calls `finish_cut`, which zeroes alpha where `lasso_mask == 0` unless a keep point is outside `point_in_lasso`.
  - `render` validation follows Global Constraints.
- [ ] **Step 4: Confirm PASS.** Run the full `--lib`. `cargo build --manifest-path src-tauri/Cargo.toml` must have no warnings.
- [ ] **Step 5: Commit.** `feat(editor): lasso box prompt and clip for subject`

### Task 2: Frontend — gesture, marks history, lasso drawing

**Files:**
- Modify:
  - `src/domain/edit/apis/edit.dto.ts`
  - `src/utils/edit.ts`
  - `src/utils/edit.test.ts`
  - `src/app/editor/_components/SubjectLayer.tsx`
  - `src/app/editor/EditorApp.tsx`
  - `src/shared/i18n/en.ts`
  - `src/shared/i18n/ko.ts`

**Interfaces:**
- Consumes: the wire shape from Task 1.
- Produces in `edit.dto.ts`: the subject op gains `lasso?: [number, number][]`.
- Produces in `utils/edit.ts`:
  - `classifyGesture(path: Pt[], minMove = 8): 'click' | 'lasso'`: lasso when max distance from `path[0]` ≥ `minMove` and the bbox long side ≥ `minMove` (view px).
  - `export interface SubjectMarks { points: SubjectPoint[]; lassos: [number, number][][]; order: ('point' | 'lasso')[] }`
  - `emptyMarks(): SubjectMarks`
  - `addPoint(m, p): SubjectMarks`
  - `addLasso(m, lasso): SubjectMarks`
  - `undoMark(m): SubjectMarks`: pops the last `order` entry and its item.
  - `subjectOp(m): EditOp | null`: `null` when there are no points and no lassos; otherwise `{ type: 'subject', points, lasso: lassos.at(-1) }`, with `lasso` omitted when there is none.
  - Remove `dropLastMark`'s subject branch (keep it for brush), or replace its subject use with `undoMark`.
- `SubjectLayer` props become `{ view, image, points, lasso, onPoint(p: SubjectPoint), onLasso(pts: [number, number][]), disabled }`.
  - It uses pointer events with `setPointerCapture`.
  - While a drag is a lasso, it draws a live polyline.
  - On pointer-up, `classifyGesture` on the view-px path decides: a click → `onPoint({...toImagePx(start), keep: !e.altKey})`; a lasso → `onLasso(thinPoints(path.map(p => toImagePx(p)), gap))`, where `gap = max(1, round(max(image.w, image.h) / 400))`, as `[x, y]` tuples.
  - It renders the current `lasso` as an SVG polygon: `stroke` `var(--color-primary)`, dashed, `fill` primary at about 10 % opacity. Points stay as today.
- `EditorApp`:
  - The Subject tool's state is `marks: SubjectMarks`, and the pending subject op is `subjectOp(marks)`.
  - Apply, auto-apply on switch, Reset and close each reset `marks` to `emptyMarks()`.
  - ⌘Z with marks → `undoMark`.
  - A failed live subject render → `undoMark` (instead of `dropLastMark`).
  - Brush is unchanged.
- i18n: `subjectHint` uses the Global Constraints copy.

- [ ] **Step 1: Write the failing tests** (`edit.test.ts`):
  - `classifyGesture`:
    - `[{0,0},{3,4}]` → click (distance 5)
    - `[{0,0},{10,0},{10,10}]` → lasso
    - `[{0,0},{9,0},{0,0}]` → lasso (moved 9, bbox 9)
    - `[{0,0},{7,0}]` → click
  - `undoMark`:
    - lasso A → point p → lasso B, then undo ×1 gives `lassos [A]`, `points [p]`
    - undo ×2 gives `lassos [A]`, `points []`
    - undo ×3 gives empty, and `subjectOp` is `null`
  - `subjectOp`:
    - with points only, there is no `lasso` key
    - with lassos `[A, B]`, `lasso === B`
- [ ] **Step 2: Run and confirm FAIL.** `mise exec -- yarn test`
- [ ] **Step 3: Implement** the helpers, `SubjectLayer` and the `EditorApp` wiring, plus i18n.
- [ ] **Step 4: Run and confirm PASS.** `mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint`
- [ ] **Step 5: Commit.** `feat(editor): draw a lasso to pick a subject`

### Task 3: Docs and checks

**Files:**
- Modify: `docs/agents/architecture.md` (the subject op's `lasso`, the box prompt, the clip rule and margin, the gesture rule)

- [ ] **Step 1: Update the doc.**
- [ ] **Step 2: Run all four checks** from Global Constraints, and confirm every one passes.
- [ ] **Step 3: Commit.** `docs: subject lasso in the editor`
