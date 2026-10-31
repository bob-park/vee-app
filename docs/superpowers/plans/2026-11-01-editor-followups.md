# Editor Follow-ups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Clear the follow-ups listed in PR #26 on the same branch, except the large-image memory test and the Windows check.

**Spec:**
- `docs/superpowers/specs/2026-11-01-editor-subject-lasso-design.md`
- `docs/superpowers/specs/2026-10-31-editor-bg-brush-subject-design.md`

## Global Constraints

- No new dependencies. English code/comments. Strings only in i18n. Colors only from tokens.
- Behaviour stays the same except Task 1's soft edge. Tasks 2 and 3 must not change a pixel or an interaction.
- Commits are split by kind: `fix(editor):` (Task 1), `refactor(editor):` (Task 2), `style:` (Task 3).
- SAFETY: never launch the vee-app binary, `yarn tauri dev` or `yarn tauri build`. The real data folder is `com.bobpark.vee`.
- Done means all four pass:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `mise exec -- yarn test`
  - `mise exec -- yarn typecheck`
  - `mise exec -- yarn lint`
- Also `cargo build --manifest-path src-tauri/Cargo.toml` with no warnings.

## Review Focus

1. **Soft edge at the image border.** The lasso touches x=0 or y=0. The ramp must not panic or wrap. Test in Task 1.
2. **A keep point outside the lasso still skips the clip,** with no ramp applied either. The existing test must keep passing.
3. **The refactor keeps every close and reset path.** `editor://closed` still clears the brush/subject state after the hook extraction. A manual check is in the Task 2 report.

---

### Task 1: Soft lasso edge, one buffer

**Files:** `src-tauri/src/app/ai.rs` (`lasso_mask`, `finish_cut`, tests); `docs/agents/architecture.md` (clip sentence)

**Behaviour:**
- Inside the lasso dilated by `margin`, alpha is unchanged.
- From there, alpha fades linearly to 0 over `feather = max(4, margin / 2)` px.
  - Approach: dilate by `margin + feather`, then box-blur that binary mask, both separable and O(W·H). Or use any other O(W·H) approach with the same visible result.
  - Multiply alpha by `mask / 255`.
- Skip rules are unchanged: no clip when a keep point is outside the lasso, and none when the undilated fill is empty.
- Memory: keep at most **one** extra W×H u8 buffer alive at a time beyond the image and the SAM mask. Reuse buffers, or apply to alpha in place.

- [ ] **Step 1: Write the failing tests.**
  - `lasso_soft_edge`: 200×200, square lasso `[[50,50],[150,50],[150,150],[50,150]]`, margin 8, feather 4.
    - Mask is 255 at (100,100) and at (158,100), the dilated edge.
    - Mask is strictly between 0 and 255 at (160,100).
    - Mask is 0 at (170,100).
    - Mask is non-increasing from x=158 to x=170 along y=100.
  - `lasso_soft_edge_at_border`: a lasso touching x=0, y=0. No panic, and (0,0) is 255.
  - Update the existing `lasso_clip_square` and other expectations to the new edge. Keep `lasso_clip_large_is_fast` under 1 s.
- [ ] **Step 2: Confirm FAIL.**
- [ ] **Step 3: Implement.** Update the architecture.md clip sentence to describe the feather.
- [ ] **Step 4: Confirm PASS** (all four checks and `cargo build`).
- [ ] **Step 5: Commit.** `fix(editor): soft edge where a lasso cuts the subject`

### Task 2: Frontend tidy-up (no behaviour change)

**Files:**
- Create: `src/utils/pointer.ts` (`pointIn(e: React.PointerEvent<Element>): Pt`, view-px position relative to `e.currentTarget`), with a small `node:test` if it is testable without a DOM. If not, skip the test and say so.
- Create: a hook for the Remove background tab's state and handlers, e.g. `src/app/editor/_hooks/useBgTools.ts`, placed per `docs/agents/structure.md` and `docs/agents/conventions/naming.md`. It owns `bgTool`, brush mode/size, subject marks, and the tool-switch, ⌘Z-mark and reset helpers.
- Modify: `CropBox.tsx`, `BrushLayer.tsx`, `SubjectLayer.tsx` (import `pointIn`), `EditorApp.tsx`.

**Rules:**
- No change to rendered markup, class names or behaviour.
- `EditorApp.tsx` should shrink noticeably. Target under ~400 lines; report the number.
- Follow `docs/agents/conventions/react-sections.md`.

- [ ] **Step 1: Extract `pointIn`** and run the four checks.
- [ ] **Step 2: Extract the hook** and run the four checks.
- [ ] **Step 3: Self-check by reading the diff.** Every handler and effect that moved keeps the same dependencies and order, and `reset()` still clears the hook's state.
- [ ] **Step 4: Commit.** `refactor(editor): share pointIn and move bg tool state into a hook` (one or two commits)

### Task 3: Make the Rust tree rustfmt-clean

**Files:** possibly create `src-tauri/rustfmt.toml`; reformat `src-tauri/**/*.rs`.

**Steps:**
- Currently `cargo fmt --check` reports about 687 diffs, and there is no rustfmt config.
- First find the config the existing code was written with, most likely a wider `max_width` (e.g. 120) plus whatever else minimizes the diff, using only stable options.
- Add that `rustfmt.toml`, then run `cargo fmt`. Formatting only, with no other edits.
- Mention the new config in `docs/agents/conventions/rust-modules.md` or the testing doc, in one line.

- [ ] **Step 1: Pick the config.** Try candidates and record the `--check` diff count for each in the report.
- [ ] **Step 2: Run `cargo fmt`, then the four checks and `cargo build`.**
- [ ] **Step 3: Commit.** `style: rustfmt config and format the Rust tree`
