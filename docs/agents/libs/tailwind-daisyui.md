---
title: Tailwind 4 + daisyUI 5
scope: src/**/*.{tsx,css}
applies_to: styling a component, adding a color token
related:
  - ../design.md
  - ./theme.md
---

# Tailwind 4 + daisyUI 5

> Tailwind v4 via `@tailwindcss/vite`, configured entirely in `src/app/globals.css` — the **only** stylesheet (no `tailwind.config.js`, no per-screen CSS). daisyUI 5 provides the `light`/`dark` themes only; vee draws its own components with utilities. Compose conditional classes with `classnames` (`cx`).

## `globals.css` layout

1. `@import 'tailwindcss';` and `@plugin 'daisyui' { themes: false; exclude: …; }`.
   - `themes: false` disables the built-in themes.
   - `exclude` drops daisyUI's root background/scroll-lock/scrollbar styles (they would paint the transparent panel and toast) and every daisyUI component vee doesn't use (`badge, button, card, dropdown, kbd, link, modal, select, stack, stat, tab, toast`).
2. Two `@plugin 'daisyui/theme'` blocks, `light` (default) and `dark` (`prefersdark`). They hold the core palette, radius and sizes.
3. `[data-theme='light']` / `[data-theme='dark']` blocks for vee tokens daisyUI has no slot for: `--vee-card`, `--vee-divider`, `--vee-subtle`, `--vee-muted`, `--vee-accent-subtle`, `--vee-badge-*`, `--vee-ok-*`, `--vee-toast-*`.
4. `@theme inline` maps those to Tailwind names (`bg-card`, `text-muted`, `border-divider`, `bg-badge-bg`, `bg-toast-bg`, …), adds theme-independent colors (`danger-*`, `backdrop`, `tip`, `keycap-*`, …) and sets `--font-sans` / `--font-mono`.
5. `@utility shadow-*` rules (`shadow-seg`, `shadow-card`, `shadow-card-selected`, `shadow-modal`, …).
6. `@theme` animations (`animate-toast-in`, `animate-toast-out`, `animate-toast-check-in`).
7. `@layer base`: `line-height: normal`, `overflow: clip` on `html, body, #root`, body 13px + `user-select: none`, transparent body for `html[data-window='panel'|'toast']`, the focus ring.
8. `@layer components`: `.ghost` (the dragged card copy).

## Rules

- **Adding a color:** if it differs per theme, add it to **both** `[data-theme]` blocks and expose it through `@theme inline`; otherwise add it to `@theme inline` directly. Never write a literal color in a component.
- **Shadows:** write a new shadow as a plain `@utility shadow-<name> { box-shadow: …; }`. A theme `--shadow-*` token goes through Tailwind's ring/inset shadow stack and renders small blurs a shade off. (`shadow-suggest` and `shadow-toast` are theme tokens that render correctly; leave them.)
- **The `exclude` list stays** even after a name disappears from the source. daisyUI's scanner matches any word (`toast` is a window label, `card` is a DOM hook class), so a removed entry ships dead CSS — or worse, styles a vee element.
- **Components:** use the existing building blocks and plain Tailwind utilities. Don't use daisyUI component classes (`btn`, `card`, `toggle`, …); most are excluded, and `card` is a DOM hook class here ([design](../design.md)).
- **Conditional classes:** use `cx('base', { 'on-class': cond })` from `classnames`. Don't build class strings with template literals.
- **Arbitrary values** (`w-[72px]`, `text-[11px]`): use them to match vee's existing metrics; prefer the scale otherwise.
- **Class order:** Prettier sorts classes. Run `yarn prettier`.
