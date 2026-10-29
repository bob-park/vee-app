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

> One process, two halves: a React SPA (one bundle, one screen per window label) and Rust. The frontend reaches Rust only through `call()` → `#[tauri::command]`. Front and back are split into **domains with the same name** on both sides (`clip`, `source_app`, `settings`, `whats_new`, `convert`, `edit`), plus frontend-only `updater` and `panel` whose Rust half is the app shell.

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

The panel's clip list skips the `queries/` step: `PanelApp` calls `apis/clip.ts` directly for paging ([react-query](./libs/react-query.md#the-panel-clip-list)). Window commands (`hide_panel`, `resize_panel`, …) live in `app/window.rs`'s `commands` submodule, not in a domain. The editor's commands live in `app/editor.rs`'s `commands` submodule; `domain/edit` has no `commands.rs`.

Errors travel back as `AppError` → `{ code, message }` ([error-handling](./conventions/error-handling.md)).

## Rust layers

| Layer | Path | May import `tauri`? |
| ----- | ---- | ------------------- |
| Assembly | `lib.rs` | yes — plugins, setup, window events, `generate_handler!` only |
| App shell | `app/` (state, window + `window::commands`, tray, updater, watcher, shortcut, sound, front_app, disk_access, i18n, autostart, retention, convert + `convert::commands`, engines + `engines::commands`, editor + `editor::commands`, ai, drop_target) | yes |
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
- `EditorState` (`app/editor.rs`) — the image open in the editor (decoded into memory, so deleting its clip or file doesn't break it) and that session's cache of AI results. Managed before `setup`.
- `Engines` (`app/engines.rs`) — one slot per downloaded engine (FFmpeg, rhwp, bgremove): install dir, status, and the running download's cancel flag and `curl` child. Managed by `engines::manage` in setup.
- `UpdateState` (`app/updater.rs`) — update status and the downloaded bundle, in memory.
- Each **window is a separate JS context**: React Query caches and Zustand stores are per window. Cross-window changes are broadcast as Tauri events ([react-query](./libs/react-query.md#multiple-windows)).

## Startup (`lib.rs` → `setup`)

1. Plugins: single-instance (must be first; a second launch opens Settings), log, dialog, global-shortcut, autostart, updater. `UpdateState` is managed before `setup`.
2. macOS: `ActivationPolicy::Accessory` (no Dock icon).
3. Open + migrate `vee.db` in the app data dir and create `images/`. On failure: show a native error dialog and exit. **Never wipe the history automatically.** Then `manage(AppState)` and `convert::spawn` (manages `Convert`, starts the convert worker thread).
4. Build the six windows. They are `create: false` in `tauri.conf.json`, so no webview can call a command before state exists.
5. `window::follow_active_space`, `whats_new::commands::show_if_due`, `disk_access::prompt_once` (macOS).
6. Tray, retention pruner thread (hourly), clipboard watcher threads, the OCR worker thread (fills `ocr_text` for image and files clips, newest first), the stored global shortcut, then the periodic updater thread.

## Windows

| Label | Screen | Opened by |
| ----- | ------ | --------- |
| `panel` | `src/app/panel/PanelApp.tsx` | global shortcut (`app/shortcut.rs` → `window::toggle_panel`), tray "Open" |
| `settings` | `src/app/settings/SettingsApp.tsx` | tray "Settings", `Cmd/Ctrl+,` in the panel (`open_settings`), second launch |
| `toast` | `src/app/toast/ToastApp.tsx` | after a copy, drop or convert (`toast://show`) |
| `convert` | `src/app/convert/ConvertApp.tsx` | convert key held during a card drag (`app/convert.rs`, `convert://open`) |
| `editor` | `src/app/editor/EditorApp.tsx` | a single image dropped on the donut's edit slice (`app/convert.rs` → `editor::open`, `editor://opened`) |
| `whats-new` | `src/app/whats-new/WhatsNewApp.tsx` | at startup when an installed update left notes |

- `main.tsx` picks the screen from the window label and sets `<html data-window>`, which `globals.css` uses to make the panel and toast backgrounds transparent.
- Closing `settings` **hides** it. Closing `editor` hides it and drops its image (`editor::close`). Closing `whats-new` dismisses the notes. The `panel` hides when it loses focus unless it is pinned or a drag is in progress (`lib.rs` `on_window_event`).
- `show_settings(tab)` emits `settings://opened` with a `SettingsTab` name as the payload, so the page starts over on that tab (an unknown name → `general`). Tray, second launch and `open_settings()` without a tab use `general`; the editor's "needs model" prompt calls `open_settings('convert')`.

## Clipboard watcher

`app/watcher.rs` polls the clipboard (200 ms) and records a change only after 0.5 s of quiet, so a burst of changes stores the last one. It skips excluded source apps and Vee's own writes (`AppState::suppress_watcher`), stores the clip via `clip::upsert`, emits `clips://added`, and calls `ocr::wake()` so the OCR worker picks up a new image or files clip.

## Drag-to-convert

- `window::start_drag` calls `convert::begin` for image and files clips (before `drag::start_drag`, which blocks on Windows). If `dragConvert` is on, a thread polls the `convertKey` modifier (`drop_target::key_down`) while the drag lasts.
- On a key press it looks up the folder under the cursor (`drop_target::folder_at`: desktop, Finder window via Apple Events on macOS, Explorer on Windows) off the main thread, then shows the `convert` overlay at the press point on the main thread with `convert://open`. Releasing the key closes it (`convert://close`). Not over a folder, or Finder access not granted → a toast instead of the donut.
- `finish_drag` asks `convert::dropped_on_overlay` (by cursor geometry) whether the drop hit the overlay; if so it skips `confirm_copy`. The overlay's drop calls `convert_drop(format)`, which queues one job per file.
- One worker thread runs jobs (`domain::convert::run`: convert into a temp file, then move to a unique `name (1).ext` path). Job changes emit `convert://jobs` to the panel (card progress); a batch ends with one toast.
- Video sources (`domain::convert::is_video`: `mp4 m4v mov mkv webm avi wmv flv mpg mpeg mts m2ts 3gp`; no `ts`) offer MP4, WEBM, MOV, GIF, MP3 and MP4 720p. `domain/convert/video.rs` builds the FFmpeg args (quality from the `videoQuality` setting: `small` / `normal` / `original`), reads the duration with `ffprobe`, and parses `-progress pipe:1` into the card's percent. Without FFmpeg the donut shows those slices dimmed (`Engine::Ffmpeg`), and a drop on a dimmed slice is rejected (`take_drop`). A job fails with `JobError::Engine` only if FFmpeg is removed after the drag started.
- Documents (`domain/convert/mod.rs` family table; the source's own format is left out, and a card mixing families is unsupported):
  - HWP `hwp hwpx` → PDF, HWPX (only from `.hwp`), HWP (only from `.hwpx`); needs rhwp (`Engine::Rhwp`).
  - Text documents `docx doc odt rtf txt html htm` → PDF, DOCX, ODT, TXT, RTF, HTML, all LibreOffice (`Engine::LibreOffice`), on both platforms.
  - Spreadsheets `xlsx xls ods` → PDF, XLSX, ODS; presentations `pptx ppt odp` → PDF, PPTX, ODP (both LibreOffice).
- Runners: `domain/convert/hwp.rs` runs `rhwp export-pdf <in> -o <out>` / `export-hwpx <in> <out>` / `convert <in> <out>` (2 min timeout). `domain/convert/office.rs` runs `soffice --headless --norestore -env:UserInstallation=file:///<app data>/lo-profile --convert-to <filter> --outdir <fresh dir> <in>` (5 min) and moves `<outdir>/<src stem>.<ext>` out by that exact name (LibreOffice exits 0 even when it couldn't load the file); its own `lo-profile` lets it run while the user's LibreOffice is open. No progress (spinning ring) and no cancel for these.
- `domain/convert/process.rs`: `command()` (no console window on Windows) and `exec()` (wait with a deadline; kill on timeout → `JobError::Engine`, on cancel → `JobError::Cancelled`; exit `0xC0000135` = missing MSVC runtime → `JobError::Runtime`). The child sits in a kill-on-drop guard; on Windows it kills the whole tree (`taskkill /T /F`), since `soffice.exe` launches `soffice.bin`.
- LibreOffice is never downloaded (`app/office.rs`): `soffice()` checks `/Applications/LibreOffice.app/Contents/MacOS/soffice` on macOS, `%ProgramFiles%\LibreOffice\program\soffice.exe` then `%ProgramFiles(x86)%\…` on Windows. `get_libreoffice` (async) returns `{ version }` — from `soffice --version` on macOS (10 s), from `MsiProductVersion` in `program\version.ini` on Windows (`soffice --version` opens a console there; LibreOffice bug 103255); `null` version if that fails — or `null` when not found; `open_libreoffice_download` opens the fixed download page. Settings re-checks when the Convert tab mounts.
- `cancel_convert(id)` sets a running video job's cancel flag: FFmpeg is killed, its temp output removed, and the job dropped without a toast. A cancel that lands as FFmpeg exits still wins (checked after the wait). FFmpeg is held in a kill-on-drop guard, so an error or a panic mid-run also stops it.
- Temp output goes to `<temp>/vee-convert-<identifier>`, cleared at startup. On `RunEvent::Exit`, `convert::shutdown` drops queued jobs, cancels the running one, waits up to 2 s for the running converter to be killed, and removes the temp dir.
- A single image the `image` crate can read on this platform gets an `Edit` slice first (`domain::convert::formats_for`). Over a folder the donut shows every slice; elsewhere it shows only `Edit` (an edit-only donut, `donut_slices`) instead of the "not a folder" toast. A drop on `Edit` needs no folder: `drop_on` opens the image in the editor on a new thread instead of queueing a job ([Image editor](#image-editor)).
- A drag generation counter keeps a late key press or job from reviving the overlay of an earlier drag.
- macOS: reading Finder's folder needs the Automation permission — `NSAppleEventsUsageDescription` in `Info.plist` and the `com.apple.security.automation.apple-events` entitlement ([configuration](./configuration.md)). Settings → Convert shows it (`get_finder_access`, `request_finder_access`, `open_automation_settings`).

## Engines

- `app/engines.rs` keeps one table (`SPECS`, indexed by `EngineId`: `ffmpeg`, `rhwp`, `bgremove`) of install dir name and binaries. `resolve(id)` returns a `Release` (version + `Asset`s: URL, SHA-256, size, and kind — an `Archive` unpacked for its binaries, or a `Raw` file installed as is, like a model). FFmpeg and rhwp are not pinned: `resolve` looks up their latest release with `curl`. bgremove is pinned (below). A release is accepted only with a digits-and-dots version, URLs under the allowed release prefixes (`SOURCES`: the rhwp, codexffmpeg and rembg GitHub `…/releases/download/` paths, and `https://ffmpeg.martin-riedl.de/download/`), and full SHA-256s; every `curl` is https-only (`--proto =https --proto-redir =https`). None is bundled.
  - FFmpeg, macOS arm64: martin-riedl.de's static builds. `…/redirect/latest/macos/arm64/release/{ffmpeg,ffprobe}.zip` redirects (GET; HEAD answers 404) to `…/download/macos/arm64/<id>_<ver>/<name>.zip`; checksum from `<url>.sha256`, size from a HEAD's `content-length`. Both zips must name the same version.
  - FFmpeg, Windows x64: gyan.dev's essentials build via its GitHub mirror `GyanD/codexffmpeg`: the newest release whose tag is digits and dots (daily `…-git-…` builds skipped), asset `ffmpeg-<ver>-essentials_build.zip`; checksum from the asset's `digest`.
  - bgremove (a model engine): **pinned**, not looked up. `resolve` returns `pinned(id)`: BiRefNet general (swin-v1-tiny) from `danielgatis/rembg`'s release, saved as `model.onnx` (a raw asset, not unpacked). URL, SHA-256 and size are constants; the version is `MODEL_VERSION` (bump it when a pinned file changes; `2` dropped the runtime, so a version-1 install that still holds the library shows the update badge and the update replaces the dir). Installed file: `model.onnx`. ONNX Runtime is **not** an engine: it ships inside the app (see [Image editor](#image-editor)).
  - rhwp: `edwardkim/rhwp`'s latest GitHub release, `rhwp-v<ver>-macos-aarch64.tar.gz` / `rhwp-v<ver>-windows-x86_64.zip`; checksum from `digest`. Windows rhwp needs the MSVC runtime (`JobError::Runtime` toast if missing).
- `check_engine_updates` (async) resolves both engines when the Convert tab mounts, keeps each `Release` in its slot (a failed lookup keeps the previous one, silently) and re-emits the status unless that engine is downloading. `on_disk` derives `update` (latest is a newer dotted version than the install; never a downgrade) and `downloadSize` (`null` until a lookup succeeds) from it.
- `download_engine(engine)` uses the slot's `Release` (resolving it first if none, shown as "Finding the latest version…"; a failed lookup ends as the `network` error and keeps any stored release) and runs `curl` into `<temp>/vee-<engine>-download-<identifier>` (per identifier, so a dev build and the installed app don't collide), verifies every hash, puts each binary in place (an archive unpacked with the OS `tar -xf` — bsdtar on macOS, `tar.exe` on Windows 10 1803+, which reads zip — and searched by file name, symlinks never followed — an archive may hold only some binaries, the install needs all; `chmod 755` on unix), and swaps the binaries + `meta.json` into `<app data>/engines/<engine>/`. One download at a time per engine; the two engines download side by side. A failed update keeps the old install; `manage` restores an install a failed swap left aside, clears the temp dir, and removes the `.<engine>-new` work dir (and a no-longer-needed `.<engine>-old`) a quit mid-install left. Removing one engine never touches the other's dir. The `missing` status carries `downloadSize` (the release's total, or `null`) for the Download hint; `downloading` has a `stage` (`download` / `verify` / `unpack`).
- Hardware H.264 (`videoHwAccel` setting, default `on`): MP4, MOV and 720p swap libx264 for a GPU encoder; WEBM / GIF / MP3 never do. `domain::convert::hw_encoder` picks VideoToolbox on macOS (`-hwaccel videotoolbox`, `-q:v` 47 / 60 / 70, sized to match libx264 crf 28 / 23 / 18). Elsewhere it probes `h264_nvenc`, `h264_qsv`, `h264_amf` in that order with a one-frame `lavfi` encode, once per app run (`OnceLock`), and uses `-hwaccel auto`; no working encoder means software. `ffmpeg_args(…, hw)` stays pure. If a hardware encode exits non-zero (not a cancel), `video::convert` deletes its temp output, logs a warning and reruns that file once in software; progress restarts at 0.
- Every status change emits `engines://status` with `{ engine, status }` (`EngineStatus`; progress every 250 ms). `cancel_engine_download(engine)` kills `curl`; `remove_engine(engine)` deletes the install; `get_engine_status(engine)` reads it (`engine` = `"ffmpeg" | "rhwp" | "bgremove"`). `remove_engine` first calls `ai::forget`, which refuses while a background removal is running. `engines::shutdown` stops downloads on exit.
- Settings → Convert has an FFmpeg, an rhwp and a background removal model section (same component: missing / downloading / installed) and a LibreOffice row (found → a "Found" badge, with the version in the hint when one is found; not found → a gray "Not found" badge and a button opening the download page).

## Image editor

- `editor::open(source)` decodes the file into a new `Session` (replacing any open one), emits `editor://opened` to the `editor` window and shows it. The page reloads its source on that event (and once on mount). A second drop while the editor is open replaces the image.
- The page keeps the edit as a list of `Op`s (`domain/edit/model.rs`: `crop`, `rotate { quarter: ±1 }`, `resize`, `removeBg`, `upscale { scale: 2 | 4 }`) and sends the whole list each time: `render_edit(ops)` renders off the main thread and returns a PNG data URL fitted to `PREVIEW_SIDE` (2048) plus the full-resolution size; `get_edit_source` returns the original the same way. Undo drops the last op.
- `domain::edit::apply_geometric` does crop / rotate / resize (no `tauri`). AI ops go through `app/ai.rs` and are cached per session by the ops before them, so undo or a tab switch doesn't rerun the model.
- `copy_edit(ops)` renders at full resolution, saves a new image clip (`clip::upsert`), writes it to the clipboard, then `confirm_copy` (sound, `clips://changed`, toast) and closes the editor unless another image opened meanwhile. The original file and clip are never touched. If the clipboard write fails the clip stays in history and a failed toast shows.
- `app/ai.rs`:
  - Background removal: BiRefNet (`model.onnx` from `engines::model_dir(Bgremove)`) through `ort` with ONNX Runtime 1.31.0 loaded dynamically from the app bundle (`ai::runtime_path`: macOS `Contents/Frameworks/libonnxruntime.dylib`, Windows `onnxruntime.dll` beside the exe; a debug build falls back to `src-tauri/vendor/onnxruntime/`) by `ort::init_from` (the environment lives until exit). The library is bundled, not downloaded, because the hardened runtime only loads libraries signed by Vee's own team. A missing runtime is `Other("onnx runtime missing from the app")`. Input 1024×1024, ImageNet mean/std; the sigmoid mask is resized back and becomes the alpha channel. CPU only, at most 4 threads, no arena. One global lock: runs are serial, and the session is loaded per run and dropped after it (BiRefNet holds GBs while loaded). Without the engine it fails with `Other("model missing")`, and the editor's tab shows a pointer to Settings › Convert instead.
  - Upscale: no model (Real-ESRGAN rewrote Korean glyphs). `domain::edit::upscale_fallback` does Lanczos3 to ×2 / ×4, capped so the long side is at most `UPSCALE_CAP` (4096), then a light unsharp mask.

## Updater

- Release builds only: checks GitHub Releases `latest.json` at startup and every 6 h.
- Downloads in the background; installs only when the user picks "Restart to Update" (tray, Settings, or the panel's update button).
- Status changes emit `update://status` and rebuild the tray menu.
- The release notes are saved by `whats_new::save` and shown once by the `whats-new` window after the restart.
