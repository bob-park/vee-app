---
title: Theme (light / dark / system)
scope: src/shared/providers/theme/**, src/app/main.tsx, src/utils/theme.ts
applies_to: theme switching, reading the active theme
related:
  - ./tailwind-daisyui.md
  - ./react-query.md
  - ../design.md
---

# Theme

> The preference (`system` | `light` | `dark`) is the `theme` key in SQLite, read as part of `useSettings()`. `ThemeProvider` resolves it with `resolveTheme(pref, prefersDark)` and writes `<html data-theme>`. With `system`, it follows `prefers-color-scheme` live. Changes reach every window via `settings://changed`.

## Flow

1. **Startup (`main.tsx`):** awaits `getSettings()`, seeds the settings query, and calls `applyTheme(settings.theme)` before the first render, so there is no flash.
2. **`ThemeProvider`:**
   - Applies the theme whenever the preference changes.
   - While the preference is `system`, it subscribes to `matchMedia('(prefers-color-scheme: dark)')`.
   - It listens for `settings://changed` and invalidates the settings query.
3. **Settings → General → Theme:** a `Segmented` control (System / Light / Dark) that calls `useSettingUpdate` with `{ key: 'theme', value }`. Rust validates the value, stores it, and emits `settings://changed`.

## Rules

- **Pure logic:** keep it in `src/utils/theme.ts` (`resolveTheme`), tested with `node:test`.
- **Reading the theme in a component:** read `document.documentElement.dataset.theme`, or `useSettings().settings?.theme` if you need the preference itself.
- **Theme-specific styling:** use the tokens. Never branch on the theme in JS to choose colors.
