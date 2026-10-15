---
title: Project Overview
scope: "**"
applies_to: onboarding, deciding whether a change fits the app
related:
  - ./tech-stack.md
  - ./architecture.md
  - ./structure.md
---

# Project Overview

> Vee is a clipboard-history app (Tauri 2, Rust + React). It records every copy (text, links, images, files) in SQLite and shows it as a card panel at the bottom of the screen on a global shortcut. It lives in the tray; there is no main window. Four windows: `panel`, `settings`, `toast`, `whats-new`.

## What it is

- A **tray-only app** (macOS: `ActivationPolicy::Accessory`, no Dock icon). Closing a window never quits; "Quit" is in the tray menu.
- **Windows:**
  - `panel`: the card panel. Transparent, borderless, always on top, on the monitor under the cursor. Search, `@app` tag, filters, pins, drag-out.
  - `settings`: four tabs (General, Panel, Shortcuts, History).
  - `toast`: the "Copied" notice. Never takes focus.
  - `whats-new`: release notes of the update just installed, shown once.
- **Rust side:** clipboard watcher, global shortcuts, the frontmost app (source app name and icon), copy sounds, retention pruning, auto-update, launch at login, macOS Full Disk Access prompt.
- All data stays on the machine. The network is used only for update checks.

## What it is not

- No HTTP backend. The "backend" is the Rust side of the same app, reached through Tauri commands.
- No sync, no cloud, no account.
- No CI build. Releases are built, signed and uploaded from personal machines ([release](./workflow/release.md)). CI only bumps the version ([versioning](./workflow/versioning.md)).

## Platforms

- macOS: Apple Silicon only, signed and notarized. Uses `macos-private-api` (transparent windows) and `objc2*` for the frontmost app.
- Windows: x64 (NSIS installer). No code signing; only the updater `.sig`. Uses the `windows` crate.
- Platform code sits behind `#[cfg(target_os = "macos")]` / `#[cfg(windows)]`. A change to one side must keep the other compiling and behaving the same.

## Names

`Vee` (productName) · `com.bobpark.vee` (identifier) · `vee-app` / `vee_app_lib` (crate) · `bob-park/vee-app` (repo).
