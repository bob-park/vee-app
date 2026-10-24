---
title: Configuration & Secrets
scope: src-tauri/tauri.conf.json, src-tauri/capabilities/**, .yarnrc.yml, scripts/**
applies_to: changing app config, windows, permissions, or anything touching keys and tokens
related:
  - ./build-run.md
  - ./workflow/release.md
  - ./conventions/tauri-commands.md
---

# Configuration & Secrets

> App config is `src-tauri/tauri.conf.json`. Webview permissions are in `src-tauri/capabilities/default.json`. Secrets (signing keys, Apple credentials, npm token) live **outside the repo**: `~/.config/vee/sign.env` and the `GITHUB_NPM_AUTH_TOKEN` env var. **Never commit a secret.**

## `tauri.conf.json`

| Field | Notes |
| ----- | ----- |
| `version` | source of truth for the app version — changed only by `bump-version-up.sh` ([versioning](./workflow/versioning.md)) |
| `productName`, `identifier` | `Vee` / `com.bobpark.vee`; release scripts depend on `productName` for file names, and the identifier names the user's data folder — never change it |
| `app.macOSPrivateApi` | `true`, for the transparent panel/toast windows |
| `app.windows` | `panel`, `toast`, `convert`, `settings`, `whats-new`, all `create: false` (built in `setup` after state exists), all `visible: false` except `convert`: it is built visible at `-10000, -10000` and hidden right after, because wry only takes over drops on the WebView2 child windows that exist when the webview is built (Windows) |
| `bundle.targets` | `app`, `dmg`, `nsis` |
| `bundle.macOS.entitlements` | `Entitlements.plist` — `com.apple.security.automation.apple-events`, so the signed (hardened runtime) app may ask Finder for its folder |
| `bundle.createUpdaterArtifacts` | `true` — full builds need the signing key |
| `plugins.updater.pubkey` | public half of the updater key ([release](../release.md)) |
| `plugins.updater.endpoints` | `…/releases/latest/download/latest.json` of `bob-park/vee-app` |
| `plugins.updater.windows.installMode` | `passive` |

## macOS plists

- `src-tauri/Info.plist` is merged into the bundle's `Info.plist` by Tauri. It holds `NSAppleEventsUsageDescription`, the text macOS shows when Vee first asks to control Finder (drag-to-convert).
- `src-tauri/Entitlements.plist` is referenced from `bundle.macOS.entitlements`. Without the Apple Events entitlement a notarized build can't send Apple Events even after the user allows it.

## Network and temp paths

- Network use besides the updater (`app/engines.rs`): opening Settings → Convert looks up FFmpeg's and rhwp's latest releases (GitHub API for rhwp and gyan.dev's FFmpeg mirror, martin-riedl.de for macOS FFmpeg), and the downloads the user starts fetch them, checked against the SHA-256 the source publishes (not bundled). Both run the system `curl` (and `tar` to unpack), so no HTTP crate or capability is needed. LibreOffice is never downloaded; Settings only opens its download page.
- Temp dirs: `<temp>/vee-<engine>-download-<identifier>` (`ffmpeg` / `rhwp` download) and `<temp>/vee-convert-<identifier>` (conversion output before the move, cleared at startup and exit).
- App data: `<app data>/engines/{ffmpeg,rhwp}/` (installed engines) and `<app data>/lo-profile/` (LibreOffice's own profile for headless runs, so a LibreOffice the user has open doesn't block it). Run a dev build with its own identifier so it doesn't share these with the installed app ([build-run](./build-run.md)).

## Capabilities

- `capabilities/default.json` lists the windows (`panel`, `toast`, `convert`, `settings`, `whats-new`) and `core:default`, `dialog:default`.
- A plugin API **called from JS** needs its permission here. Rust-only plugin use (global-shortcut, autostart, updater) does not.
- A new window label must be added here.

## Secrets

| Secret | Where | Used by |
| ------ | ----- | ------- |
| Apple signing identity, Apple ID, app password, team ID | `~/.config/vee/sign.env` | `scripts/release.sh` (notarization) |
| `TAURI_SIGNING_PRIVATE_KEY` (+ password) | `sign.env` on each release machine (`%USERPROFILE%\.config\vee\sign.env` on Windows) | `yarn tauri build`, both release scripts |
| `GITHUB_NPM_AUTH_TOKEN` | shell env (GitHub PAT with `read:packages`) | `yarn install` (`@bob-park/*` from GitHub Packages, via `.yarnrc.yml`) |
| `GITHUB_TOKEN` | Actions-provided | `bump-version.yml` |

- Don't print secrets in logs, scripts, or commit messages.
- The repo is public, but `@bob-park/*` packages still need a token: an outside contributor needs their own PAT with `read:packages` to run `yarn install`.
- If a key file path is wrong, the build fails late with `Invalid symbol 46` ([release](../release.md#문제-해결)).
