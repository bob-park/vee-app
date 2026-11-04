# Capture Overlay Toolbar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the capture overlay's text hint with a clickable glass toolbar (capture↔record, area↔window, cancel) shown only on the cursor's monitor, add Tab to switch capture↔record, restyle the record pill above it, and round the window highlight's corners.

**Architecture:** Rust gains a `set_capture_mode` command that flips every overlay's `payload.mode` and keeps `recorder::Phase` in step (Idle↔Selecting). Rust also sends a side-effect-free "why recording is blocked" value in the session payload so the UI can disable Record in advance. The frontend splits the toolbar and the record pill into `src/app/capture/_components/` and wires them in `CaptureApp.tsx`.

**Tech Stack:** Tauri 2 (Rust), React 19 + TypeScript, Tailwind 4 + daisyUI 5, `node:test`.

**Spec:** `docs/superpowers/specs/2026-11-04-capture-overlay-toolbar-design.md`

## Global Constraints

- Branch `feature/capture-overlay-polish`. Commit prefixes per `docs/agents/workflow/commit-messages.md` (`feat:`, `fix:`, `docs:` …). End every commit with the attribution trailer lines from the session.
- Colors only through `globals.css` tokens. The red Start button uses the existing `rec` token (`bg-rec`). Do not write hex in components.
- Every user-visible string lives in `src/shared/i18n/{en,ko}.ts`. Remove `capture.regionHint` / `capture.windowHint`.
- Component bodies follow `docs/agents/conventions/react-sections.md` section order. Code comments are in English.
- Window highlight radius: **12** (macOS), **8** (Windows), in logical px.
- Toolbar order: `[캡처|녹화] · [영역|창] · ✕`. No keycap hints on it.
- Keys: `Space` area↔window (unchanged), `Tab` capture↔record (new; normal focus traversal only while this overlay shows the record pill), `Esc` cancel, `Enter` start a waiting recording (unchanged).
- When recording is blocked: dim the Record segment, show the reason as its `title` tooltip, and ignore Tab. No toast, no Settings window.
- No dependency changes.
- Done = `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` all pass, plus a look at the running app in light/dark and ko/en.

## Review Focus

1. **Capture → record → Esc / capture → record → capture → Esc:** afterwards the record shortcut must open a record overlay again (the recorder must be back at `Idle`). Owned by Task 2 (`set_mode_*` tests) and Task 5 (manual check).
2. **Capture overlay opened while a recording runs:** Record is dimmed with the "already recording" tooltip, and Tab does nothing. Owned by Task 1 (`block_reason` test) and Task 5.
3. **Two monitors:** the toolbar shows only under the cursor and moves with it. A record selection's pill hides when the cursor leaves that monitor and comes back with it. Owned by Task 4 (manual check; there is no DOM test harness).
4. **Clicking the toolbar or the pill never starts a drag or a window pick.** Both stop `pointerdown` propagation. Owned by Task 4.
5. **Switching mode with a waiting record selection** clears it on every monitor. The capture cursor and record cursor follow the new mode. Owned by Task 4.

---

### Task 1: Recorder — block reason and adopting an open overlay

**Files:**
- Modify: `src-tauri/src/app/recorder/mod.rs` (`Event`, `next`, `can_record` ~252, tests ~730)

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)] #[serde(rename_all = "camelCase")] pub enum RecordBlock { Recording, Unsupported, NeedsFfmpeg }`
  - `fn block_reason(idle: bool, supported: bool, has_ffmpeg: bool) -> Option<RecordBlock>`: pure. The order is `Recording` (not idle), then `Unsupported`, then `NeedsFfmpeg`.
  - `pub(crate) fn record_block(app: &AppHandle) -> Option<RecordBlock>`: `idle = phase(app) == Phase::Idle`, `supported = screen_access::recording_supported()`, `has_ffmpeg = cfg!(not(windows)) || engines::ffmpeg(app).is_some()`. No side effects.
  - `Event::Adopt { session: u64 }`: `(Idle, Adopt{session}) → (Selecting{session}, Action::Nothing)`. Every other phase falls through to `(p, Nothing)`.
  - `pub(crate) fn adopt_overlay(app: &AppHandle) -> bool`: if `record_block(app).is_some()` return false. Otherwise take the next session number the same way `on_shortcut` does, `dispatch(app, Event::Adopt { session })`, and return `phase(app) == Phase::Selecting { session }`.
  - `can_record` keeps its toasts and Settings, but decides from `record_block`. `Recording` is never its concern, because `on_shortcut` only calls it when idle.

- [ ] **Step 1: Write failing tests** in the existing `tests` module:

```rust
#[test]
fn block_reason_orders_busy_then_os_then_ffmpeg() {
    assert_eq!(block_reason(true, true, true), None);
    assert_eq!(block_reason(false, false, false), Some(RecordBlock::Recording));
    assert_eq!(block_reason(true, false, false), Some(RecordBlock::Unsupported));
    assert_eq!(block_reason(true, true, false), Some(RecordBlock::NeedsFfmpeg));
}

#[test]
fn adopt_selects_only_when_idle() {
    assert_eq!(next(&Phase::Idle, Event::Adopt { session: 7 }), (Phase::Selecting { session: 7 }, Action::Nothing));
    assert_eq!(next(&SEL, Event::Adopt { session: 7 }), (SEL, Action::Nothing));
    assert_eq!(next(&recording(1), Event::Adopt { session: 7 }), (recording(1), Action::Nothing));
}
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --lib recorder` → fails to compile (`block_reason`, `Adopt` missing).
- [ ] **Step 3:** Implement the interfaces above. The `windows`-only `can_record` branch keeps its exact toast and `show_settings(app, "convert")`.
- [ ] **Step 4:** Same command → PASS, and the existing recorder tests still pass.
- [ ] **Step 5: Commit** `feat: let a capture overlay hand its session to the recorder`

---

### Task 2: Capture session — mode switch command

**Files:**
- Modify: `src-tauri/src/app/capture/mod.rs` (`Mode`, `OpenPayload`, `open`, `commands`, tests)
- Modify: `src-tauri/src/lib.rs` (~175, `generate_handler!`)

**Interfaces:**
- Consumes: `recorder::{RecordBlock, record_block, adopt_overlay, overlay_closed}` (Task 1).
- Produces:
  - `Mode` also derives `Deserialize` (`camelCase`: `"capture"` / `"record"`).
  - `OpenPayload.record_blocked: Option<RecordBlock>` (JSON `recordBlocked`). `open` fills it with `recorder::record_block(app)` when `mode == Capture`, and with `None` for `Record`.
  - `fn set_mode(slot: &mut Option<Session>, mode: Mode) -> Option<Vec<String>>`: pure. It sets `payload.mode = mode` on every overlay, and sets `record_blocked = None` when `mode == Record`. When the mode actually changed, it returns the overlay labels to notify; it returns `None` if no session is open or the mode is already `mode`.
  - Command `set_capture_mode(app: AppHandle, mode: Mode) -> AppResult<Mode>`. It returns the mode now in effect. Error `AppError::InvalidInput("capture overlay is not open")` when there is no session.
    - Read the current mode under the lock, then drop the lock. If it equals `mode`, return it.
    - To `Record`: `if !recorder::adopt_overlay(&app) { return Ok(current) }`. The UI already disabled Record; this is only a race guard, so it stays silent.
    - To `Capture`: `recorder::overlay_closed(&app)`.
    - Re-lock and `set_mode`. If the session vanished meanwhile and the target was `Record`, call `recorder::overlay_closed(&app)` so the recorder isn't left in `Selecting`.
    - `emit_to(label, "capture://session-mode", mode)` for each returned label.
  - Event `capture://session-mode` with payload `"capture" | "record"`.

- [ ] **Step 1: Write failing tests** in the capture `tests` module (reuse its `session(generation)` helper):

```rust
#[test]
fn set_mode_flips_every_overlay_once() {
    let mut slot = session(1); // two overlays, Capture
    let labels = set_mode(&mut slot, Mode::Record).unwrap();
    assert_eq!(labels.len(), slot.as_ref().unwrap().overlays.len());
    assert!(slot.as_ref().unwrap().overlays.iter().all(|o| o.payload.mode == Mode::Record && o.payload.record_blocked.is_none()));
    assert_eq!(set_mode(&mut slot, Mode::Record), None);
    assert!(set_mode(&mut None, Mode::Capture).is_none());
}
```

Adjust the helper's `OpenPayload` literal for the new field (`record_blocked: None`). If the helper has one overlay, add a second so the test proves "every".

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --lib capture` → fails to compile.
- [ ] **Step 3:** Implement. Register `capture::commands::set_capture_mode` in `lib.rs`. No capability change is needed (app command; overlays already match `capture*`).
- [ ] **Step 4:** Run the full `cargo test --manifest-path src-tauri/Cargo.toml --lib` → PASS.
- [ ] **Step 5: Commit** `feat: switch an open capture overlay between capture and record`

---

### Task 3: Frontend contract, i18n, and an icon-capable Segmented

**Files:**
- Modify: `src/domain/capture/apis/capture.dto.ts`, `src/domain/capture/apis/capture.ts`
- Modify: `src/shared/components/Segmented.tsx`
- Modify: `src/shared/i18n/en.ts`, `src/shared/i18n/ko.ts` (and the dictionary type if it is declared separately)

**Interfaces:**
- Produces:
  - `export type RecordBlock = 'recording' | 'unsupported' | 'needsFfmpeg';` and `CaptureOpen.recordBlocked: RecordBlock | null`.
  - `export async function setCaptureMode(mode: CaptureMode) { return call<CaptureMode>('set_capture_mode', { mode }); }`
  - `Segmented` option: `label: ReactNode` (was `string`) and optional `activeClassName?: string`. When selected, `activeClassName` replaces `text-base-content`. Existing callers are unchanged.
  - i18n `capture` block (ko / en):
    - `modeLabel: '모드' / 'Mode'`
    - `capture: '캡처' / 'Capture'`
    - `record: '녹화' / 'Record'`
    - `targetLabel: '선택 방식' / 'Select'`
    - `area: '영역' / 'Area'`
    - `window: '창' / 'Window'`
    - `cancel: '취소' / 'Cancel'`
    - `recordBusy: '이미 녹화 중이에요' / 'Already recording'`
    - Keep `regionHint` and `windowHint` for now; Task 4 removes them together with their only use.
  - Tooltip copy for the other block reasons reuses `record.unsupported` and `record.ffmpegNeeded`.
  - `record.start` becomes `'녹화 시작' / 'Start recording'`, as in the mockup.

- [ ] **Step 1:** Make the changes.
- [ ] **Step 2:** `yarn typecheck && yarn lint && yarn test` → PASS.
- [ ] **Step 3: Commit** `feat: add the capture mode switch to the frontend contract`

---

### Task 4: Toolbar, record pill, Tab, cursor-monitor visibility, rounded highlight

**Files:**
- Create: `src/app/capture/_components/CaptureToolbar.tsx`
- Create: `src/app/capture/_components/RecordPill.tsx`
- Modify: `src/app/capture/CaptureApp.tsx`
- Modify: `src/shared/i18n/{en,ko}.ts` (drop `regionHint` / `windowHint`)

**Interfaces:**
- Consumes: `setCaptureMode`, `RecordBlock`, `CaptureOpen.recordBlocked`, `Segmented` with `ReactNode` labels (Task 3); event `capture://session-mode` (Task 2).
- Produces:
  - `CaptureToolbar({ mode, windowMode, recordBlocked, onMode, onWindowMode, onCancel }: { mode: CaptureMode; windowMode: boolean; recordBlocked: RecordBlock | null; onMode: (m: CaptureMode) => void; onWindowMode: (on: boolean) => void; onCancel: () => void })`
    - Glass pill: `glass shadow-glass rounded-full p-1`, positioned `absolute bottom-10 left-1/2 -translate-x-1/2`.
    - Contents: Segmented (capture icon + label with `activeClassName="text-primary"`, record dot + label with `activeClassName="text-rec"`, disabled with a `title` when `recordBlocked`), a 1px `bg-divider` separator, Segmented (area / window icons + labels), a separator, and an icon-only ✕ button with `aria-label`.
    - Its root stops `pointerdown` propagation.
  - `RecordPill({ format, ffmpeg, onFormat, onStart })`
    - The same glass pill, positioned one row above the toolbar (`bottom-[88px]` or measured, implementer's call; it must not overlap).
    - Contents: the existing format `Segmented` and a `bg-rec text-white rounded-full` Start button with a white dot.
    - No ✕. It also stops `pointerdown` propagation.
  - Inline SVG icons (camera, corner-brackets area, window), copied from the mockup `.superpowers/brainstorm/69955-1793772844/content/toolbar-a.html`.

- [ ] **Step 1: Mode switching in `CaptureApp`.**
  - A `handleMode(next)`: if `next === open.mode` or (`next === 'record' && open.recordBlocked`), return. Otherwise call `void setCaptureMode(next)`.
  - Listen to `capture://session-mode`. Update `open`/`openRef` with the new `mode` (and `recordBlocked: null` when it is `'record'`), then clear `pending`, `windowPicked`, `dragFrom` and `picked`. Keep `windowMode`.
  - `select` already branches on `openRef.current.mode`, so it follows.
- [ ] **Step 2: Tab.** In the global keydown handler, `e.key === 'Tab' && !e.repeat && openRef.current`:
  - If this overlay currently renders the record pill, let the browser handle Tab.
  - Otherwise `preventDefault()` and `handleMode(other mode)`.
  - Read the pill's visibility and `open` from refs so the handler stays registered once.
- [ ] **Step 3: Cursor-monitor visibility.** Render the toolbar and the pill only when `pointer !== null && !dragFrom && !leaving`. The pill additionally needs `pending`. Delete the old hint `<div>` and the i18n keys `regionHint`/`windowHint`.
- [ ] **Step 4: Rounded highlight.**
  - `const WINDOW_RADIUS = isMac ? 12 : 8;`
  - Pass `rx`/`ry` to the tint `rect`, its mask `rect`, and the `planned` rect when `pending?.selection.kind === 'window'`.
  - The region-drag `outline` and `hole` stay square.
- [ ] **Step 5:** `yarn typecheck && yarn lint && yarn test` → PASS.
- [ ] **Step 6: Commit** `feat: capture overlay toolbar with mode switch and rounded window highlight`

---

### Task 5: Verify in the running app

**Files:** none (fix-ups go into the task that owns the code)

- [ ] **Step 1:** Run all four checks: `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` → all PASS.
- [ ] **Step 2:** `yarn tauri dev`. If Vee is installed, use the separate identifier per `docs/agents/build-run.md`. Drive it with agent-desktop per `docs/agents/ui-testing.md`.
- [ ] **Step 3:** Check each Review Focus line 1–5 in both light and dark theme and in both 한국어 and English:
  - capture → record → Esc, then the record shortcut opens again
  - Tab cycles capture↔record
  - Space still toggles area↔window
  - the window tint has rounded corners
  - the pill sits above the toolbar with the red Start button
  - with a recording running, opening the capture overlay shows Record dimmed with a tooltip
- [ ] **Step 4:** Note anything not checkable on this machine (Windows radius 8, multi-monitor if only one display) in the PR description.
