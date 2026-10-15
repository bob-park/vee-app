---
title: Design
scope: src/**/*.{tsx,css}
applies_to: any UI work — new screen, component, color, spacing
related:
  - ./libs/tailwind-daisyui.md
  - ./libs/theme.md
  - ./conventions/react-sections.md
---

# Design

> **Read [`docs/design/`](../design/) before building or changing any UI.** Colors exist only in `src/app/globals.css`. Components use semantic classes. Check every change in light **and** dark, ko **and** en. A refactor must not change a single pixel.

## Sources of truth

1. [`docs/design/`](../design/) — the design system (`kraken-design.md`: purple brand, near-black text, cool gray neutrals, 12px radius, whisper-level shadows).
2. The existing screens: the settings tab bar, bordered section cards, label-left/control-right rows, segmented radio groups, switches, tinted buttons, the panel cards, Helvetica Neue at 13px.

When the two disagree, ask the user. Do not invent a third style.

## Rules

- **Colors live only in `src/app/globals.css`.**
  - The `light`/`dark` daisyUI theme blocks hold the core colors.
  - Tokens daisyUI has no slot for are `--vee-*` variables in the `[data-theme]` blocks, exposed through `@theme inline` (`bg-card`, `text-muted`, `border-divider`, `bg-badge-bg`, `bg-toast-bg`, …).
  - Never write a hex/rgb value in a component.
- **Use semantic classes:**
  - surfaces: `bg-base-100`, `bg-card`, `bg-subtle`
  - text: `text-base-content`, `text-muted`, `text-primary`
  - borders: `border-base-300`, `border-divider`
- A new themed token must get a value in **both** the light and dark blocks.
- **Reuse the settings building blocks** in `src/app/settings/_components/` before writing new ones:
  - `SettingSection` (bordered card), `Row` (label/hint left, control right)
  - `Segmented`: for a pick-one choice of 2–4 options, use this instead of a `<select>`
  - `Dropdown`: for longer lists (sounds, retention)
  - `Switch`: for on/off
  - `Button`, `ConfirmModal`, `ShortcutRecorder`
  - If another screen needs one, promote it to `src/shared/components/` first ([structure](./structure.md#placement-rules)).
- **DOM hook classes are read by code — don't rename or drop them:**
  - `card` + `data-id` on panel cards (`motion.ts` measures them for FLIP; `PanelApp` scrolls to `[data-id]`).
  - `update` on the panel's update button (outside clicks are detected with `closest('.update')`).
  - `ghost` on the dragged card copy (added by `motion.ts`, styled in `globals.css`).
- Every string comes from `useI18n().t` ([i18n](./libs/i18n.md)).
- **Accessibility:**
  - Keep the roles the building blocks use: `radiogroup`/`radio`, `switch`, `tablist`/`tab`.
  - Keep the `:focus-visible` ring.
  - Icon-only controls need an `aria-label`.
- **Check before you finish:**
  - Look at the screen in Light and Dark (Settings → General → Theme).
  - Look at it in 한국어 and English; Korean strings are often longer.
  - The panel and toast windows are transparent: check edges and shadows against both a light and a dark desktop.
