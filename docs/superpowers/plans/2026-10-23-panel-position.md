# Panel Position Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user dock the panel at the bottom, top, left or right of the screen (Settings → Panel, default bottom), with a vertical card list and width resizing on the sides; also enlarge the What's New window.

**Architecture:** A new `panelPosition` setting drives both sides. Rust (`app/window.rs`) turns position + saved ratio (`panelHeightRatio` for top/bottom, new `panelWidthRatio` for left/right) into the window rect and the toast rect. The panel frontend reads the same setting and switches between the existing horizontal layout and a vertical one. Pure geometry/key logic gets unit tests; layout is checked by eye.

**Tech Stack:** Tauri 2, Rust, React 19, Tailwind 4 + daisyUI 5, `node:test`.

**Spec:** `docs/superpowers/specs/2026-10-23-panel-position-design.md`

## Global Constraints

- Branch `feature/panel-position`. Commit messages `<prefix>: <subject>` ([commit-messages](../../agents/workflow/commit-messages.md)).
- Positions: `bottom | top | left | right`; default `bottom`.
- Bottom mode must look and behave exactly as today (no pixel change).
- Side panel: default width 25% of the primary work-area width; clamp 15–50%; minimum 340pt (but never over the 50% / per-monitor cap). Margin 8pt on every side.
- Side cards: full panel width, 160pt tall (`h-40`); only the history list scrolls, the pinned group does not.
- Height ratio is shared by top/bottom, width ratio by left/right. Reset clears only the current position's axis.
- Changing the position while the panel is visible re-places it at once.
- Slide curves/durations unchanged (`EASE_MOVE` 300ms in, `ease-in` 200ms out); direction follows the docked edge.
- What's New: 520×640, `resizable: false`.
- No new dependencies. No migration (settings is key-value). Every user-visible string in `src/shared/i18n/{en,ko}.ts`. Colors only in `globals.css`, both themes.
- Keep DOM hook classes `card` + `data-id`, `update`, `ghost`.
- Done = `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` pass, plus a look in `yarn tauri dev` at 4 positions × light/dark × ko/en.

## Review Focus

1. A small or secondary monitor (e.g. 1280×800, or an external display smaller than the primary): the 340pt side minimum must not exceed the 50% cap, and a ratio saved on the primary must still fit. → test `side_width_floor_never_exceeds_the_cap` (Task 2).
2. Windows at 150% scale: side and top rects must be in pixels like the bottom one. → test `panel_rects_scale_on_windows` (Task 2).
3. Switching position while the panel is open and pinned: it moves, and the toast follows the new position. → manual check (Task 6).
4. Typing in the search box in side mode: Up/Down still drive the `@app` suggestion list when it is open. → test `side_up_down_still_drive_suggestions` (Task 4).
5. A stale ratio for the other axis must not affect the current one (e.g. width ratio 0.5 saved, then switch to bottom). → test `each_axis_reads_its_own_ratio_key` (Task 1).

---

### Task 1: Settings keys

**Files:**
- Modify: `src-tauri/src/domain/settings/mod.rs` (`get` defaults, `validate`, tests)
- Modify: `src-tauri/src/domain/settings/model.rs` (`SettingsDto`)
- Modify: `src-tauri/src/domain/settings/commands.rs` (`get_settings`)
- Modify: `src/domain/settings/apis/settings.dto.ts`

**Interfaces:**
- Produces (Rust): setting `panelPosition` (`get` default `"bottom"`, validated `bottom|top|left|right`); backend-only `panelWidthRatio` (rejected by `validate`, like `panelHeightRatio`); `SettingsDto.panel_position`, `SettingsDto.panel_width_ratio`.
- Produces (TS): `export const PANEL_POSITIONS = ['bottom', 'top', 'left', 'right'] as const; export type PanelPosition = (typeof PANEL_POSITIONS)[number];` `Settings.panelPosition: PanelPosition`, `Settings.panelWidthRatio: string` (doc: `"" means the default width`), `'panelPosition'` added to `SettingKey`.

- [ ] **Step 1: Write the failing tests** in `settings/mod.rs` tests:

```rust
#[test]
fn panel_position_defaults_to_bottom_and_accepts_four_edges() {
    let conn = db::open_in_memory().unwrap();
    assert_eq!(get(&conn, "panelPosition"), "bottom");
    for v in ["bottom", "top", "left", "right"] {
        assert!(validate("panelPosition", v).is_ok(), "{v}");
    }
    assert!(validate("panelPosition", "center").is_err());
    assert!(validate("panelWidthRatio", "0.3").is_err()); // command-only
}

#[test]
fn each_axis_reads_its_own_ratio_key() {
    let conn = db::open_in_memory().unwrap();
    set_raw(&conn, "panelWidthRatio", "0.5000").unwrap();
    assert_eq!(get(&conn, "panelHeightRatio"), "");
    assert_eq!(get(&conn, "panelWidthRatio"), "0.5000");
}
```

- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib settings` → `panel_position_defaults…` FAILS (`get` returns `""`).
- [ ] **Step 3:** Add `"panelPosition" => "bottom"` to `get`, `"panelPosition" => matches!(value, "bottom" | "top" | "left" | "right")` to `validate` (update the trailing comment to name both ratio keys), the two DTO fields, the two `get_settings` lines, and the TS DTO changes above.
- [ ] **Step 4:** Run the same cargo command → PASS; `yarn typecheck` → PASS.
- [ ] **Step 5:** Commit `feat: add panel position setting`.

### Task 2: Rust panel geometry

**Files:**
- Modify: `src-tauri/src/app/window.rs` (constants, pure functions, `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub(crate) enum PanelPosition { Bottom, Top, Left, Right }` with `pub(crate) fn parse(s: &str) -> Self` (unknown → `Bottom`), `pub(crate) fn side(self) -> bool` (Left/Right), `pub(crate) fn ratio_key(self) -> &'static str` (`"panelWidthRatio"` for sides, else `"panelHeightRatio"`).
  - `const PANEL_WIDTH_RATIO: f64 = 0.25; const PANEL_MIN_WIDTH: f64 = 340.0;`
  - `fn panel_extent_for(pos: PanelPosition, base: f64, work: f64, ratio: Option<f64>) -> f64` — size along `pos`'s axis in design points. `base`/`work` are primary / current work-area height (top/bottom) or width (sides). Default share 0.30 or 0.25; clamp to `[min, panel_max(base)]` where min = `panel_min(base)` for height and `panel_min(base).max(PANEL_MIN_WIDTH).min(panel_max(base))` for width; then `.min(panel_max(work))`. Replaces `panel_height_for`; `panel_height`/`clamp_panel_height` may be folded in.
  - `fn clamp_panel_extent(pos: PanelPosition, base: f64, v: f64) -> f64` — the clamp above (used by resize).
  - `fn panel_rect(work: Rect, unit: f64, pos: PanelPosition, extent: f64) -> Rect`.
  - `fn toast_rect(work: Rect, unit: f64, pinned: Option<(PanelPosition, f64)>) -> Rect` — `f64` is the pinned panel's live extent in design points. Bottom: lift by extent + margin (today). Top: no lift. Left/Right: same y as unpinned; x centered in the work area minus `(extent + PANEL_MARGIN)` on the panel's side.

- [ ] **Step 1: Write the failing tests** (update the existing callers in tests to the new signatures; existing bottom expectations must stay identical):

```rust
#[test]
fn panel_docks_to_each_edge() {
    let work = Rect { x: -2560.0, y: -241.0, w: 2560.0, h: 1410.0 };
    use PanelPosition::*;
    assert_eq!(panel_rect(work, 1.0, Bottom, 360.0), Rect { x: -2552.0, y: 801.0, w: 2544.0, h: 360.0 });
    assert_eq!(panel_rect(work, 1.0, Top, 360.0), Rect { x: -2552.0, y: -233.0, w: 2544.0, h: 360.0 });
    assert_eq!(panel_rect(work, 1.0, Left, 400.0), Rect { x: -2552.0, y: -233.0, w: 400.0, h: 1394.0 });
    assert_eq!(panel_rect(work, 1.0, Right, 400.0), Rect { x: -408.0, y: -233.0, w: 400.0, h: 1394.0 });
}

#[test]
fn panel_rects_scale_on_windows() {
    let px = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1040.0 };
    assert_eq!(panel_rect(px, 1.5, PanelPosition::Top, 300.0), Rect { x: 12.0, y: 12.0, w: 1896.0, h: 450.0 });
    assert_eq!(panel_rect(px, 1.5, PanelPosition::Right, 340.0), Rect { x: 1398.0, y: 12.0, w: 510.0, h: 1016.0 });
}

#[test]
fn side_width_defaults_to_a_quarter_with_a_340pt_floor() {
    use PanelPosition::Left;
    assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, None), 432.0);
    assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, Some(0.1)), 340.0); // floor beats 15%
    assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, Some(0.7)), 864.0); // 50% cap
    assert_eq!(panel_extent_for(PanelPosition::Bottom, 1117.0, 1117.0, None), 335.0); // unchanged
}

#[test]
fn side_width_floor_never_exceeds_the_cap() {
    use PanelPosition::Right;
    assert_eq!(clamp_panel_extent(Right, 600.0, 100.0), 300.0); // 50% of 600 < 340
    assert_eq!(panel_extent_for(Right, 1728.0, 600.0, None), 300.0); // small external display
}

#[test]
fn pinned_toast_avoids_the_panel_on_every_edge() {
    let work = Rect { x: -2560.0, y: -241.0, w: 2560.0, h: 1410.0 };
    use PanelPosition::*;
    assert_eq!(toast_rect(work, 1.0, None), Rect { x: -1490.0, y: 1023.0, w: 420.0, h: 132.0 });
    assert_eq!(toast_rect(work, 1.0, Some((Bottom, 360.0))), Rect { x: -1490.0, y: 655.0, w: 420.0, h: 132.0 });
    assert_eq!(toast_rect(work, 1.0, Some((Top, 360.0))), toast_rect(work, 1.0, None));
    assert_eq!(toast_rect(work, 1.0, Some((Left, 400.0))), Rect { x: -1286.0, y: 1023.0, w: 420.0, h: 132.0 });
    assert_eq!(toast_rect(work, 1.0, Some((Right, 400.0))), Rect { x: -1694.0, y: 1023.0, w: 420.0, h: 132.0 });
}

#[test]
fn unknown_position_falls_back_to_bottom() {
    assert_eq!(PanelPosition::parse("left"), PanelPosition::Left);
    assert_eq!(PanelPosition::parse(""), PanelPosition::Bottom);
    assert_eq!(PanelPosition::Right.ratio_key(), "panelWidthRatio");
    assert_eq!(PanelPosition::Top.ratio_key(), "panelHeightRatio");
}
```

- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib window` → compile errors / FAIL.
- [ ] **Step 3:** Implement the items under **Produces**. Callers outside tests may be left as stubs passing `PanelPosition::Bottom` until Task 3, but the crate must compile.
- [ ] **Step 4:** Run the same command → all `window` tests PASS (including the pre-existing ones, updated only in signature).
- [ ] **Step 5:** Commit `feat: compute panel and toast rects per position`.

### Task 3: Rust wiring (open, resize, reset, toast, live move)

**Files:**
- Modify: `src-tauri/src/app/window.rs` (`place_and_show`, `resize_panel`, `reset_panel_height` → `reset_panel_size`, `show_toast`, `base_height` → `base_extent`, `stored_ratio`, `commands` submodule, new `place_panel`)
- Modify: `src-tauri/src/lib.rs:107-108` (command list)
- Modify: `src-tauri/src/domain/settings/commands.rs` (`set_setting`)
- Modify: `src/domain/panel/apis/panel.ts`, `src/app/settings/SettingsApp.tsx` (call-site rename only)

**Interfaces:**
- Consumes: Task 2 functions.
- Produces:
  - `fn stored_position(app: &AppHandle) -> PanelPosition`; `fn stored_ratio(app: &AppHandle, pos: PanelPosition) -> Option<f64>` (reads `pos.ratio_key()`); `fn base_extent(app: &AppHandle, pos: PanelPosition, fallback: f64) -> f64` (primary work-area width for sides, height otherwise).
  - `pub fn resize_panel(app: &AppHandle, size: f64, save: bool) -> AppResult<()>` — `size` along the current axis; keeps the screen-side edge put (falls out of `panel_rect`); saves to `pos.ratio_key()`.
  - `pub fn reset_panel_size(app: &AppHandle) -> AppResult<()>` — clears the current position's ratio key only, re-places a visible panel.
  - `pub fn place_panel(app: &AppHandle)` — re-places a visible panel from stored position/ratio on its current monitor; no-op when hidden.
  - Tauri commands `resize_panel { size, save }` and `reset_panel_size` (renamed from `reset_panel_height`).
  - TS: `resizePanel(size: number, save: boolean)` (`call('resize_panel', { size, save })`), `resetPanelSize()`.
  - `show_toast` passes `Some((stored_position(app), live_extent))` for a pinned panel, where live extent is `outer_size().width` for sides, `.height` otherwise, divided by scale factor.
  - `set_setting`: `if key == "panelPosition" { window::place_panel(&app) }` (before `changed(&app)`).

- [ ] **Step 1:** Make the changes above. `place_and_show` uses `panel_rect(work, unit, pos, panel_extent_for(pos, base_extent(app, pos, w), w, stored_ratio(app, pos)))` with `w` = current work height or width in points.
- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `cargo clippy --manifest-path src-tauri/Cargo.toml` if the repo uses it (see [testing](../../agents/testing.md)), `yarn typecheck` → PASS.
- [ ] **Step 3:** Commit `feat: open, resize and reset the panel at its configured edge`.

### Task 4: Frontend pure helpers (drag math, side keys)

**Files:**
- Create: `src/utils/panelEdge.ts`, `src/utils/panelEdge.test.ts`
- Modify: `src/utils/keys.ts`, `src/utils/keys.test.ts`

**Interfaces:**
- Consumes: `PanelPosition` (Task 1).
- Produces:
  - `export function isSide(p: PanelPosition): boolean`
  - `export interface DragStart { size: number; x: number; y: number }`
  - `export function dragSize(p: PanelPosition, start: DragStart, x: number, y: number): number` — bottom `size + (start.y - y)`, top `size + (y - start.y)`, left `size + (x - start.x)`, right `size + (start.x - x)`.
  - `KeyInput.side: boolean` — when true, `ArrowDown` → `{ type: 'move', delta: 1 }`, `ArrowUp` → `{ type: 'move', delta: -1 }` (only reached when not suggesting). Left/Right unchanged in both modes.

- [ ] **Step 1: Write the failing tests:**

```ts
// panelEdge.test.ts
void test('dragging the inner edge outward grows the panel on every edge', () => {
  const s = { size: 300, x: 500, y: 500 };
  assert.equal(dragSize('bottom', s, 500, 450), 350);
  assert.equal(dragSize('top', s, 500, 550), 350);
  assert.equal(dragSize('left', s, 550, 500), 350);
  assert.equal(dragSize('right', s, 450, 500), 350);
});
void test('only left and right are side positions', () => {
  assert.deepEqual(PANEL_POSITIONS.filter(isSide), ['left', 'right']);
});

// keys.test.ts (add `side: false` to the `press` defaults)
void test('side panel moves between cards with up and down', () => {
  assert.deepEqual(press('ArrowDown', { side: true }), { type: 'move', delta: 1 });
  assert.deepEqual(press('ArrowUp', { side: true }), { type: 'move', delta: -1 });
  assert.deepEqual(press('ArrowRight', { side: true }), { type: 'move', delta: 1 });
  assert.equal(press('ArrowDown'), null);
});
void test('side panel up and down still drive an open suggestion list', () => {
  assert.deepEqual(press('ArrowDown', { side: true, suggesting: true }), { type: 'suggestMove', delta: 1 });
});
```

- [ ] **Step 2:** `yarn test` → FAIL (module / field missing).
- [ ] **Step 3:** Implement. Pass `side: isSide(settings.panelPosition)` where `PanelApp` builds `KeyInput` (find the `panelKeyAction({ … })` call).
- [ ] **Step 4:** `yarn test`, `yarn typecheck` → PASS.
- [ ] **Step 5:** Commit `feat: add panel edge drag math and side arrow keys`.

### Task 5: Settings UI (position picker, hints, reset)

**Files:**
- Create: `src/app/settings/_components/PositionPicker.tsx`
- Modify: `src/app/settings/SettingsApp.tsx` (panel tab), `src/shared/i18n/en.ts`, `src/shared/i18n/ko.ts`
- Possibly modify: `src/app/globals.css` (only if a preview token is missing; both themes)

**Interfaces:**
- Consumes: `PANEL_POSITIONS`, `PanelPosition`, `isSide`, `resetPanelSize`.
- Produces: `PositionPicker({ value, onChange, labelledBy }: { value: PanelPosition; onChange: (v: PanelPosition) => void; labelledBy: string })`, default export, modeled on `ThemePicker.tsx` (radiogroup/radio, `arrowStep`, lift + primary border on the selected tile, `ease-spring duration-450`).
- i18n keys under `settings` (replace `panelResizeHint` / `panelSizeResetBody`):
  - `panelPosition`: `'Panel position'` / `'패널 위치'`
  - `panelPositionHint`: `'Screen edge the panel opens on'` / `'패널을 띄울 화면 가장자리'`
  - `panelPositions: { bottom, top, left, right }`: `Bottom/Top/Left/Right` / `하단/상단/왼쪽/오른쪽`
  - `panelResizeHints: { bottom, top, left, right }`:
    - en: `"Drag the panel's top edge to change its height (up to 50% of the screen)"`, `"…bottom edge…height…"`, `"…right edge…width…"`, `"…left edge…width…"`
    - ko: `'패널 위 가장자리를 끌어 높이를 바꿉니다 (최대 화면의 50%)'`, `'패널 아래 가장자리를 끌어 높이를 바꿉니다 (최대 화면의 50%)'`, `'패널 오른쪽 가장자리를 끌어 폭을 바꿉니다 (최대 화면의 50%)'`, `'패널 왼쪽 가장자리를 끌어 폭을 바꿉니다 (최대 화면의 50%)'`
  - `panelSizeResetBodies: { height, width }`: en `'Return the panel to its default height. Continue?'` / `'Return the panel to its default width. Continue?'`; ko `'패널 높이를 기본값으로 되돌립니다. 계속할까요?'` / `'패널 폭을 기본값으로 되돌립니다. 계속할까요?'`

- [ ] **Step 1:** Build `PositionPicker`. Layout (approved mock-up v4): the tiles sit in `Row col` under the label row; `grid grid-cols-4`, each tile `w-[84px] justify-self-center`; screen preview `aspect-[16/10] rounded-lg` with a bar on the matching edge (≈28% tall for top/bottom, ≈24% wide for sides); muted bar and border, primary when selected. Preview colors via existing tokens (`bg-preview-light`-style or `bg-subtle` / `border-base-300` / `bg-primary`); add a token pair in `globals.css` only if needed.
- [ ] **Step 2:** In the panel tab, put `<Row col label={s.panelPosition} hint={s.panelPositionHint} labelId="position-label"><PositionPicker … labelledBy="position-label" /></Row>` first in the section; resize hint becomes `s.panelResizeHints[settings.panelPosition]`; reset button disabled on `isSide ? panelWidthRatio === '' : panelHeightRatio === ''`; confirm body `s.panelSizeResetBodies[isSide ? 'width' : 'height']`; confirm runs `resetPanelSize()`.
- [ ] **Step 3:** `yarn typecheck`, `yarn lint` → PASS.
- [ ] **Step 4:** `yarn tauri dev` (separate identifier if Vee is installed, see [build-run](../../agents/build-run.md)): Settings → Panel in light/dark × ko/en. Tiles fit one row in Korean and English, keyboard arrows move the selection, the hint changes with the tile.
- [ ] **Step 5:** Commit `feat: pick the panel position in settings`.

### Task 6: Panel side layout, resize handle, motion

**Files:**
- Modify: `src/app/panel/PanelApp.tsx`, `src/app/panel/_components/Card.tsx`, `src/app/panel/_components/FileIcon.tsx`, `src/app/panel/_components/Toolbar.tsx`, `src/app/panel/_components/ResizeHandle.tsx`, `src/app/panel/_components/motion.ts`

**Interfaces:**
- Consumes: `isSide`, `dragSize`, `resizePanel(size, save)`, `settings.panelPosition`.
- Produces:
  - `cardLefts` → `export function cardOffsets(container: HTMLElement | null, side: boolean): Map<number, number>` (left or top); `playFlip(container, before, side: boolean)` animates `translateY` when `side`.
  - `Card` gets `side: boolean` prop: side → `w-full h-40` instead of `w-[calc((100vh-72px)*0.95)]`; text tile uses a fixed `text-[13px]` and fewer clamped lines (fit 160pt) instead of the `100vh` clamp. `FileIcon` gets the same `side` prop with fixed sizes in place of `100vh` widths.
  - `ResizeHandle` reads `settings.panelPosition`: strip on the inner edge (bottom→top edge, top→bottom edge, left→right edge, right→left edge), `cursor-ns-resize` / `cursor-ew-resize`, grab bar vertical on sides; drag uses `dragSize` with `innerHeight`/`innerWidth` as `start.size`.

- [ ] **Step 1: Side layout** in `PanelApp`: when `side`, body is `flex-col`; pinned group stacks vertically with a horizontal divider (`h-px w-auto mx-2 my-1.5`); history list `flex-col overflow-y-auto overflow-x-hidden`, no wheel remap, `onScroll` loads more on `scrollTop + clientHeight > scrollHeight - 400`. Reposition `pinNotice` so it shows over the pinned group. Pass `side` to `Card`, `cardOffsets`, `playFlip`.
- [ ] **Step 2: Slide direction:** replace `translate-y-[calc(100%+16px)]` with a per-position class map: bottom `translate-y-[calc(100%+16px)]`, top `-translate-y-[calc(100%+16px)]`, left `-translate-x-[calc(100%+16px)]`, right `translate-x-[calc(100%+16px)]`; `transition-[translate]` already covers both axes. Bottom unchanged.
- [ ] **Step 3: Toolbar** at ≥340pt width: search grows, filters and icon buttons must stay usable (wrap to a second line or compact); follow `docs/design/aurora-glass.md`. The `@app` suggestion list's `max-h-[calc(100vh-70px)]` stays valid.
- [ ] **Step 4: ResizeHandle** per the Interfaces block.
- [ ] **Step 5:** `yarn test`, `yarn typecheck`, `yarn lint` → PASS.
- [ ] **Step 6: Manual check** in `yarn tauri dev`, each of the 4 positions × light/dark × ko/en, on a light and a dark desktop:
  - Open/close slides from the docked edge; reduced motion still disables it.
  - Bottom looks identical to `master`.
  - Side: 3 pinned cards + several history cards visible; scrolling loads more; Up/Down and Left/Right move selection; Enter copies; pin, delete-confirm, drag-out (panel slides out toward its edge) and drag-convert work.
  - Resize: drag on top/bottom/left/right, release, reopen → size kept; switching position keeps the other axis's size; Settings → reset resets only the current axis.
  - Pinned panel + copy elsewhere: toast clears the panel at every position. Change position in Settings while pinned → panel moves at once.
- [ ] **Step 7:** Commit `feat: dock the panel on any screen edge`.

### Task 7: What's New size

**Files:**
- Modify: `src-tauri/tauri.conf.json` (`whats-new` window)

- [ ] **Step 1:** `"width": 520, "height": 640`, keep `"resizable": false`.
- [ ] **Step 2:** Look at the What's New window in `yarn tauri dev` (trigger as described in `src/app/whats-new/WhatsNewApp.tsx` / [build-run](../../agents/build-run.md)): content fills the larger window without odd gaps; light/dark, ko/en.
- [ ] **Step 3:** Commit `feat: enlarge the what's new window`.

### Task 8: Final verification

- [ ] **Step 1:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` → all PASS (paste output).
- [ ] **Step 2:** Confirm `cargo check` passes for the Windows target if the toolchain is available; otherwise note that Windows was not compiled locally.
- [ ] **Step 3:** Update `docs/agents/architecture.md` / `overview.md` lines that say the panel is "at the bottom of the screen" to mention the position setting. Commit `docs: describe panel position`.
