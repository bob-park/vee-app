# Capture Window Mode Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In the capture/record overlay's window mode, round the tint around overlapping windows, show a blocked cursor and an in-overlay notice over the desktop, and replace the pick cursors with the "Tilted Sticker" set.

**Architecture:** Frontend only. `coveredParts` returns whole front-window rects that the SVG mask draws rounded; `CaptureApp` derives "pointer over no window" from `windowAt` and uses it for the cursor class and a click notice; the notice is a small glass float next to the toolbar.

**Tech Stack:** React 19, Tailwind 4 + daisyUI 5 (`globals.css` utilities), `node:test`.

**Spec:** `docs/superpowers/specs/2026-11-07-capture-window-polish-design.md`

## Global Constraints

- Cursors stay 32px with a 2x `image-set` (WebView2 hides bigger custom cursors near the viewport edge); hotspot `16 16`.
- User-visible text only in `src/shared/i18n/{en,ko}.ts`.
- Colors only in `globals.css`; components use token classes (`glass`, `bg-glass-float`, `text-base-content`).
- No Rust or dependency changes.
- Component bodies keep the section comment order (`// ref`, `// state`, …, `// handle`, `// memorize`).

## Review Focus

- Record mode with an area already pending (pill up), then Space to window mode and click the desktop: the notice must not overlap the record pill → Task 3 places it above the pill when `showPill`.
- Window already picked for recording (`windowPicked`): desktop clicks stay silent and the cursor stays `cursor-default` → Task 2/3 conditions include `!windowPicked`.
- A front window that only touches a corner of the hovered one: its rounded corner must leave the tint showing there → Task 1 test with a corner overlap.
- Rapid repeated desktop clicks: one notice whose 1.5 s timer restarts, no stacking → Task 3 keyed timer.
- Overlay closed or switched to area mode while the notice is up: it disappears at once → Task 3 clears it in `reset`, `capture://closed`, and on `windowMode` false.

---

### Task 1: Rounded tint around overlapping windows

**Files:**
- Modify: `src/utils/captureSelection.ts` (`coveredParts`)
- Modify: `src/app/capture/CaptureApp.tsx` (the `tint-visible` mask)
- Test: `src/utils/captureSelection.test.ts`

**Interfaces:**
- Produces: `coveredParts(windows: WinInfo[], win: WinInfo): Rect[]`, same signature, now returning each front window's **whole** rect (`{x, y, width, height}` of that window) when it overlaps `win`.

- [ ] **Step 1: Update the existing `coveredParts` test** (around line 70) so the expected values are the front windows' full rects, e.g. `coveredParts([apart, front, back, behind], back)` equals `[{ x: front.x, y: front.y, width: front.width, height: front.height }]`; keep `coveredParts([front, back], front)` → `[]` and the "apart" window excluded.
- [ ] **Step 2: Run** `mise exec -- yarn test` → the updated test FAILS.
- [ ] **Step 3: Implement.** Keep the overlap check, push the front window's own rect instead of the intersection. Update the doc comment: "The windows in front of `win` that overlap it; drawn rounded, so their corners leave `win` showing."
- [ ] **Step 4:** In `CaptureApp.tsx`, give each covered `<rect className="fill-black">` in the mask `{...corner}`. The white tint rect already bounds the mask, so nothing outside the tint shows.
- [ ] **Step 5: Run** `mise exec -- yarn test` → PASS.
- [ ] **Step 6: Commit** `fix(capture): round the tint around windows in front`.

### Task 2: Tilted Sticker cursors and the blocked cursor

**Files:**
- Modify: `src/app/globals.css` (`cursor-camera`, `cursor-record`, new `cursor-blocked`, the comment above them)
- Modify: `src/app/capture/CaptureApp.tsx` (cursor class)

**Interfaces:**
- Produces: in `CaptureApp`, `const overDesktop = windowMode && !windowPicked && pointer !== null && windowAt(open.windows, pointer) === null` (computed after the `if (!open) return null`, or reuse the hover result from the `shapes` memo by also returning `hover` from it). Task 3 uses `overDesktop`'s logic for clicks.

- [ ] **Step 1: Replace the cursor SVGs** with the approved mockup (`.superpowers/brainstorm/31782-1794053868/content/cursor-options.html`, card D). Per utility: plain `url(...) 16 16, <fallback>` line, then `image-set(1x 32px, 2x 64px) 16 16, <fallback>`. Gradient ids must be unique per SVG; `#` in colors is `%23`.
  - `cursor-camera`: `<g transform='rotate(-8 16 16)'>` rect `x=3 y=3 w=26 h=26 rx=10`, linear gradient `#a98bff → #7132f5 (.55) → #5fc3ff`, white 2px stroke, four white shutter triangles + center dot. Fallback `pointer`.
  - `cursor-record`: same shape, gradient `#ff8a9a → #e5283c (.6) → #ffb3d9`, white circle r 7 at 0.3 opacity, white circle r 4.5. Fallback `pointer`.
  - `cursor-blocked`: `rotate(8 16 16)`, fill `#6b6f7d`, white 2px stroke, white circle r 6.5 stroke 2 + diagonal `M11.4 20.6l9.2-9.2`. Fallback `not-allowed`.
- [ ] **Step 2:** Update the comment above the utilities: "tilted sticker badges: aurora camera shutter for capture, red record dot for recording, grey no-entry over the desktop (tilted the other way)".
- [ ] **Step 3:** In the root `cx`, the window-mode branch becomes: `windowPicked ? 'cursor-default' : overDesktop ? 'cursor-blocked' : record ? 'cursor-record' : 'cursor-camera'`.
- [ ] **Step 4:** `pointer` changes don't call `refreshCursor` today and the class change on mouse move is picked up by the browser by itself; only add `refreshCursorSoon()` if Step 5 shows a stale cursor.
- [ ] **Step 5: Verify** `mise exec -- yarn typecheck && mise exec -- yarn lint`, then `yarn tauri dev`: window mode in capture and record, cursor over a window vs desktop/menu bar/Dock, on a Retina screen.
- [ ] **Step 6: Commit** `feat(capture): tilted sticker cursors and a blocked cursor over the desktop`.

### Task 3: "No window here" notice on desktop clicks

**Files:**
- Create: `src/app/capture/_components/DesktopNotice.tsx`
- Modify: `src/app/capture/CaptureApp.tsx`
- Modify: `src/shared/i18n/en.ts`, `src/shared/i18n/ko.ts` (`capture` block)

**Interfaces:**
- Consumes: `floatClass(shown)` from `./float`; `useI18n()`.
- Produces: `DesktopNotice({ shown, mode, abovePill }: { shown: boolean; mode: CaptureMode; abovePill: boolean })`.
- i18n keys: `capture.noWindowCapture`, `capture.noWindowRecord`.

- [ ] **Step 1: Copy.**
  - en: `noWindowCapture: 'No window here to capture'`, `noWindowRecord: 'No window here to record'`
  - ko: `noWindowCapture: '여기에는 캡처할 창이 없어요'`, `noWindowRecord: '여기에는 녹화할 창이 없어요'`
- [ ] **Step 2: Component.** Always mounted, eases via `floatClass(shown)`. Classes: `glass shadow-glass text-base-content pointer-events-none absolute left-1/2 -translate-x-1/2 rounded-full px-3 py-1.5 text-[12px] font-medium whitespace-nowrap`, plus `bottom-[88px]` or, when `abovePill`, `bottom-[136px]`. `role="status"` + `aria-live="polite"`; text from `mode`.
- [ ] **Step 3: State in `CaptureApp`.** `const [notice, setNotice] = useState(false)` and `const noticeTimerRef = useRef<number | undefined>(undefined)`. `showNotice()` clears the timer, sets `true`, sets a 1500 ms timeout to `false`. Clear (timer + `false`) in `reset`, in the `capture://closed` and `claim` handlers, and in an effect when `windowMode` becomes false. Clear the timer on unmount.
- [ ] **Step 4: Click.** In `handlePointerDown`'s window branch: `if (win) select(...) else showNotice();` (still after the `windowPicked` early return).
- [ ] **Step 5: Render** `<DesktopNotice shown={notice && showToolbar} mode={open.mode} abovePill={showPill} />` next to `RecordPill`.
- [ ] **Step 6: Verify** `mise exec -- yarn typecheck && mise exec -- yarn lint && mise exec -- yarn test`; `yarn tauri dev`: click the desktop in capture and record window modes, double-click quickly, press Space while it shows, Esc while it shows, pending area + Space + desktop click (sits above the pill); light/dark and ko/en.
- [ ] **Step 7: Commit** `feat(capture): say when a window-mode click hits no window`.

### Final check

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `mise exec -- yarn test`, `mise exec -- yarn typecheck`, `mise exec -- yarn lint` all pass.
