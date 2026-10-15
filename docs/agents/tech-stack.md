---
title: Tech Stack
scope: "**"
applies_to: checking a library or tool version, adding a dependency
related:
  - ./overview.md
  - ./build-run.md
---

# Tech Stack

> Versions are pinned in `.mise.toml` (Node/Yarn), `package.json` (frontend), and `src-tauri/Cargo.toml` (Rust). Ask the user before adding, removing, or bumping a dependency.

| Area | Choice | Pinned in |
| ---- | ------ | --------- |
| Toolchain | Node 26, Yarn 4.18.1 (`nodeLinker: node-modules`) via mise | `.mise.toml`, `package.json#packageManager` |
| Rust | stable, edition 2024 | `src-tauri/Cargo.toml` |
| Desktop shell | Tauri 2 (`tray-icon`, `image-png`, `macos-private-api`) | `src-tauri/Cargo.toml` |
| Tauri plugins | single-instance, log, dialog, global-shortcut, autostart, updater | `src-tauri/Cargo.toml` |
| DB | SQLite via `rusqlite` (`bundled`) | `src-tauri/Cargo.toml` |
| Errors | `thiserror` | `src-tauri/Cargo.toml` |
| Clipboard | `clipboard-rs` | `src-tauri/Cargo.toml` |
| Drag-out | `drag`, **vendored** in `src-tauri/vendor/drag` via `[patch.crates-io]` (centred drag image on Windows, `DragItem::Text`) | `src-tauri/Cargo.toml` |
| Hashing / encoding | `sha2` (clip dedup), `base64` (data URLs) | `src-tauri/Cargo.toml` |
| OS integration | macOS: `objc2`, `objc2-app-kit`, `objc2-foundation`; Windows: `windows`, `windows-icons` | `src-tauri/Cargo.toml` (target sections) |
| Images | `image` (different feature sets per OS) | `src-tauri/Cargo.toml` |
| OS locale | `sys-locale` (tray labels) | `src-tauri/Cargo.toml` |
| UI | React 19, TypeScript 6, Vite 8 | `package.json` |
| Tauri JS | `@tauri-apps/api`, `@tauri-apps/plugin-dialog` | `package.json` |
| Styling | Tailwind CSS 4 (`@tailwindcss/vite`) + daisyUI 5, `classnames` | `package.json` |
| Server state | `@tanstack/react-query` 5 (+ devtools in dev) | `package.json` |
| Client state | `zustand` 5 + `immer` | `package.json` |
| Lint/format | ESLint 9 + `@bob-park/eslint-config-bobpark`, Prettier + `@bob-park/prettier-config-bobpark` (GitHub Packages) | `package.json`, `eslint.config.mjs`, `.yarnrc.yml` |
| Tests | `cargo test` (+ `tempfile`), Node built-in `node:test` | — |

Notes:

- TypeScript stays on 6.x: `typescript-eslint` (pulled in by the bobpark ESLint config) does not support TS 7 yet.
- `@bob-park/*` come from GitHub Packages: `yarn install` needs `GITHUB_NPM_AUTH_TOKEN` ([configuration](./configuration.md#secrets)).
- `.yarnrc.yml` sets `npmMinimalAgeGate: 0` (no age gate) and `enableScripts: true` (dependency install scripts run).
- The vendored `drag` crate is a patched copy. Drop the patch only when upstream offers both changes.
