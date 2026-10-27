---
title: Build & Run
scope: "**"
applies_to: running the app, building, packaging
related:
  - ./testing.md
  - ./configuration.md
  - ./workflow/release.md
---

# Build & Run

> `yarn tauri dev` runs the app with hot reload. `yarn tauri build --no-bundle` checks a release compile without signing keys. A full `yarn tauri build` needs the updater signing key. Releases go through the scripts in [release](./workflow/release.md).

## Setup

```bash
mise install
export GITHUB_NPM_AUTH_TOKEN=…   # PAT with read:packages, for @bob-park/* lint configs
yarn install
```

Always use Yarn 4.18.1 (mise). A global Yarn 1 rewrites `yarn.lock`; if `yarn --version` isn't 4.x, run `corepack yarn@4.18.1 <command>`.

## Commands

| Command | What it does |
| ------- | ------------ |
| `yarn tauri dev` | Vite on :1420 + debug Rust app; frontend hot-reloads, Rust/config changes rebuild and restart |
| `yarn tauri build --no-bundle` | release compile, no `.app`/installer, no signing key needed |
| `yarn tauri build` | bundles (`.app`, `.dmg`, NSIS) + updater artifacts; needs `TAURI_SIGNING_PRIVATE_KEY` (first `source scripts/load-updater-key-macos.sh` on macOS, `. scripts/load-updater-key-windows.ps1` on Windows) |
| `scripts/release-macos.sh --build-only` | macOS `.app` + `.dmg` (Apple Silicon), Developer ID signed; both notarized and stapled with the App Store Connect API key; no upload ([configuration](./configuration.md#secrets)) |
| `yarn build` | frontend only (`tsc` + `vite build`) |
| `yarn lint` | ESLint over `src/` |
| `yarn prettier` | format `src/**/*.{ts,tsx,css}` (writes files) |

## Gotchas

- **It's a tray app:** all windows start hidden. Open the panel with the shortcut (`Cmd/Ctrl+Shift+V`) or the tray. Closing windows doesn't quit: quit from the tray before rerunning `yarn tauri dev`; otherwise single-instance just opens Settings in the old process (and on Windows the build fails with `EBUSY`).
- **An installed Vee shares the identifier**, so `yarn tauri dev` would open its data and fight over its shortcut. Run dev under its own identifier and give it another shortcut:
  ```bash
  yarn tauri dev --config '{"identifier":"com.bobpark.vee.dev"}'
  ```
- **The clipboard is live:** anything you copy while a dev build (or the installed Vee) runs is recorded. Don't seed test data through the real clipboard.
- **The updater** only runs in release builds.
- **After adding a JS dependency:** the first dev load may show a transient "Invalid hook call" while Vite re-optimizes deps. It is gone after the automatic reload.
- **React Query devtools** render only in the dev `settings` window (bottom-right button).
- **Dev data** lives in `~/Library/Application Support/<identifier>/` (macOS) or `%APPDATA%\<identifier>\` (Windows): `vee.db` + `images/` + `engines/{ffmpeg,rhwp}/` (once downloaded) + `lo-profile/` (after a LibreOffice conversion). Delete it to start fresh.
