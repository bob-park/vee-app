# Capture Smooth Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the capture/record overlay flow move smoothly end to end, and make the macOS cursor change the instant the area/window mode changes.

**Architecture:** A pure easing + rect interpolation helper in `src/utils/motion.ts` drives a small `requestAnimationFrame` hook that glides the window-mode hover tint from window to window and fades it in and out. On macOS a new `refresh_cursor` command posts a synthetic in-process `mouseMoved` NSEvent to the overlay window (no Accessibility permission needed), so WKWebView re-reads the CSS cursor without the mouse moving. The recording pill fades out before Rust hides it, using the frame's existing generation guard.

**Tech Stack:** React 19, Tailwind 4 / daisyUI 5, Tauri 2, Rust, `objc2-app-kit` 0.3.2 (already a dependency).

**Spec:** `docs/superpowers/specs/2026-11-07-capture-smooth-motion-design.md`

## Global Constraints

- No new dependencies (`package.json`, `Cargo.toml` unchanged).
- Motion follows `docs/design/aurora-glass.md`: moves use `EASE_MOVE` `cubic-bezier(0.32, 0.72, 0, 1)`, exits 150–250ms `ease-in`, enters `ease-spring`.
- Under `prefers-reduced-motion: reduce` every new motion is off and the final state shows at once.
- The rect being dragged stays glued to the pointer (no glide).
- CSS durations and the Rust hide delays that wait for them stay in step (name the pairing in a comment on both sides).
- Code and comments in English. No new user-visible strings (so no i18n change).
- Done means all four pass: `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` (through `mise exec --`), and the running app is checked in light/dark and ko/en.

## Review Focus

- Moving fast across many windows: each new target must restart the glide from where the tint is drawn now, not from the last target (no jump back). Pinned by the `glideFrom` test in Task 2.
- Pointer leaves every window (desktop) then enters one: the tint fades in at the new window, not slide in from the old one. Pinned by the "hidden→a target: jump" `nextGlide` test in Task 2.
- Space pressed with the pointer on another monitor: `refresh_cursor` on an overlay that doesn't have the pointer must be harmless. Task 3 posts only to the calling window; the event at an out-of-window location is ignored by WebKit — verified by hand in Task 3 step 5.
- A new recording started within 150ms of the last one stopping: the delayed pill hide must not hide the new pill. Pinned by the generation guard in Task 4 (manual check in step 5).
- Reduced motion on: tint snaps, pill hides at once. Checked by hand in Task 5.

Calls made while planning (not in the spec): the glide applies to the hover tint only. `hole` and `planned` change only on a click and the drag rect must not glide, so gliding them adds nothing. Capture completion stays instant: Rust hides the overlay before the grab and a fade would end up in the screenshot.

---

### Task 1: Easing and rect interpolation helpers

**Files:**
- Modify: `src/utils/motion.ts`
- Test: `src/utils/motion.test.ts`

**Interfaces:**
- Produces:
  - `cubicBezier(x1: number, y1: number, x2: number, y2: number): (t: number) => number`. Clamps `t` to [0, 1]. Returns exactly 0 at 0 and 1 at 1.
  - `lerpRect(a: Rect, b: Rect, k: number): Rect`. Linear per field. `Rect` is `{ x; y; width; height }`, declared locally as a `type` like the file's `Box`; `src/utils` must not import `@/` (node --test caveat, see `module-aliases`).
  - `easeMove: (t: number) => number`, which is `cubicBezier(0.32, 0.72, 0, 1)`.

- [ ] **Step 1: Write the failing tests** in `src/utils/motion.test.ts`, in the file's existing `node:test` style:
  - `cubicBezier endpoints`: `easeMove(0) === 0`, `easeMove(1) === 1`, `easeMove(-1) === 0`, `easeMove(2) === 1`.
  - `cubicBezier linear`: `cubicBezier(0, 0, 1, 1)(0.3)` within 1e-4 of `0.3`.
  - `easeMove front-loads`: `easeMove(0.5) > 0.85` (EASE_MOVE is about 0.9 at its midpoint) and the values at 0.1, 0.2 … 0.9 strictly increase.
  - `lerpRect`: `lerpRect({x:0,y:0,width:10,height:20},{x:10,y:10,width:30,height:40},0.5)` deep-equals `{x:5,y:5,width:20,height:30}`, and `k=0`/`k=1` return the endpoints.
- [ ] **Step 2: Run** `mise exec -- yarn test`. Expected: FAIL, `cubicBezier` is not exported.
- [ ] **Step 3: Implement** in `src/utils/motion.ts`. To solve x(s) = t, use a few Newton steps, then fall back to bisection when the slope is near 0 (about 8 Newton steps and 20 bisection steps). Return y(s).
- [ ] **Step 4: Run** `mise exec -- yarn test`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: add easing and rect interpolation helpers`

### Task 2: Glide the window-mode hover tint

**Files:**
- Create: `src/app/capture/_hooks/useGlide.ts`
- Modify: `src/utils/motion.ts`, `src/utils/motion.test.ts` (pure step function)
- Modify: `src/app/capture/CaptureApp.tsx` (tint rendering, around the `{tint && (...)}` block)

**Interfaces:**
- Consumes: `easeMove`, `lerpRect` (Task 1); `reducedMotion` pattern from `src/app/panel/_components/motion.ts` (re-implement as a `matchMedia` check inside the hook. Don't import panel code into capture).
- Produces:
  - Pure, in `src/utils/motion.ts`: `nextGlide(prev: { from: Rect; to: Rect; drawn: Rect; shown: boolean } | null, target: Rect | null): { from: Rect; to: Rect; drawn: Rect; shown: boolean; restart: boolean } | null`. This decides what happens when the target changes:
    - target `null`: keep the last rect with `shown: false` (`null` if there was none);
    - shown→a new target: `from = prev.drawn`, `restart: true`;
    - hidden (or none)→a target: jump, `from = to = target`, `shown: true`, `restart: false`;
    - same target (equal fields): return `prev` unchanged with `restart: false`.
  - Hook: `useGlide(target: Rect | null, ms?: number): { rect: Rect | null; shown: boolean }`. Default `ms = GLIDE_MS = 180`. It runs `requestAnimationFrame` while a glide is in flight, cancels it on change and unmount, and snaps when reduced motion is on.

- [ ] **Step 1: Write the failing tests** for `nextGlide` in `src/utils/motion.test.ts`. There is one test per bullet above, plus `glideFrom`: when `prev.drawn` is midway between A and B and the target becomes C, the result has `from` deep-equal to `prev.drawn` (not A, not B).
- [ ] **Step 2: Run** `mise exec -- yarn test`. Expected: FAIL.
- [ ] **Step 3: Implement** `nextGlide`, then `useGlide`. The hook stores the `nextGlide` state in a ref. On `restart` it animates `drawn = lerpRect(from, to, easeMove(elapsed / ms))` and writes state each frame. Compare targets by value (the memo in `CaptureApp` creates new objects every render).
- [ ] **Step 4: Wire it in `CaptureApp.tsx`.** Use `const glide = useGlide(tint)` (hooks section, before the early `return null`). Always render the tint `<mask>`/`<rect>` when `glide.rect` is set. Wrap them in a `<g>` with `transition-opacity duration-150 motion-reduce:transition-none` and `opacity-0` when `!glide.shown`, so the tint fades instead of cutting. Draw the tint rects from `glide.rect` and keep the `covered` mask parts as they are. Keep the `{...corner}` radius.
- [ ] **Step 5: Run** `mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint`. Expected: all PASS.
- [ ] **Step 6: Commit** `feat: glide the capture window highlight between windows`

### Task 3: Refresh the macOS cursor on mode change

**Files:**
- Modify: `src-tauri/src/app/capture/mod.rs` (new `refresh_cursor` fn + command in `commands`)
- Modify: `src-tauri/src/lib.rs:175-178` (register `capture::commands::refresh_cursor`)
- Modify: `src/domain/capture/apis/capture.ts` (wrapper)
- Modify: `src/app/capture/CaptureApp.tsx` (call sites)

**Interfaces:**
- Produces:
  - Rust: `#[tauri::command] pub fn refresh_cursor(app: AppHandle, window: WebviewWindow)`. It is a no-op off macOS. On macOS it runs on the main thread (`app.run_on_main_thread`) and takes the `NSWindow` from `window.ns_window()` (same SAFETY pattern as `raise_above_menu_bar`). It builds `NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure` with `NSEventType::MouseMoved`, `location = ns_window.mouseLocationOutsideOfEventStream()`, empty flags, timestamp 0, `windowNumber = ns_window.windowNumber()`, no context, event number 0, click count 0, pressure 0. It posts with `ns_window.postEvent_atStart(&event, false)`. Comment why: WKWebView only re-reads the CSS cursor on a mouse event.
  - TS: `export async function refreshCursor() { return call<void>('refresh_cursor'); }` in `capture.ts`, with a one-line doc comment like its neighbours.
  - Capability: `capture*` already covers the window; check `src-tauri/capabilities/*.json` and `docs/agents/conventions/tauri-commands.md` to see whether app commands need listing. Add it only if the convention says so.

- [ ] **Step 1: Implement the Rust command** and register it. No unit test: the body is a single AppKit call with no logic of its own.
- [ ] **Step 2: Run** `cargo test --manifest-path src-tauri/Cargo.toml --lib`. Expected: PASS. Then `cargo build --manifest-path src-tauri/Cargo.toml`. Expected: no warnings from the new code on macOS.
- [ ] **Step 3: Add the TS wrapper.** In `CaptureApp.tsx`, call `void refreshCursor()` right after `setWindowMode(...)` in three places: the Space handler, `handleWindowMode`, and the `capture://mode` listener (the other monitors' overlays). Also call it after `setOpen(session)` in `claim`, and after `capture://session-mode` (that one changes `cursor-camera` ↔ `cursor-record`). It must run after React commits the new class. Call it inside `requestAnimationFrame(() => void refreshCursor())`.
- [ ] **Step 4: Run** `mise exec -- yarn typecheck && mise exec -- yarn lint`. Expected: PASS.
- [ ] **Step 5: Check by hand** in `yarn tauri dev` (see `docs/agents/build-run.md` for a separate identifier if Vee is installed):
  - Open the capture overlay, keep the mouse still, and press Space. The cursor switches crosshair ↔ camera at once.
  - Press Tab with the mouse still. In window mode it switches camera ↔ record.
  - With two monitors, press Space while the pointer is on the other one. Nothing odd happens.
  - If the cursor does not update, stop and report back. The fallback (a `CGEventPost` mouse move, which needs the Accessibility permission) is a scope change for the user to decide.
- [ ] **Step 6: Commit** `fix(capture): show the mode's cursor without moving the mouse on macOS`

### Task 4: Fade the recording pill out

**Files:**
- Modify: `src-tauri/src/app/recorder/mod.rs` (`hide_controls`, ~line 545)
- Modify: `src/app/recorder/RecorderApp.tsx`

**Interfaces:**
- Consumes: the existing `FRAME_GENERATION` and `CROSSFADE_MS` (150) in `recorder/mod.rs`.
- Produces: `recorder://stopped` still means "the pill is going". It now arrives `CROSSFADE_MS` before the window is hidden.

- [ ] **Step 1: Rust.** In `hide_controls`, stop hiding `RECORDER` at once. Hide it inside the existing delayed thread, next to the frame, under the same `FRAME_GENERATION == generation` check, so a new recording's pill is never hidden. Update the doc comments on `CROSSFADE_MS` and `hide_controls` to say the pill shares the fade.
- [ ] **Step 2: Front.** In `RecorderApp.tsx`, keep the last `started` drawn after stop. Add `const [leaving, setLeaving] = useState(false)`. On `recorder://stopped`, `handleStop` and `handleCancel`, set `leaving` and stop the tick instead of clearing `started`. Set `leaving` back to false on `recorder://started` and on mount. On the pill's root `div`, add `transition-[opacity,scale] duration-150 ease-in motion-reduce:transition-none`, and add `pointer-events-none scale-95 opacity-0` while `leaving`. Disable the buttons while leaving, so a second click can't send stop twice. The old timer must never flash when the pill shows again. `recorder://started` replaces `started` before the window is shown, so the next show draws the new recording.
- [ ] **Step 3: Run** `cargo test --manifest-path src-tauri/Cargo.toml --lib && mise exec -- yarn typecheck && mise exec -- yarn lint`. Expected: PASS. Fix any recorder state-machine test that asserted an immediate hide.
- [ ] **Step 4: Commit** `feat(recorder): fade the recording pill out with its frame`
- [ ] **Step 5: Check by hand:**
  - Start a recording, then stop it. The pill and the frame fade out together.
  - Start, stop, and start again within about 150ms (the global shortcut twice). The second pill stays up.

### Task 5: Walk the whole flow and tune

**Files:**
- Modify: only what the walk turns up, limited to `src/app/capture/**`, `src/app/recorder/**`, `src/app/globals.css` (capture/record utilities) and the paired Rust delay constants
- Modify: `docs/design/aurora-glass.md` (motion table: add a row for "캡처 창 하이라이트 이동 · 180ms · EASE_MOVE")

- [ ] **Step 1: Walk the flow** in `yarn tauri dev` and note every hard cut or mismatched timing. Use `agent-desktop` per `docs/agents/ui-testing.md` for screenshots. Walk these:
  - open the overlay (capture and record)
  - Space and toolbar area↔window
  - hover across windows
  - drag an area
  - pick a window in record mode (the pill appears)
  - Tab to switch the mode
  - Esc
  - take a capture
  - start a recording (overlay→frame crossfade)
  - stop or cancel it
- [ ] **Step 2: Fix only the cuts on that list.** Match `aurora-glass.md`'s table and keep Rust waits paired with their CSS durations. Candidates already seen: the overlay's 150ms fade is `ease-out` in and default out; the frame's fade-out uses the default curve, so make it `ease-in`. Leave anything already smooth alone.
- [ ] **Step 3: Check reduced motion** (System Settings → Accessibility → Display → Reduce motion). The tint snaps, nothing fades, and the pill hides at once.
- [ ] **Step 4: Check** light/dark and ko/en (the toolbar labels and the glass over a light and a dark desktop).
- [ ] **Step 5: Update** the `aurora-glass.md` motion table.
- [ ] **Step 6: Run all four checks:** `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `mise exec -- yarn test`, `mise exec -- yarn typecheck`, `mise exec -- yarn lint`. Expected: all PASS.
- [ ] **Step 7: Commit** `fix(capture): even out overlay and recording fades` (include the doc change)
