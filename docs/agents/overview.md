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

> Vee is a clipboard-history app (Tauri 2, Rust + React). It records every copy (text, links, images, files) in SQLite and shows it as a card panel docked to a screen edge (bottom by default; top, left or right in Settings) on a global shortcut. It lives in the tray; there is no main window. Five windows: `panel`, `settings`, `toast`, `convert`, `whats-new`.

## What it is

- A **tray-only app** (macOS: `ActivationPolicy::Accessory`, no Dock icon). Closing a window never quits; "Quit" is in the tray menu.
- **Windows:**
  - `panel`: the card panel. Transparent, borderless, always on top, on the monitor under the cursor. Search, `@app` tag, filters, pins, drag-out.
  - `settings`: five tabs (General, Panel, Shortcuts, Convert, History).
  - `toast`: the "Copied" notice (also convert results). Never takes focus.
  - `convert`: the format donut shown at the cursor while a card is dragged out with the convert key held (default Option on macOS, Ctrl on Windows). Dropping on a format converts the image(s) or video(s) into the folder under the cursor instead of copying. Never takes focus.
  - `whats-new`: release notes of the update just installed, shown once.
- **Rust side:** clipboard watcher, global shortcuts, the frontmost app (source app name and icon), copy sounds, retention pruning, drag-to-convert (folder under the cursor, image conversion via `sips` on macOS / the `image` crate, video conversion via a downloaded FFmpeg, HWP/HWPX via a downloaded rhwp, office and text documents via the user's own LibreOffice), auto-update, launch at login, macOS Full Disk Access prompt.
- All data stays on the machine. The network is used only for update checks and, when the user presses Download / Update in the Convert tab, to fetch the pinned FFmpeg (shaka-project's GPL static builds from GitHub) or rhwp (its GitHub release) into `<app data>/engines/<engine>/` (`app/engines.rs`). Neither is bundled, and LibreOffice is never downloaded.

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
