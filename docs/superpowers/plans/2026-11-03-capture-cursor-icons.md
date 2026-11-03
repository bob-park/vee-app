# Capture/Record Cursor Icons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the capture overlay's window-pick cursors with the chosen badge design (A) and make them render at full size and sharp on HiDPI (Windows 150%/200%).

**Architecture:** CSS-only. The two `@utility` blocks in `src/app/globals.css` get new 32×32 SVG data URLs, declared through `image-set()` with a `1x` and a `2x` entry (the 2x SVG has `width/height=64`, same `viewBox`), preceded by a plain `url()` declaration as a fallback.

**Tech Stack:** Tailwind 4 `@utility`, WKWebView (macOS), WebView2/Chromium (Windows).

**Spec:** `docs/superpowers/specs/2026-11-03-capture-cursor-icons-design.md`

## Global Constraints

- Only `cursor-camera` and `cursor-record` change; `cursor-crosshair` and every other cursor stay as they are.
- Cursor size stays at 32 CSS px (WebView2 hides custom cursors larger than 32px near the viewport edge); hotspot `16 16`; final fallback keyword `pointer`.
- Colors are fixed hex in the data URL: capture `#7132f5`, record `#e5283c`; `#` is written `%23`.
- No dependency changes, no user-visible strings (no i18n change).
- Code comments in English.

## Review Focus

- Pointer near a screen edge on Windows: must stay the custom icon, not fall back to `pointer` (holds because size ≤ 32 CSS px).
- Lightning CSS (Tailwind 4) may drop the plain `url()` fallback declaration when it sees `image-set()`; the built CSS must still contain `image-set(` for both utilities.
- WKWebView: if `image-set()` in `cursor` is unsupported, the plain `url()` fallback must apply, not `pointer`.
- Icon must stay legible on white, near-black and photo backgrounds (2px white rim).
- Hotspot is the centre: a window is picked under the badge centre, not its corner.

---

### Task 1: New badge cursors with HiDPI image-set

**Files:**
- Modify: `src/app/globals.css:225-240` (the `cursor-camera` and `cursor-record` utilities and their comments)

**Interfaces:**
- Consumes: nothing.
- Produces: the same utility names `cursor-camera`, `cursor-record` (used by `src/app/capture/CaptureApp.tsx:242-243`, unchanged).

SVG sources (32×32 viewBox; for the 2x entry change only `width='64' height='64'`):

```
camera: <svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'><rect x='2' y='2' width='28' height='28' rx='9' fill='%237132f5' stroke='white' stroke-width='2'/><path d='M9 13a2 2 0 0 1 2-2h1.8l1.2-1.6h4l1.2 1.6H21a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H11a2 2 0 0 1-2-2z' fill='white'/><circle cx='16' cy='16.3' r='3' fill='%237132f5'/></svg>
record: <svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'><rect x='2' y='2' width='28' height='28' rx='9' fill='%23e5283c' stroke='white' stroke-width='2'/><circle cx='16' cy='16' r='6.5' fill='none' stroke='white' stroke-width='2'/><circle cx='16' cy='16' r='3.8' fill='white'/></svg>
```

Declaration shape for each utility:

```css
cursor: url("data:…32…") 16 16, pointer;
cursor: image-set(url("data:…32…") 1x, url("data:…64…") 2x) 16 16, pointer;
```

- [ ] **Step 1: Replace both utilities** with the shape above; update the comments (capture: purple badge with a camera; record: red badge with a record button; one line on why `image-set` + 32px cap).
- [ ] **Step 2: Build and check the output keeps image-set**

Run: `mise exec -- yarn build && grep -o "image-set(url(\"data:image/svg+xml,<svg[^)]*" dist/assets/*.css | wc -l`
Expected: `4` or more (two entries × two utilities, possibly prefixed). If 0, Lightning CSS rewrote it: drop the plain `url()` line or switch to `-webkit-image-set` and re-check.

- [ ] **Step 3: Run the four checks**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib && mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint`
Expected: all pass.

- [ ] **Step 4: Look at it in the dev app** (`yarn tauri dev`, see `docs/agents/build-run.md`): open capture → window mode, then record → window mode; confirm the purple/red badges over a light and a dark window, sharp on the Retina display, and that the window under the badge centre is the one highlighted.

- [ ] **Step 5: Commit**

```bash
git add src/app/globals.css docs/superpowers/specs/2026-11-03-capture-cursor-icons-design.md docs/superpowers/plans/2026-11-03-capture-cursor-icons.md
git commit -m "feat: badge cursors for capture and record window pick, sharp on HiDPI"
```

- [ ] **Step 6: Ask the user to check on Windows** (150%/200% scale): size, sharpness, and near-edge behaviour. If still too small, the spec's fallback is approach 2 (DOM-drawn cursor), a new change.
