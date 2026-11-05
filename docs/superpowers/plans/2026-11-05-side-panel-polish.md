# Side Panel Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the panel-size-reset hang and the off-screen update popover on left/right panels, let side panels shrink to 260pt with icon-only filters, and swap the pin icon for a red pushpin.

**Architecture:** One Rust fix in `src-tauri/src/app/window.rs` (lock order + min-width constant). The rest is CSS-first in `src/app/panel/_components/Toolbar.tsx`: the existing `in-data-side:` variant drives the three-row side layout, a Tailwind 4 container query (`@container` / `@max-[…]`) turns filter labels into icons, and new `--vee-pin-*` tokens in `src/app/globals.css` colour the pushpin.

**Tech Stack:** Tauri 2 (Rust), React 19, Tailwind 4 + daisyUI 5, `node:test`.

**Spec:** `docs/superpowers/specs/2026-11-05-side-panel-polish-design.md`

## Global Constraints

- Colours only in `src/app/globals.css`; every new token gets a value in both the light and dark `[data-theme]` blocks and is exposed via `@theme inline`.
- No new dependencies (`package.json`, `Cargo.toml` untouched). Icons are inline SVG (Lucide, ISC), credited in a comment like the existing pin/settings icons.
- No new user-visible strings: filter tooltips and `aria-label`s reuse `t.filters[f]`.
- Keep the `update` DOM hook class on the update wrapper.
- Bottom/top panel toolbar must look exactly as before.
- Every new animation stops under the reduced-motion block in `globals.css` (none planned; the pin keeps `animate-pin-drop` / `animate-pin-pull`).
- Code and comments in English; commit messages `<prefix>: <subject>` with the attribution trailer.
- Done = `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` all pass (run through `mise exec --`).

## Review Focus

1. **Reset while the panel is hidden or bottom-docked** must also return immediately. The lock fix is position-independent; check by hand in Task 5 with a saved height on a bottom panel and a saved width on a side panel, panel open and closed.
2. **Update popover on a 260pt side panel**: inner width ~236px, so the popover must cap at the toolbar width (`max-w-full`), not overflow the window. Checked by hand in Task 5.
3. **Filter breakpoint in Korean vs English**: the label row must never clip or wrap at the switch-over width in either language; the breakpoint is measured on the longer of the two (Task 4).
4. **Dragging across the breakpoint**: labels ↔ icons switch live with no layout jump of the sliding pill (the pill uses column math, which stays valid because the grid keeps `auto-cols-fr`). Checked by hand in Task 5.
5. **Pinned-card count after the toolbar gains a row**: `pinsThatFit` must still leave one whole history card. Pinned by the updated tests in Task 3.

## Files

- `src-tauri/src/app/window.rs`: `reset_panel_size` lock order, `PANEL_MIN_WIDTH`, width tests.
- `src/app/globals.css`: `--vee-pin`, `--vee-pin-hi`, `--vee-pin-needle` tokens.
- `src/app/panel/_components/Toolbar.tsx`: pushpin SVG, side row 3, popover anchoring, responsive filters.
- `src/utils/panelEdge.ts` + `src/utils/panelEdge.test.ts`: `CHROME` re-measured for the three-row toolbar.

A scan for other `db.lock()` held across a call that locks again found only `reset_panel_size` (`window.rs:494`), so nothing else is in scope.

---

### Task 1: Rust: reset no longer deadlocks, side floor drops to 260pt

**Files:**
- Modify: `src-tauri/src/app/window.rs:31` (`PANEL_MIN_WIDTH`), `:493-498` (`reset_panel_size`), tests `side_width_defaults_to_a_quarter_with_a_340pt_floor`, `side_width_floor_never_exceeds_the_cap`

**Interfaces:**
- Produces: `PANEL_MIN_WIDTH = 260.0`; `reset_panel_size(app: &AppHandle) -> AppResult<()>` keeps its signature.

- [ ] **Step 1: Update the width tests**

Rename `side_width_defaults_to_a_quarter_with_a_340pt_floor` → `side_width_defaults_to_a_quarter_with_a_260pt_floor`:

```rust
assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, None), 432.0);
assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, Some(0.1)), 260.0); // floor beats 15% (259.2)
assert_eq!(panel_extent_for(Left, 1728.0, 1728.0, Some(0.7)), 864.0); // 50% cap
assert_eq!(panel_extent_for(PanelPosition::Bottom, 1117.0, 1117.0, None), 335.0); // unchanged
```

In `side_width_floor_never_exceeds_the_cap`, the cap must still beat the floor:

```rust
assert_eq!(clamp_panel_extent(Right, 400.0, 100.0), 200.0); // 50% of 400 < 260
assert_eq!(panel_extent_for(Right, 1728.0, 400.0, None), 200.0); // small external display
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib side_width`
Expected: FAIL (`left: 340.0, right: 260.0`).

- [ ] **Step 3: Set `PANEL_MIN_WIDTH` to `260.0` and fix the lock order**

In `reset_panel_size`, bind `let key = stored_position(app).ratio_key();` before the `settings::set_raw(&…db.lock().unwrap(), key, "")?` line, plus a one-line comment that `stored_position` takes the same lock. Grep the doc comment on `PANEL_MIN_WIDTH` and any "340" in comments in this file and update them.

- [ ] **Step 4: Run the Rust suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: PASS.

- [ ] **Step 5: Commit**

`fix(panel): stop panel size reset from deadlocking, let side panels shrink to 260pt`

---

### Task 2: Red pushpin icon

**Files:**
- Modify: `src/app/globals.css` (light block near `--vee-ok-solid-fg` at :114, dark block near :154, `@theme inline` at :175)
- Modify: `src/app/panel/_components/Toolbar.tsx:263-287` (pin `<svg>`)

**Interfaces:**
- Produces: utility colours `text-pin`, `text-pin-hi`, `text-pin-needle` (via `--color-pin`, `--color-pin-hi`, `--color-pin-needle`).

- [ ] **Step 1: Add the tokens**

Light: `--vee-pin: #e5484d; --vee-pin-hi: #ff8a8d; --vee-pin-needle: #8b8fa3;`
Dark: `--vee-pin: #ff6369; --vee-pin-hi: #ffa3a6; --vee-pin-needle: #a6a9ba;`
`@theme inline`: `--color-pin: var(--vee-pin);` etc.

- [ ] **Step 2: Replace the Lucide pin paths with the pushpin**

Keep the `<svg>` element, its `className` (size and `animate-pin-drop` / `animate-pin-pull`), `viewBox="0 0 24 24"`, and `aria-hidden`. Drop `stroke="currentColor"` from the root. Paths (from the approved mockup):

```tsx
<path className="stroke-pin-needle" d="M11 14.5 4.5 21" strokeWidth="2" strokeLinecap="round" />
<path className="fill-pin" d="M14.6 2.6a1.2 1.2 0 0 1 1.7 0l5.1 5.1a1.2 1.2 0 0 1 0 1.7l-1.2 1.2-1.6-.5-3.1 3.1.7 3.4a1 1 0 0 1-.3.9l-.8.8L6.7 10l.8-.8a1 1 0 0 1 .9-.3l3.4.7 3.1-3.1-.5-1.6z" />
<path className="stroke-pin-hi" d="M15.4 3.6 19.9 8" strokeWidth="1.3" strokeLinecap="round" opacity="0.8" />
```

Pinned state: the button background stays `bg-accent-subtle`, and the icon tilts `-30deg` (`panelPinned && '-rotate-30'` with `transition-transform duration-200 ease-spring`), replacing the old head fill. Update the comment to say it's a custom pushpin, not Lucide.

- [ ] **Step 3: Typecheck and lint**

Run: `mise exec -- yarn typecheck && mise exec -- yarn lint`
Expected: PASS.

- [ ] **Step 4: Commit**

`feat(panel): red pushpin icon for the pin button`

---

### Task 3: Three-row side toolbar with the update popover kept inside

**Files:**
- Modify: `src/app/panel/_components/Toolbar.tsx:132` (root), `:172` (tablist), `:210-288` (spacer, update, pin/settings group)
- Modify: `src/utils/panelEdge.ts:33` (`CHROME`), `src/utils/panelEdge.test.ts:18-25`

**Interfaces:**
- Consumes: the `in-data-side:` variant already on the toolbar.
- Produces: in side mode, row 3 is a single element `div.flex.items-center` holding the update wrapper (`.update`) and the pin/settings group.

- [ ] **Step 1: Restructure the right-hand cluster**

Wrap the existing `{updateVersion && <div className="update …">…</div>}` and the pin/settings `<div>` in one `<div className="flex items-center gap-1.5 in-data-side:basis-full in-data-side:justify-between">`. When there is no update, the pin/settings group keeps `ml-auto` so it stays right. Give the tablist `in-data-side:basis-full` so filters take row 2. Remove the pin/settings group's own `in-data-side:ml-auto` if the wrapper now does the job. On bottom/top the wrapper sits after the `flex-1` spacer, so the look doesn't change.

- [ ] **Step 2: Anchor the popover left on side panels**

On the popover `div` add `in-data-side:right-auto in-data-side:left-0 in-data-side:origin-top-left in-data-side:max-w-[calc(100vw-24px)]`. 24px is the panel's horizontal padding; confirm it against PanelApp's root padding and use that number. Keep `w-60` for bottom/top.

- [ ] **Step 3: Re-measure the side toolbar and update `CHROME`**

In `yarn tauri dev` with a side panel, read the toolbar's height in devtools (rows + gaps + `mb-2.5`). Set `CHROME = 22 + <toolbar height> + 14` and update its comment. Then compute the expected `pinsThatFit` values for (432, 1101), (340, 1101), (260, 1101), (340, 1904) with the new `CHROME`. Write them into `panelEdge.test.ts`: rename the first test to cover 432/340/260 and add `assert.equal(pinsThatFit(260, 1101), <computed>)`. Keep the `(1280, 684) → 0` test.

- [ ] **Step 4: Run frontend tests and checks**

Run: `mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint`
Expected: PASS.

- [ ] **Step 5: Commit**

`fix(panel): give side panels a third toolbar row and keep the update prompt inside`

---

### Task 4: Filters turn into icons on a narrow toolbar

**Files:**
- Modify: `src/app/panel/_components/Toolbar.tsx` (root `div` at :132, `FILTERS.map` at :182-197, new `FILTER_ICONS` const near `ICON_BTN` at :46)

**Interfaces:**
- Consumes: `FILTERS: Filter[]` (Toolbar.tsx:13; `Filter` from `@/domain/clip/apis/clip.dto`), `t.filters[f]`, `Tip`.
- Produces: `FILTER_ICONS: Record<Filter, ReactNode>`; `Tip` with optional `accel`. Lucide `layout-grid` (all), `type` (text), `image` (image), `file` (files), `link` (link), each a `size-4` inline SVG with `stroke="currentColor"`.

- [ ] **Step 1: Make the toolbar a container**

Add `@container` to the toolbar root `div`.

- [ ] **Step 2: Render label + icon per tab**

Each tab button renders `<span className="@max-[Npx]:hidden">{t.filters[f]}</span>` and `<span className="hidden @max-[Npx]:block">{FILTER_ICONS[f]}</span>`, gets `aria-label={t.filters[f]}`, and becomes a `group`. It renders `<Tip label={t.filters[f]} className="left-1/2 -translate-x-1/2 @min-[Npx]:hidden" />` so the tooltip only shows in icon mode. Make `Tip`'s `accel` optional (`accel?: string`) and render the keycap `<span>` only when it is set: `acceleratorKeys('')` returns `['']`, which would draw one empty keycap. The tab padding in icon mode keeps the current height (`py-1`) so the click target doesn't shrink.

- [ ] **Step 3: Pick N**

In `yarn tauri dev`, drag a side panel narrower in ko and en, then set N to the toolbar width where the longer label row stops fitting, plus 4px slack. Expected around 300. Use one literal N in all three classes.

- [ ] **Step 4: Run checks**

Run: `mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint`
Expected: PASS.

- [ ] **Step 5: Commit**

`feat(panel): show filter icons when the side panel is narrow`

---

### Task 5: Verify in the running app

- [ ] **Step 1:** `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `mise exec -- yarn test`, `mise exec -- yarn typecheck`, `mise exec -- yarn lint`: all PASS.
- [ ] **Step 2:** `yarn tauri dev` (separate identifier if the installed Vee is running, per `docs/agents/build-run.md`), driven with agent-desktop per `docs/agents/ui-testing.md`. Check:
  - Reset panel size returns at once on bottom (saved height) and on left/right (saved width), with the panel open and with it hidden.
  - Side panel at 260pt: three rows; with an update ready, the popover opens rightward and stays inside. To force the pill, temporarily point the status at `ready` locally and revert before committing.
  - Dragging width across N switches labels and icons; the pill tracks the selected tab; the icon tooltip shows the name.
  - Bottom/top toolbar unchanged.
  - Pushpin idle and pinned, with the press/pull animation, in light and dark, 한국어 and English, against light and dark desktops.
- [ ] **Step 3:** Commit any fixes the checks turn up, with `fix(panel): …` messages.
