# Update Pill Jump Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the panel's "new version" pill a solid green pill that loops a squash-and-stretch jump (with a landing shadow) until hovered.

**Architecture:** Pure CSS. Two `@theme` animation tokens plus keyframes in `src/app/globals.css`; `Toolbar.tsx` only swaps classes and adds a shadow `<span>`. No JS state, no new files.

**Tech Stack:** Tailwind 4 `@theme` animations, React (Toolbar component).

**Spec:** `docs/superpowers/specs/2026-11-02-update-pill-jump-design.md`

## Global Constraints

- Colors only from `globals.css` tokens: `bg-success`, `text-success-content`; no hex in components.
- Keep the `update` class on the wrapper div and the confirm popover markup unchanged.
- New animations must be listed in the `prefers-reduced-motion: reduce` block.
- Jump height ≈ 7px (panel top padding is 10px, panel is `overflow-hidden`).
- Animation pauses only while hovered (`hover:animate-none`), resumes on leave.
- Code comments in English.

## Review Focus

- Panel top edge: the jump's highest frame must not be clipped by the panel's `overflow-hidden` (check in the running app).
- Side-docked panel (`in-data-side`, toolbar wraps): the pill must not shift neighbors while jumping (transform only, no layout props).
- Reduced motion on: pill is solid green and static, shadow static.
- Hover over the shadow area / pill edge: pausing must not flicker as the pill moves out from under the cursor (pause on the button's `:hover`, which the shadow child is part of).
- Confirm popover open: the popover is a sibling of the button, so it must not jump.

No automated test: the change is CSS classes and keyframes with no logic; `yarn test` has nothing to assert. Verification is visual.

---

### Task 1: Jump animation on the update pill

**Files:**
- Modify: `src/app/globals.css` (`@theme` motion block ~line 297-323, keyframes ~325-430, reduced-motion block ~553-566)
- Modify: `src/app/panel/_components/Toolbar.tsx:214-221`
- Modify: `docs/agents/libs/tailwind-daisyui.md:23` (list the new animations)

**Interfaces:**
- Produces: utilities `animate-jump` (the pill) and `animate-jump-shadow` (its shadow), both 1.8s infinite, same easing so they stay in phase.

- [ ] **Step 1: Add tokens** in `@theme` after `--animate-bob`, with a one-line comment ("The update pill hops until hovered; ~7px so the panel's 10px top padding never clips it."):
  - `--animate-jump: jump 1.8s cubic-bezier(0.3, 0.7, 0.4, 1) infinite;`
  - `--animate-jump-shadow: jump-shadow 1.8s cubic-bezier(0.3, 0.7, 0.4, 1) infinite;`

- [ ] **Step 2: Add keyframes** next to `bob` (values from mockup E, height reduced to 7px):
  - `jump`: `0%,55%,100% { transform: translateY(0) scale(1,1) }`, `60% { scale(1.12, .85) }`, `72% { translateY(-7px) scale(.95, 1.08) }`, `84% { translateY(0) scale(1.08, .9) }`, `90% { scale(.98, 1.03) }`
  - `jump-shadow`: `0%,55%,100% { opacity: .9; transform: scaleX(1) }`, `72% { opacity: .25; transform: scaleX(.6) }`, `84% { opacity: 1; transform: scaleX(1.15) }`

- [ ] **Step 3: Reduced motion** — add `.animate-jump` and `.animate-jump-shadow` to the `animation: none` selector list.

- [ ] **Step 4: Toolbar markup** — on the update `<button>`:
  - classes: replace `bg-ok-bg text-ok-fg` with `bg-success text-success-content`; add `group relative animate-jump hover:animate-none` (keep the rest).
  - replace the dot `<i className="size-1.5 rounded-full bg-current" aria-hidden />` with `<span aria-hidden>↑</span>`.
  - add, as the button's first child, the shadow: `<span className="bg-success/45 animate-jump-shadow pointer-events-none absolute inset-x-[15%] -bottom-1.5 h-1 rounded-full blur-[2px] group-hover:animate-none" aria-hidden />`.

- [ ] **Step 5: Docs** — append `animate-jump`, `animate-jump-shadow` to the motion list in `docs/agents/libs/tailwind-daisyui.md:23`.

- [ ] **Step 6: Run the four checks**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib && mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint`
Expected: all pass.

- [ ] **Step 7: Visual check in the dev app** (`mise exec -- yarn tauri dev --config '{"identifier":"com.bobpark.vee.dev"}'`)
  - The pill only shows when an update is `ready`. If the dev build has no ready update, temporarily force `updateVersion` in `PanelApp.tsx:154` to a literal (e.g. `'9.9.9'`) for the check and **revert before committing**.
  - Ask the user to open and pin the dev panel; capture with `agent-desktop screenshot --screen 0`, crop to the panel.
  - Check: light/dark, 한국어/English, side-docked, hover pauses and leave resumes, top of jump not clipped. Tune keyframe numbers if needed.

- [ ] **Step 8: Commit**

```bash
git add src/app/globals.css src/app/panel/_components/Toolbar.tsx docs/agents/libs/tailwind-daisyui.md
git commit -m "feat: jumping update pill in panel"
```
