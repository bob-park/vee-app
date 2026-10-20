---
title: Architecture
scope: "**"
applies_to: deciding where code belongs, tracing a request, touching app lifecycle
related:
  - ./structure.md
  - ./conventions/rust-modules.md
  - ./conventions/tauri-commands.md
  - ./libs/react-query.md
---

# Architecture

> One process, two halves: a React SPA (one bundle, one screen per window label) and Rust. The frontend reaches Rust only through `call()` → `#[tauri::command]`. Front and back are split into **domains with the same name** on both sides (`clip`, `source_app`, `settings`, `whats_new`, `convert`), plus frontend-only `updater` and `panel` whose Rust half is the app shell.

## Request path

```
component
  → queries/<domain>.tsx        React Query hook (cache, loading, invalidation)
  → apis/<domain>.ts            typed function per command
  → shared/api  call<T>()       the only `invoke` caller; rejects with AppError
  ⇢ IPC
  → domain/<name>/commands.rs   #[tauri::command], thin: lock state, call domain API, emit events
  → domain/<name>/mod.rs        public domain API: validation + rules
  → domain/<name>/repository.rs SQL for this domain's tables
  → shared/db.rs                Connection (opened + migrated at startup)
```

The panel's clip list skips the `queries/` step: `PanelApp` calls `apis/clip.ts` directly for paging ([react-query](./libs/react-query.md#the-panel-clip-list)). Window commands (`hide_panel`, `resize_panel`, …) live in `app/window.rs`'s `commands` submodule, not in a domain.

Errors travel back as `AppError` → `{ code, message }` ([error-handling](./conventions/error-handling.md)).

## Rust layers

| Layer | Path | May import `tauri`? |
| ----- | ---- | ------------------- |
| Assembly | `lib.rs` | yes — plugins, setup, window events, `generate_handler!` only |
| App shell | `app/` (state, window + `window::commands`, tray, updater, watcher, shortcut, sound, front_app, disk_access, i18n, autostart, retention, convert + `convert::commands`, engines + `engines::commands`, drop_target) | yes |
| Domain adapters | `domain/<name>/commands.rs` | yes |
| Domain core | `domain/<name>/{mod,model,repository}.rs` | **no** |
| Infrastructure | `shared/` (db, error, png, migrations) | **no** |

Dependencies point one way: `commands → domain core → shared`. The app shell may call a domain's public API (e.g. `watcher` calls `clip::upsert`, `tray` reads `settings::get`).

## State

- `AppState` (`app/state.rs`):
  - `db: Mutex<Connection>` — one connection, locked per command.
  - `images_dir` — where image clips keep their PNGs (`<app data>/images`).
  - `prev_app_pid` — the app to re-activate after a copy.
  - `dragging`, `panel_pinned` — `AtomicBool`s read by the panel's focus handling.
  - a short watcher-suppression window after Vee writes the clipboard itself.
- `Convert` (`app/convert.rs`) — the current drag's convert session, a drag generation counter, the in-memory job list, and the worker queue. Managed by `convert::spawn` in setup.
- `Engines` (`app/engines.rs`) — FFmpeg install dir, status, and the running download's cancel flag and `curl` child. Managed by `engines::manage` in setup.
- `UpdateState` (`app/updater.rs`) — update status and the downloaded bundle, in memory.
- Each **window is a separate JS context**: React Query caches and Zustand stores are per window. Cross-window changes are broadcast as Tauri events ([react-query](./libs/react-query.md#multiple-windows)).

## Startup (`lib.rs` → `setup`)

1. Plugins: single-instance (must be first; a second launch opens Settings), log, dialog, global-shortcut, autostart, updater. `UpdateState` is managed before `setup`.
2. macOS: `ActivationPolicy::Accessory` (no Dock icon).
3. Open + migrate `vee.db` in the app data dir and create `images/`. On failure: show a native error dialog and exit. **Never wipe the history automatically.** Then `manage(AppState)` and `convert::spawn` (manages `Convert`, starts the convert worker thread).
4. Build the five windows. They are `create: false` in `tauri.conf.json`, so no webview can call a command before state exists.
5. `window::follow_active_space`, `whats_new::commands::show_if_due`, `disk_access::prompt_once` (macOS).
6. Tray, retention pruner thread (hourly), clipboard watcher threads, the OCR worker thread (fills `ocr_text` for image and files clips, newest first), the stored global shortcut, then the periodic updater thread.

## Windows

| Label | Screen | Opened by |
| ----- | ------ | --------- |
| `panel` | `src/app/panel/PanelApp.tsx` | global shortcut (`app/shortcut.rs` → `window::toggle_panel`), tray "Open" |
| `settings` | `src/app/settings/SettingsApp.tsx` | tray "Settings", `Cmd/Ctrl+,` in the panel (`open_settings`), second launch |
| `toast` | `src/app/toast/ToastApp.tsx` | after a copy, drop or convert (`toast://show`) |
| `convert` | `src/app/convert/ConvertApp.tsx` | convert key held during a card drag (`app/convert.rs`, `convert://open`) |
| `whats-new` | `src/app/whats-new/WhatsNewApp.tsx` | at startup when an installed update left notes |

- `main.tsx` picks the screen from the window label and sets `<html data-window>`, which `globals.css` uses to make the panel and toast backgrounds transparent.
- Closing `settings` **hides** it. Closing `whats-new` dismisses the notes. The `panel` hides when it loses focus unless it is pinned or a drag is in progress (`lib.rs` `on_window_event`).
- `show_settings` emits `settings://opened`, so the page resets to its first tab.

## Clipboard watcher

`app/watcher.rs` polls the clipboard (200 ms) and records a change only after 0.5 s of quiet, so a burst of changes stores the last one. It skips excluded source apps and Vee's own writes (`AppState::suppress_watcher`), stores the clip via `clip::upsert`, emits `clips://added`, and calls `ocr::wake()` so the OCR worker picks up a new image or files clip.

## Drag-to-convert

- `window::start_drag` calls `convert::begin` for image and files clips (before `drag::start_drag`, which blocks on Windows). If `dragConvert` is on, a thread polls the `convertKey` modifier (`drop_target::key_down`) while the drag lasts.
- On a key press it looks up the folder under the cursor (`drop_target::folder_at`: desktop, Finder window via Apple Events on macOS, Explorer on Windows) off the main thread, then shows the `convert` overlay at the press point on the main thread with `convert://open`. Releasing the key closes it (`convert://close`). Not over a folder, or Finder access not granted → a toast instead of the donut.
- `finish_drag` asks `convert::dropped_on_overlay` (by cursor geometry) whether the drop hit the overlay; if so it skips `confirm_copy`. The overlay's drop calls `convert_drop(format)`, which queues one job per file.
- One worker thread runs jobs (`domain::convert::run`: convert into a temp file, then move to a unique `name (1).ext` path). Job changes emit `convert://jobs` to the panel (card progress); a batch ends with one toast.
- Video sources (`domain::convert::is_video`: `mp4 m4v mov mkv webm avi wmv flv mpg mpeg mts m2ts 3gp`; no `ts`) offer MP4, WEBM, MOV, GIF, MP3 and MP4 720p. `domain/convert/video.rs` builds the FFmpeg args (quality from the `videoQuality` setting: `small` / `normal` / `original`), reads the duration with `ffprobe`, and parses `-progress pipe:1` into the card's percent. Without FFmpeg the donut shows those slices dimmed (`Engine::Ffmpeg`), and a drop on a dimmed slice is rejected (`take_drop`). A job fails with `JobError::Engine` only if FFmpeg is removed after the drag started.
- `cancel_convert(id)` sets a running video job's cancel flag: FFmpeg is killed, its temp output removed, and the job dropped without a toast. A cancel that lands as FFmpeg exits still wins (checked after the wait). FFmpeg is held in a kill-on-drop guard, so an error or a panic mid-run also stops it.
- Temp output goes to `<temp>/vee-convert-<identifier>`, cleared at startup. On `RunEvent::Exit`, `convert::shutdown` drops queued jobs, cancels the running one, waits up to 2 s for FFmpeg to be killed, and removes the temp dir.
- A drag generation counter keeps a late key press or job from reviving the overlay of an earlier drag.
- macOS: reading Finder's folder needs the Automation permission — `NSAppleEventsUsageDescription` in `Info.plist` and the `com.apple.security.automation.apple-events` entitlement ([configuration](./configuration.md)). Settings → Convert shows it (`get_finder_access`, `request_finder_access`, `open_automation_settings`).

## FFmpeg engine

- `app/engines.rs` pins FFmpeg 9.0.2 per platform (`ASSETS`: URL, SHA-256, size; macOS arm64 `ffmpeg.zip` + `ffprobe.zip` from martin-riedl.de, Windows the GyanD essentials zip). Nothing is downloaded until the user presses Download / Update in Settings → Convert.
- `download_ffmpeg` runs `curl` into `<temp>/vee-ffmpeg-download-<identifier>` (per identifier, so a dev build and the installed app don't collide), verifies the hash, unpacks, and swaps `ffmpeg` + `ffprobe` + `meta.json` into `<app data>/engines/ffmpeg/`. A failed update keeps the old install; `manage` restores an install a failed swap left aside, clears the temp dir, and removes the `.ffmpeg-unpack` / `.ffmpeg-new` work dirs (and a no-longer-needed `.ffmpeg-old`) a quit mid-install left. The `missing` status carries `downloadSize` (sum of the pinned asset sizes) for the Download hint.
- Every status change emits `engines://ffmpeg` (`FfmpegStatus`; progress every 250 ms). `cancel_ffmpeg_download` kills `curl`; `remove_ffmpeg` deletes the install; `get_ffmpeg_status` reads it. `engines::shutdown` stops a download on exit.

## Updater

- Release builds only: checks GitHub Releases `latest.json` at startup and every 6 h.
- Downloads in the background; installs only when the user picks "Restart to Update" (tray, Settings, or the panel's update button).
- Status changes emit `update://status` and rebuild the tray menu.
- The release notes are saved by `whats_new::save` and shown once by the `whats-new` window after the restart.
