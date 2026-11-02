# Screen Capture & Recording Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Global shortcuts open a Vee-drawn selection overlay (drag = region, Space = whole window) that either captures a PNG straight to the clipboard or records the screen with system audio to a file that is saved and put on the clipboard.

**Architecture:** One new transparent full-monitor window `capture` (selection + pre-record pill) and one small window `recorder` (pill while recording), both in the existing single React bundle. Rust app-shell modules `app/capture/` (window list, grab, capture flow) and `app/recorder/` (state machine, per-OS engines, finalize). macOS records with ScreenCaptureKit `SCRecordingOutput` (15+); Windows records video with the downloaded FFmpeg and system audio with WASAPI loopback, then muxes. Conversion to WEBM/GIF reuses `domain/convert/video.rs`; clip + clipboard reuse `editor::copy_to_history` and `window::write_clipboard`; the done toast reuses `ConvertNote::Saved`.

**Tech Stack:** Tauri 2, Rust (objc2 / ScreenCaptureKit on macOS, `windows` crate on Windows), React 19 + Tailwind 4 + daisyUI 5, FFmpeg engine.

**Spec:** `docs/superpowers/specs/2026-11-03-screen-capture-recording-design.md`

## Global Constraints

- Branch `feature/screen-capture-recording`; commit prefixes per `docs/agents/workflow/commit-messages.md`.
- Recording: macOS 15+ and Windows 10 2004 (build 19041)+. Capture: every OS Vee supports.
- Default shortcuts: capture `CommandOrControl+Alt+Shift+Digit4`, record `CommandOrControl+Alt+Shift+Digit5` (the recorder's `e.code` format).
- Settings keys: `captureShortcut`, `recordShortcut`, `recordFolder` (`""` = OS Downloads), `recordFormat` (`mp4` default, `mov` · `webm` · `gif`).
- GIF: max 30 s, auto-stop at 30 s, no audio. Audio: system audio only, never microphone.
- Sounds: capture = macOS system `Screen Capture.aif` (played from `/System/Library/Components/CoreAudio.component/Contents/SharedSupport/SystemSounds/system/Screen Capture.aif`, never bundled) / Windows bundled `shutter.wav` (Holga, public domain). Record start/stop = Two-Tone (start 660→990 Hz, stop 990→660 Hz; sine; 0.14 s + 0.2 s, second tone at 0.13 s). All follow the `sound` on/off setting.
- Vee's own sounds must not be recorded: start recording only after the start sound ends, play the stop sound after recording stops, and suppress copy sounds while recording.
- Overlay / pill must never appear in a capture or recording.
- Every user-visible string in `src/shared/i18n/{en,ko}.ts`; colors only in `src/app/globals.css` (new `--vee-rec` token, light + dark). Aurora Glass (`docs/design/aurora-glass.md`), pill = mockup A (`.superpowers/brainstorm/67055-1793633854/content/record-hud.html`).
- **No new dependency or crate feature before the user approves the list in Task 0.**
- Memory: after a recording ends, `footprint` of Vee must be ≤ +50 MB vs before (user's bar). If in-process ScreenCaptureKit misses it, move recording into a child worker (`vee-app --record-worker`, like `--ai-worker`).
- Done = `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` pass, and the dev app checked in light/dark × ko/en (`docs/agents/ui-testing.md`).

## Review Focus

1. Shortcut pressed again while the overlay is open, or while a recording is finishing → no second overlay/recording; record shortcut while recording = stop. (Task 7 state-machine test.)
2. Save folder deleted or not writable → saves to Downloads and the toast says so. (Task 7 `resolve_folder` test.)
3. Same file name already in the folder → ` (2)`, ` (3)`… never overwrite. (Task 7 `unique_path` test.)
4. Retina / scaled / secondary monitor → selected rect maps to the right physical pixels. (Task 3 `to_global` tests with scale 2 and a non-zero monitor origin.)
5. A click without a drag (or a < 4 px drag) in region mode → nothing captured, overlay stays. (Task 3 `normalizeRect` test.)

---

### Task 0: Dependency approval (no code)

- [ ] **Step 1:** Ask the user to approve exactly this list, then add it to `src-tauri/Cargo.toml`:
  - macOS: `objc2-screen-capture-kit = { version = "0.3.2", default-features = false, features = ["std", "block2", "SCStream", "SCRecordingOutput", "SCShareableContent", "SCStreamConfiguration", "SCContentFilter"] }`, `block2 = "0.6.2"` (already in `Cargo.lock` transitively); extra features on existing crates: `objc2-foundation` `NSProcessInfo` / `NSURL` / `NSError`, `objc2-app-kit` `NSSound` (if not already), `objc2-core-graphics` whatever feature exposes `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` (verify with docs.rs).
  - Windows (`windows` crate features): `Win32_Graphics_Gdi`, `Win32_Graphics_Dwm`, `Win32_System_Registry`.
- [ ] **Step 2:** `cargo check --manifest-path src-tauri/Cargo.toml` passes. Commit `chore: add screen capture dependencies`.

### Task 1: Settings keys and multi-key global shortcuts

**Files:**
- Modify: `src-tauri/src/domain/settings/model.rs`, `src-tauri/src/domain/settings/mod.rs`, `src-tauri/src/domain/settings/commands.rs`, `src-tauri/src/app/shortcut.rs`, `src-tauri/src/lib.rs` (handler list)
- Modify: `src/domain/settings/apis/settings.ts`, `src/domain/settings/apis/settings.dto.ts`, `src/app/settings/SettingsApp.tsx` (only the `setShortcut` call site)

**Interfaces:**
- Produces (Rust): `DEFAULT_CAPTURE_SHORTCUT`, `DEFAULT_RECORD_SHORTCUT`, `RECORD_FORMATS: [&str; 4] = ["mp4","mov","webm","gif"]`; `settings::capture_sounds(conn) -> bool` (`sound == "on"`); `enum shortcut::Global { Panel, Capture, Record }` with `fn key(self) -> &'static str` (`"shortcut"`, `"captureShortcut"`, `"recordShortcut"`); `shortcut::register(app, Global, accel) -> AppResult<()>`; `shortcut::register_stored(app)` (replaces `register_stored_shortcut`, records failures); `shortcut::failures() -> Vec<&'static str>` (setting keys that failed to register).
- Produces (commands): `set_shortcut(key: String, accel: String)`, `get_shortcut_failures() -> Vec<String>`, `open_keyboard_settings()` (macOS: `open x-apple.systempreferences:com.apple.Keyboard-Settings.extension`; Windows: no-op `Ok(())`).
- Produces (TS): `Settings.captureShortcut/recordShortcut/recordFolder/recordFormat`, `type RecordFormat = 'mp4'|'mov'|'webm'|'gif'`, `RECORD_FORMATS`, `setShortcut(key: GlobalShortcutKey, accel)`, `getShortcutFailures()`, `openKeyboardSettings()`; `SettingKey` gains `recordFolder`, `recordFormat`.
- Shortcut actions wired here as stubs: `Capture` → `crate::app::capture::open(app, Mode::Capture)`, `Record` → `crate::app::recorder::on_shortcut(app)` (both added in Tasks 5/7; until then call `log::info!`).

- [ ] **Step 1: Failing tests** in `domain/settings/mod.rs`:
  - `capture_and_record_defaults`: `get(&c,"captureShortcut") == "CommandOrControl+Alt+Shift+Digit4"`, `get(&c,"recordShortcut") == "CommandOrControl+Alt+Shift+Digit5"`, `get(&c,"recordFormat") == "mp4"`, `get(&c,"recordFolder") == ""`.
  - `record_format_accepts_only_known`: `set(&c,"recordFormat","gif")` ok; `"mkv"` → `AppError::InvalidInput`, stored value unchanged.
  - `record_folder_must_be_an_existing_absolute_dir`: a `tempfile::tempdir()` path ok; `"relative/dir"`, a missing path and `""`-via-set rejected (reset goes through `set_raw`).
  - `capture_sounds_follow_the_sound_switch`: default true; after `set("sound","off")` false; `soundName = "none"` alone keeps it true.
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --lib settings` → the four fail.
- [ ] **Step 3:** Implement the defaults, `validate` arms (`recordFormat` ∈ `RECORD_FORMATS`; `recordFolder` = `Path::is_absolute && is_dir`), `capture_sounds`, DTO fields, `Global` + `register`/`register_stored`/`failures` (a `static FAILED: Mutex<Vec<&'static str>>`; the default fallback stays panel-only as today), and `set_shortcut(key, accel)` generalizing today's swap-and-restore for any `Global` key; unknown `key` → `InvalidInput`. Keep `set_raw` for writes.
- [ ] **Step 4:** Tests pass; `yarn typecheck` passes after the TS changes.
- [ ] **Step 5:** Commit `feat: capture and record settings keys and shortcuts`.

### Task 2: Capture and record sounds

**Files:**
- Modify: `scripts/gen-sounds.mjs` (add `record-start`, `record-stop`), `src-tauri/src/app/sound.rs`
- Create: `src-tauri/sounds/record-start.wav`, `src-tauri/sounds/record-stop.wav` (generated), `src-tauri/sounds/shutter.wav` (from Wikimedia Commons `File:Holga_shuttersound.ogg`: `ffmpeg -ss 0.28 -t 0.62 -i Holga_shuttersound.ogg -af "afade=t=out:st=0.5:d=0.12,loudnorm=I=-16:TP=-1" -ar 44100 -ac 1 shutter.wav`; note the public-domain source in a comment in `sound.rs`)

**Interfaces:**
- Produces: `sound::play_shutter()`, `sound::play_record(start: bool)` (both main thread on macOS, ignore the mute), `sound::set_recording(on: bool)` (an `AtomicBool`; while on, `sound::play` — the copy sound — returns early). macOS `play_shutter` loads the system file with `NSSound::initWithContentsOfFile_byReference` and does nothing if it is missing; Windows plays `shutter.wav` from memory like `play`.
- Callers check `settings::capture_sounds` before calling.

- [ ] **Step 1: Failing tests** in `sound.rs`: `record_sounds_are_wavs` (`wav("record-start")`, `wav("record-stop")`, `wav("shutter")` start with `RIFF`/`WAVE`); `copy_sound_is_muted_while_recording` (`set_recording(true)` → `copy_muted() == true`; `false` → `false`; expose `fn copy_muted() -> bool` for the test).
- [ ] **Step 2:** Run → fail.
- [ ] **Step 3:** Add the two Two-Tone entries to `SOUNDS` in `gen-sounds.mjs` (start: `tone(f0=660,f1=660,dur=0.14,vol=0.22)` + `tone(f0=990,f1=990,start=0.13,dur=0.2,vol=0.22)`; stop: the same with 990/660; `len: 0.36`), run `node scripts/gen-sounds.mjs`, add the WAV, implement the functions. `wav()` gains the three names but `SOUND_NAMES` (copy-sound picker) does not.
- [ ] **Step 4:** Tests pass (the existing `every_sound_is_a_wav` too).
- [ ] **Step 5:** Commit `feat: shutter and record sounds`.

### Task 3: Geometry, window list and selection helpers

**Files:**
- Create: `src-tauri/src/app/capture/mod.rs` (module root; only `pub mod geometry; pub mod window_list;` for now), `src-tauri/src/app/capture/geometry.rs`, `src-tauri/src/app/capture/window_list.rs`
- Create: `src/utils/selection.ts`, `src/utils/selection.test.ts`
- Modify: `src-tauri/src/app/mod.rs`

**Interfaces:**
- Produces (Rust, serde camelCase): `struct MonitorInfo { x: f64, y: f64, width: f64, height: f64, scale: f64 }` (global logical coords: points on macOS, DIPs on Windows); `struct WinInfo { id: u64, app: String, x: f64, y: f64, width: f64, height: f64 }` (monitor-local logical coords, front-to-back order); `enum Selection { Rect { x, y, width, height }, Window { id: u64 } }` (`#[serde(tag = "kind")]`, rect in monitor-local logical px).
- `geometry::to_global(sel_rect, &MonitorInfo) -> (f64, f64, f64, f64)` (global logical) and `geometry::to_physical(sel_rect, &MonitorInfo) -> (i32, i32, u32, u32)` (rounded, width/height ≥ 1, even sides rounded down for video use is done in recorder, not here).
- `window_list::list(monitor: &MonitorInfo) -> Vec<WinInfo>`: macOS `CGWindowListCopyWindowInfo(OnScreenOnly | ExcludeDesktopElements)`, layer 0 only; Windows `EnumWindows` + `IsWindowVisible` + not `DWMWA_CLOAKED` + `DWMWA_EXTENDED_FRAME_BOUNDS`, no tool windows. Both drop Vee's own pid. Decision kept pure: `window_list::keep(raw: Vec<RawWin>, own_pid: u32, monitor: &MonitorInfo) -> Vec<WinInfo>` (drops own pid, zero-size, fully off-monitor; clips to the monitor; converts to monitor-local).
- Produces (TS): `normalizeRect(a: Point, b: Point): Rect | null` (null when either side < 4); `windowAt(windows: WinInfo[], p: Point): WinInfo | null` (first hit in order).

- [ ] **Step 1: Failing tests**:
  - Rust `to_physical_scales_and_offsets`: monitor `{x:1440,y:0,width:1920,height:1080,scale:2}` + rect `{x:10,y:20,width:100,height:50}` → `to_global` = `(1450,20,100,50)`, `to_physical` = `(2900,40,200,100)`.
  - Rust `keep_drops_own_and_offscreen_windows`: own pid dropped, a window entirely on another monitor dropped, a window half off the monitor clipped to it, order preserved.
  - TS `normalizeRect`: `({x:50,y:50},{x:10,y:20})` → `{x:10,y:20,width:40,height:30}`; `({x:0,y:0},{x:3,y:100})` → `null`.
  - TS `windowAt`: overlapping windows → the earlier (front) one; point outside all → `null`.
- [ ] **Step 2:** Run `cargo test … --lib capture` and `yarn test` → fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Pass.
- [ ] **Step 5:** Commit `feat: capture geometry and window list`.

### Task 4: Screen recording permission and OS gates

**Files:**
- Create: `src-tauri/src/app/screen_access.rs`
- Modify: `src-tauri/src/app/mod.rs`, `src-tauri/src/lib.rs`, `src/domain/settings/apis/settings.ts` (wrappers), `src/domain/settings/queries/settings.tsx` (`useScreenAccess`, re-checked on window focus like `useDiskAccess`), `src/app/settings/_components/PermissionsSection.tsx`, `src/shared/i18n/{en,ko}.ts`

**Interfaces:**
- Produces: `screen_access::granted() -> bool` (macOS `CGPreflightScreenCaptureAccess`; Windows `true`); `screen_access::recording_supported() -> bool` (macOS `NSProcessInfo` major ≥ 15; Windows `CurrentBuildNumber` from `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` ≥ 19041); pure `fn version_ok(major_or_build: u32, min: u32) -> bool`.
- Commands: `get_screen_access() -> Option<bool>` (`None` on Windows), `request_screen_access()` (`CGRequestScreenCaptureAccess`), `open_screen_access_settings()` (`x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture`).
- UI: a third `Row` in `PermissionsSection` (label "Screen & System Audio Recording" / "화면 및 시스템 오디오 기록", hint "Needed to capture and record the screen" / "화면 캡처와 녹화에 필요해요"): granted → `BADGE` "Allowed"; not granted → button "Allow" calling `request_screen_access`, and after one request the button becomes "Open System Settings". macOS only reports granted after a relaunch, so the hint under a requested-but-not-granted row says "Quit and reopen Vee after allowing" / "허용한 뒤 Vee를 다시 실행해 주세요". The section's early `return null` must account for the new row.

- [ ] **Step 1: Failing test** `version_ok_compares_inclusive`: `(15,15)` true, `(14,15)` false, `(19041,19041)` true, `(19040,19041)` false.
- [ ] **Step 2:** Run → fail. **Step 3:** Implement. **Step 4:** Pass + `yarn typecheck`.
- [ ] **Step 5:** Commit `feat: screen recording permission row`.

### Task 5: Capture overlay and capture flow

**Files:**
- Modify: `src-tauri/tauri.conf.json` (window `capture`: `create:false`, `visible:false`, `decorations:false`, `transparent:true`, `alwaysOnTop:true`, `skipTaskbar:true`, `resizable:false`, `shadow:false`, `visibleOnAllWorkspaces:true`), `src-tauri/capabilities/default.json` (add `capture`, `recorder`), `src/app/main.tsx` (route `capture`), `src/app/globals.css` (`[data-window=capture]` transparent background; `--vee-rec` light/dark), `src-tauri/src/app/capture/mod.rs`, `src-tauri/src/app/editor.rs` (make `copy_to_history` `pub(crate)`; no other change), `src-tauri/src/lib.rs`, i18n files
- Create: `src-tauri/src/app/capture/shot.rs`, `src/app/capture/CaptureApp.tsx`, `src/domain/capture/apis/capture.ts`, `src/domain/capture/apis/capture.dto.ts`

**Interfaces:**
- Consumes: Task 1 `Global::Capture`, Task 2 `play_shutter`, Task 3 types + `list`, Task 4 `granted`.
- Produces: `capture::CAPTURE = "capture"`; `enum Mode { Capture, Record }`; `capture::open(app, Mode)`: on macOS without `granted()` → `window::show_settings(app, "permissions")` and stop; otherwise take `window::cursor_monitor`, build `MonitorInfo`, `window_list::list`, place the window over the whole monitor (macOS: above the menu bar — raise the NSWindow level to `NSScreenSaverWindowLevel` with objc2-app-kit), show + focus, emit `capture://open` `{ mode, monitor, windows }` to it. A second `open` while it is visible is ignored.
- Commands: `take_capture(selection: Selection)` (async): hide the overlay, wait 120 ms, grab (`shot::grab(&MonitorInfo, &Selection) -> AppResult<DynamicImage>`: macOS `screencapture -x -R x,y,w,h <tmp>.png` or `-x -o -l <id> <tmp>.png`, read, delete the tmp; Windows GDI `BitBlt` of the physical rect, window = its rect), then `editor::copy_to_history`, then on the main thread `play_shutter` (if `capture_sounds`), emit `clips://changed`, `window::show_toast(app, ToastPayload::copied(&content, false))`. `cancel_capture()` hides the overlay.
- `CaptureApp` (TS): dims the monitor (`bg-black/35`), crosshair; drag draws the rect with a size label (`W × H` in physical px = logical × scale); `Space` toggles window mode (hover highlights `windowAt`, click selects); `Esc` → `cancel_capture`; `Enter` in record mode starts (Task 8). In capture mode, mouse-up with a non-null `normalizeRect` or a window click calls `take_capture`. Resets all state on every `capture://open`.

- [ ] **Step 1:** Wire the shortcut action `Capture` → `capture::open(app, Mode::Capture)`.
- [ ] **Step 2:** Implement Rust + TS above. No new unit tests (pure parts are covered by Task 3).
- [ ] **Step 3:** Run all four checks.
- [ ] **Step 4: Manual check in the dev app** (macOS, and Windows if available): region capture lands in the clipboard and the panel with the shutter sound; Space → window capture has no dim/border in the image; Esc cancels; capture on a second monitor and on a Retina display has the right pixels; light/dark, ko/en.
- [ ] **Step 5:** Commit `feat: screen capture overlay`.

### Task 6: Recording engines

**Files:**
- Create: `src-tauri/src/app/recorder/mod.rs` (only `mod mac; mod win; pub mod plan;` + the `Engine` trait-free API below), `src-tauri/src/app/recorder/mac.rs`, `src-tauri/src/app/recorder/win.rs`, `src-tauri/src/app/recorder/plan.rs`
- Modify: `src-tauri/src/app/mod.rs`

**Interfaces:**
- Produces: `struct Running` (per-OS, opaque) with `fn start(app: &AppHandle, monitor: &MonitorInfo, sel: &Selection, container: Container, out: &Path) -> AppResult<Running>` and `fn stop(self) -> AppResult<PathBuf>` (blocks until the file is final) and `fn cancel(self)` (stops and deletes). `enum Container { Mp4, Mov }` (`plan::container_for(format) -> Container`: `mov` → Mov, everything else → Mp4).
- macOS (`mac.rs`): `SCShareableContent` → the `SCDisplay` for the monitor; filter = display excluding Vee's `SCRunningApplication` (rect mode, `sourceRect` = the display-local rect) or `initWithDesktopIndependentWindow` for `Selection::Window`; config: width/height = physical rect (even), `showsCursor = true`, `capturesAudio = true`, `excludesCurrentProcessAudio = true`, 30 fps; `SCRecordingOutput` with `outputFileType` `public.mpeg-4` / `com.apple.quicktime-movie`; `stop` waits for the delegate's `recordingOutputDidFinishRecording` (10 s timeout → `AppError::Other`). Bridge completion handlers with `block2` + a channel.
- Windows (`win.rs`): `Selection::Window { id }` → the window's `DWMWA_EXTENDED_FRAME_BOUNDS` at start (the HWND is `id`), recorded as a fixed rect; then an FFmpeg child (`process::command`, kill-on-drop guard) with `plan::grab_args(…)` writing `<tmp>/video.mkv`, and a WASAPI loopback thread (`IAudioClient` with `AUDCLNT_STREAMFLAGS_LOOPBACK` on the default render device) writing `<tmp>/audio.wav`; `stop` writes `q` to FFmpeg's stdin, joins the audio thread, then runs FFmpeg with `plan::mux_args(…)` to `out`. Gdigrab vs ddagrab and A/V sync are the implementer's call (spec §9); document the choice in a comment.
- `plan.rs` (pure): `grab_args(phys: (i32,i32,u32,u32), out: &Path) -> Vec<OsString>`; `mux_args(video: &Path, audio: &Path, out: &Path) -> Vec<OsString>` (`-c:v copy -c:a aac -shortest`); `even(n: u32) -> u32`.

- [ ] **Step 1: Failing tests** in `plan.rs`: `grab_args_sets_offset_and_even_size` (`(10,20,301,201)` → contains `-offset_x 10 -offset_y 20 -video_size 300x200`, `-draw_mouse 1`, ends with the out path); `mux_args_copies_video_and_encodes_aac`; `container_for_maps_formats` (`mov`→Mov, `mp4`/`webm`/`gif`→Mp4).
- [ ] **Step 2:** Run → fail. **Step 3:** Implement `plan.rs`, then the engines. **Step 4:** Tests pass.
- [ ] **Step 5: Memory check (macOS):** with a temporary debug command, record 30 s at full screen, stop, wait 10 s; `footprint <pid>` before vs after ≤ +50 MB. If not, move `mac.rs` behind a child `vee-app --record-worker <args>` (checked in `main.rs` like `--ai-worker`) and re-measure. Record the numbers in the commit message. Remove the debug command.
- [ ] **Step 6:** Commit `feat: screen recording engines`.

### Task 7: Recording flow and finalize

**Files:**
- Modify: `src-tauri/src/app/recorder/mod.rs`, `src-tauri/src/app/recorder/plan.rs`, `src-tauri/src/lib.rs` (commands; on exit call `recorder::shutdown`)

**Interfaces:**
- Consumes: Task 5 `capture::open`, Task 6 engines, Task 2 sounds, `engines::ffmpeg`, `video::convert`, `editor`-free clip path: `clip::upsert(NewClip::Files { paths: vec![out], thumbs: vec![] })` with the Vee source app (same as `copy_to_history`), `window::write_clipboard`, `window::show_toast(app, ToastPayload::convert(ConvertNote::Saved { format, count: 1, folder }, Kind::Files))`; failures → `ConvertNote::Failed`.
- Produces: managed `Recorder` state with `enum Phase { Idle, Selecting, Recording { started_ms: i64, format: RecordFormat, limit_ms: Option<i64> }, Finishing }`; pure transition `fn next(phase: &Phase, ev: Event) -> (Phase, Action)` for `Event::{Shortcut, Start, Stop, Cancel, Finished, LimitReached}` and `Action::{OpenOverlay, StartEngine, StopEngine, CancelEngine, Nothing}`.
- `recorder::on_shortcut(app)`: Idle → gates (macOS `granted`, `recording_supported`, Windows `engines::ffmpeg` present — else toast + `show_settings("convert")`; unsupported OS → toast) → `capture::open(app, Mode::Record)`; Recording → stop; Selecting/Finishing → nothing.
- Commands: `start_recording(selection, format: String)`: hide overlay, `sound::set_recording(true)`, play start sound (if `capture_sounds`), wait 400 ms, `Running::start` into `<temp>/vee-record-<identifier>/`, show `recorder` window, emit `recorder://started { startedMs, format, limitMs }` (`limitMs = 30000` for gif); spawn the GIF limit timer. `stop_recording()`, `cancel_recording()`.
- Finalize (off the main thread): `Running::stop` → for `webm`/`gif` `video::convert` (GIF with `Quality::Original`, WEBM with the `videoQuality` setting; reusing the existing GIF pipeline means GIF is 15 fps and ≤ 800 px wide instead of the spec's 960 — tell the user) → move into `resolve_folder` with `unique_path(file_name(now))` → clip + clipboard → `sound::set_recording(false)` → stop sound → toast. On convert failure keep the MP4 in the folder and show `Failed`. Always delete the temp dir. `recorder::shutdown(app)` cancels a running recording on exit.
- Pure helpers in `plan.rs`: `file_name(local: (y,m,d,h,mi,s), ext: &str) -> String` → `"Vee Recording 2026-11-03 at 15.12.08.mp4"`; `unique_path(dir, name, exists: impl Fn(&Path)->bool) -> PathBuf`; `resolve_folder(setting: &str, downloads: &Path, is_dir: impl Fn(&Path)->bool) -> (PathBuf, bool /* fell back */)`; `needs_ffmpeg(format, macos: bool) -> bool` (macOS: webm/gif; Windows: always).

- [ ] **Step 1: Failing tests** (`plan.rs` + `mod.rs`):
  - `file_name_is_fixed_format` → `"Vee Recording 2026-11-03 at 15.12.08.mp4"`.
  - `unique_path_appends_counter`: existing `a.mp4`, `a (2).mp4` → `a (3).mp4`.
  - `resolve_folder_falls_back_to_downloads`: `""` → downloads, false; missing dir → downloads, true; existing dir → it, false.
  - `needs_ffmpeg_per_os`.
  - `shortcut_while_recording_stops` / `shortcut_while_selecting_or_finishing_does_nothing` / `limit_reached_stops_once` (a second `LimitReached` or `Stop` in `Finishing` → `Nothing`).
- [ ] **Step 2:** Run → fail. **Step 3:** Implement. **Step 4:** Pass.
- [ ] **Step 5:** Commit `feat: recording flow and save`.

### Task 8: Pre-record pill and recorder window

**Files:**
- Modify: `src/app/capture/CaptureApp.tsx`, `src-tauri/tauri.conf.json` (window `recorder`: like `toast` — `focus:false`, `focusable:false`, transparent, always on top, ~360×64), `src/app/main.tsx`, `src-tauri/src/app/recorder/mod.rs` (place `recorder` at the monitor's bottom centre; Windows `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` on it and on `capture`), i18n files
- Create: `src/app/recorder/RecorderApp.tsx`, `src/utils/recording.ts`, `src/utils/recording.test.ts`

**Interfaces:**
- Consumes: `capture://open` with `mode: 'record'`, `recorder://started`, commands from Task 7, `useSettings`, the FFmpeg engine status query already used by `ConvertSection`.
- Produces (TS pure): `formatElapsed(ms: number): string` (`"00:12"`, `"1:02:03"` past an hour); `gifRemaining(elapsedMs: number, limitMs: number): { seconds: number; share: number }` (seconds rounded up, clamped ≥ 0; share 0..1); `initialFormat(saved: RecordFormat, ffmpeg: boolean, mac: boolean): RecordFormat` (macOS without FFmpeg + webm/gif → `mp4`).
- Pre-record pill (in `CaptureApp`, record mode, after a selection): glass pill at the monitor's bottom centre: `Segmented` MP4 · MOV · WEBM · GIF (WEBM/GIF dimmed with an "FFmpeg needed" tooltip on macOS without FFmpeg), gradient "Record" button, ✕ cancel; `Enter` = Record. Selection stays visible and can be redrawn until Record.
- `RecorderApp`: pulsing `--vee-rec` dot (off under reduced motion), `formatElapsed`, for GIF a "GIF · N s left" label + bar from `gifRemaining`, stop (■, red) and cancel (✕) icon buttons with `aria-label`s.

- [ ] **Step 1: Failing tests** in `recording.test.ts`: `formatElapsed(12_000) === '00:12'`, `formatElapsed(3_723_000) === '1:02:03'`; `gifRemaining(12_000, 30_000)` → `{seconds: 18, share: 0.4}`, `gifRemaining(31_000, 30_000).seconds === 0`; `initialFormat('gif', false, true) === 'mp4'`, `initialFormat('gif', true, true) === 'gif'`, `initialFormat('webm', false, false) === 'webm'`.
- [ ] **Step 2:** Run → fail. **Step 3:** Implement. **Step 4:** Four checks pass.
- [ ] **Step 5: Manual check:** record region and window in each format; the pill and dim are not in the video; the file opens, is in Downloads, is on the clipboard (paste into Finder/Explorer) and in the panel; GIF stops itself at 30 s; the record shortcut stops; system audio is in MP4/MOV/WEBM and Vee's start/stop/copy sounds are not; light/dark, ko/en.
- [ ] **Step 6:** Commit `feat: recording pill`.

### Task 9: Settings UI (General and Shortcuts)

**Files:**
- Modify: `src/app/settings/SettingsApp.tsx`, `src/shared/i18n/{en,ko}.ts`

**Interfaces:**
- Consumes: Task 1 commands and keys, `@tauri-apps/plugin-dialog` `open({ directory: true })` (already a dependency), `Segmented`, `Row`, `Button`, `ShortcutRecorder`.
- General tab: Row "Recording folder" / "녹화 저장 폴더" — hint shows the folder (`recordFolder` or "Downloads" / "다운로드"), button "Change…" / "변경…" → dialog → `setSetting('recordFolder', path)`. Row "Recording format" / "녹화 포맷" — `Segmented` MP4/MOV/WEBM/GIF; hint "GIF up to 30 seconds, no sound" / "GIF는 최대 30초, 소리 없음", plus "WEBM and GIF need FFmpeg (Settings › Convert)" / "WEBM과 GIF는 FFmpeg가 필요해요(설정 › 변환)" when FFmpeg is missing.
- Shortcuts tab: `SHORTCUT_KEYS` gains `captureShortcut` (label "Screen capture" / "스크린 캡처", hint "Select an area; Space picks a window" / "영역 선택, Space로 창 선택") and `recordShortcut` ("Screen recording" / "스크린 녹화", hint "Press again to stop" / "한 번 더 누르면 정지"); both save via `setShortcut(key, accel)`. Under any row whose key is in `getShortcutFailures()` (or whose save just failed): a warning line "Another app or macOS is using this shortcut" / "다른 앱이나 macOS가 이 단축키를 쓰고 있어요" and, on macOS, a button "Open Keyboard Shortcuts" / "키보드 단축키 열기" → `openKeyboardSettings()`.

- [ ] **Step 1:** Implement. **Step 2:** Four checks pass.
- [ ] **Step 3: Manual check:** change folder → next recording lands there; delete that folder → recording lands in Downloads with the fallback note; set capture to `⌘⇧4` (taken by macOS) → warning + button opens Keyboard Shortcuts; light/dark, ko/en (Korean strings fit).
- [ ] **Step 4:** Commit `feat: capture and recording settings`.

### Task 10: Docs and final verification

**Files:**
- Modify: `docs/agents/overview.md` (eight windows; capture/recording in "Rust side"), `docs/agents/architecture.md` (Windows table rows for `capture` and `recorder`; a "Capture & recording" section: flow, engines, gates, temp dir, sounds), `README.md` feature list if it lists features

- [ ] **Step 1:** Update the docs (agents docs in English).
- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` — all pass.
- [ ] **Step 3:** Final `footprint` check after a recording and after a capture (≤ +50 MB).
- [ ] **Step 4:** Commit `docs: screen capture and recording`.
