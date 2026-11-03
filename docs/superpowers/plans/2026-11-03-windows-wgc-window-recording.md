# Windows WGC Window Recording Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On Windows, a window recording captures only the chosen window through Windows.Graphics.Capture (WGC), following it when it moves, instead of gdigrab-ing the screen rect it had at the start.

**Architecture:** A new `recorder/wgc.rs` owns the WGC session and keeps the latest frame as tight BGRA. In `win.rs`, a pump thread writes one frame every 1/30 s (repeating the last one when nothing new arrived) into FFmpeg's stdin as rawvideo, at a fixed output size. FFmpeg encodes to the same `video.mkv`, and the existing loopback WAV and mux steps run unchanged. Pure pieces (output size, fit rect, frame composition, frame schedule, FFmpeg args) live in `plan.rs` and are unit-tested on macOS.

**Tech Stack:** Rust, `windows` 0.62.2 (WinRT `Windows.Graphics.Capture`, D3D11/DXGI), `image` 0.25 (resize), FFmpeg rawvideo stdin.

**Spec:** `docs/superpowers/specs/2026-11-03-windows-wgc-window-recording-design.md`

## Global Constraints

- Only the `Selection::Window` path on Windows changes. `Selection::Rect` keeps gdigrab, audio keeps system-wide WASAPI loopback, and macOS code and `capture/shot.rs` are untouched.
- `Running`'s public API (`start` / `stop` / `cancel`) and `recorder/mod.rs` stay as they are.
- Output size is the WGC item size at start, longest side capped at **1920** (aspect kept), then rounded down to even (`plan::even`).
- Frames larger than the output are scaled down to fit, keeping aspect. Smaller ones are drawn 1:1 and centred, never upscaled. The rest is black (`[0,0,0,255]` BGRA).
- 30 fps, cursor included (`IsCursorCaptureEnabled(true)`). Try `IsBorderRequired(false)` and ignore failure.
- Fallback to the old gdigrab rect path when `GraphicsCaptureSession::IsSupported()` is false, the D3D11 device can't be made (hardware, then WARP), or `CreateForWindow` fails on a window that still exists (`IsWindow` true). A window that no longer exists (`IsWindow` false) → `Err`.
- Dependency change approved: add to the `windows` feature list only (`Graphics_Capture`, `Graphics_DirectX`, `Graphics_DirectX_Direct3D11`, `Win32_Graphics_Direct3D`, `Win32_Graphics_Direct3D11`, `Win32_Graphics_Dxgi`, `Win32_Graphics_Dxgi_Common`, `Win32_System_WinRT`, `Win32_System_WinRT_Direct3D11`, `Win32_System_WinRT_Graphics_Capture`). Adding or removing feature names within `windows` is in scope. A new crate or a version change needs the user's approval: stop and ask.
- Errors go through the existing `other(step, e)` → `AppError::Other`. No new user-visible strings. Comments in English.
- After stop or cancel, no WGC session, frame pool, D3D device, thread or FFmpeg process may remain, and frame buffers are dropped. The user's top priority is memory returning.
- This Mac can't cross-compile the full crate for Windows: `ring` needs MSVC headers. Windows code is type-checked with a scratch crate (Task 2).

## Review Focus

- FFmpeg slower than 30 fps (large window, slow PC): pipe writes block and the pump falls behind. The video must still match wall-clock length (catch up by writing the current frame for every tick that is due), or A/V sync drifts. Pinned by `frames_due` tests in Task 1.
- Stop in pipe mode: writing `q` into a rawvideo stdin corrupts the stream. Stop must end the pump and close stdin, never write `q`. Covered by a code rule in Task 3, checked in review.
- Target window closed or minimized mid-recording: WGC stops delivering frames, or the session errors. The pump keeps repeating the last frame, and neither thread panics or exits early.
- Elevated (admin) or protected target window: `CreateForWindow` fails while `IsWindow` is true. This must fall back to gdigrab, not fail the recording.
- Window resized mid-recording: the frame pool is recreated at the new `ContentSize`, and frames are composed to the fixed output with no size change reaching FFmpeg. Pinned by `fit_rect` and `compose` tests in Task 1.

---

### Task 1: Pure frame geometry, composition, schedule and pipe args

**Files:**
- Modify: `src-tauri/src/app/recorder/plan.rs` (new functions next to `grab_args`; tests in its `mod tests`)

**Interfaces:**
- Produces (all `pub`, each `#[cfg_attr(not(windows), allow(dead_code))]` like `grab_args`):
  - `pub const MAX_SIDE: u32 = 1920;`
  - `pub const FPS: u32 = 30;`
  - `pub fn output_size(w: u32, h: u32) -> (u32, u32)`
  - `pub fn fit_rect(src: (u32, u32), dst: (u32, u32)) -> (u32, u32, u32, u32)`: `(x, y, w, h)` inside `dst`
  - `pub fn compose(src: &[u8], src_size: (u32, u32), dst_size: (u32, u32)) -> Vec<u8>`: tight BGRA in → tight BGRA `dst` out
  - `pub fn frames_due(elapsed: std::time::Duration) -> u64`: how many frames should have been written by `elapsed` (tick 0 at `elapsed = 0`)
  - `pub fn pipe_args(w: u32, h: u32, out: &Path) -> Vec<OsString>`

- [ ] **Step 1: Write the failing tests** in `plan.rs` `mod tests`:

```rust
#[test]
fn output_size_caps_longest_side_and_keeps_even() {
    assert_eq!(output_size(3840, 2160), (1920, 1080));
    assert_eq!(output_size(2160, 3840), (1080, 1920));
    assert_eq!(output_size(4000, 1000), (1920, 480));
    assert_eq!(output_size(1000, 700), (1000, 700));
    assert_eq!(output_size(1001, 701), (1000, 700));
    assert_eq!(output_size(1920, 1200), (1920, 1200));
    assert_eq!(output_size(1, 1), (2, 2));
}

#[test]
fn fit_rect_shrinks_larger_and_centres_smaller() {
    assert_eq!(fit_rect((1920, 1080), (1920, 1080)), (0, 0, 1920, 1080));
    assert_eq!(fit_rect((800, 600), (1920, 1080)), (560, 240, 800, 600)); // no upscale
    assert_eq!(fit_rect((3840, 1080), (1920, 1080)), (0, 270, 1920, 540));
    assert_eq!(fit_rect((1000, 2000), (1920, 1080)), (690, 0, 540, 1080));
    let (_, _, w, h) = fit_rect((10_000, 1), (100, 100));
    assert!(w >= 1 && h >= 1);
}

#[test]
fn compose_same_size_is_identity() {
    let src: Vec<u8> = (0..2 * 2 * 4).map(|i| i as u8).collect();
    assert_eq!(compose(&src, (2, 2), (2, 2)), src);
}

#[test]
fn compose_centres_smaller_on_black() {
    let red = [0u8, 0, 255, 255];
    let src: Vec<u8> = red.repeat(4); // 2×2
    let out = compose(&src, (2, 2), (4, 4));
    assert_eq!(out.len(), 4 * 4 * 4);
    let px = |x: usize, y: usize| &out[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
    assert_eq!(px(0, 0), [0, 0, 0, 255]);
    assert_eq!(px(1, 1), red);
    assert_eq!(px(2, 2), red);
    assert_eq!(px(3, 3), [0, 0, 0, 255]);
}

#[test]
fn compose_shrinks_wider_with_letterbox() {
    let src: Vec<u8> = [255u8, 255, 255, 255].repeat(4 * 2); // white 4×2
    let out = compose(&src, (4, 2), (2, 2)); // fits as 2×1 at y = 0
    assert_eq!(out.len(), 2 * 2 * 4);
    assert_eq!(&out[2 * 4..], [0, 0, 0, 255, 0, 0, 0, 255]); // row 1 black
    assert!(out[..2 * 4].iter().all(|&b| b > 200)); // row 0 white-ish
}

#[test]
fn frames_due_follows_the_wall_clock() {
    use std::time::Duration;
    assert_eq!(frames_due(Duration::ZERO), 1);
    assert_eq!(frames_due(Duration::from_millis(33)), 1);
    assert_eq!(frames_due(Duration::from_millis(34)), 2);
    assert_eq!(frames_due(Duration::from_secs(1)), 31);
    assert_eq!(frames_due(Duration::from_secs(60)), 1801);
}

#[test]
fn pipe_args_read_bgra_from_stdin() {
    let a = pipe_args(1920, 1080, Path::new("v.mkv"));
    let s: Vec<&str> = a.iter().map(|x| x.to_str().unwrap()).collect();
    assert_eq!(
        s,
        [
            "-y", "-hide_banner", "-loglevel", "error", "-nostats", "-f", "rawvideo", "-pix_fmt", "bgra", "-s", "1920x1080",
            "-r", "30", "-i", "-", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "18", "-pix_fmt", "yuv420p", "v.mkv"
        ]
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib recorder::plan`
Expected: compile errors for `output_size`, `fit_rect`, `compose`, `frames_due` and `pipe_args` not found.

- [ ] **Step 3: Implement the five functions and two constants in `plan.rs`**

- `output_size`: if `max(w, h) > MAX_SIDE`, scale both by `MAX_SIDE / max` (round) first, then apply `even` to each side.
- `fit_rect`: scale = `min(1, dst.w/src.w, dst.h/src.h)`. Size = `round(src * scale)` clamped to ≥ 1, position = `(dst - size) / 2` (integer floor).
- `compose`: return `src.to_vec()` when the sizes are equal. Otherwise fill a black canvas (`[0,0,0,255]`) and resize `src` to the fit size with `image::imageops::resize` (`FilterType::Triangle`) when it differs, treating BGRA bytes as an `RgbaImage`: channel order doesn't matter for resizing. Copy the rows in at `(x, y)`.
- `frames_due`: `elapsed.as_nanos() * FPS / 1e9` floored, plus 1.
- `pipe_args`: exactly the vector in the test. Doc comment: rawvideo BGRA from stdin, same encoder settings as `grab_args`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib recorder::plan`
Expected: all `recorder::plan` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/app/recorder/plan.rs
git commit -m "feat: frame sizing, composition and pipe args for window recording on Windows"
```

---

### Task 2: WGC capture module and the Windows type-check harness

**Files:**
- Modify: `src-tauri/Cargo.toml:47` (the `windows` features; the approved list in Global Constraints)
- Create: `src-tauri/src/app/recorder/wgc.rs`
- Modify: `src-tauri/src/app/recorder/mod.rs` (add `#[cfg(windows)] mod wgc;` next to `mod win;`; nothing else)
- Create (scratch, git-ignored): `.superpowers/tmp/wincheck/` (a Cargo crate)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces (in `wgc.rs`, all `pub(super)`):
  - `pub(super) struct Frame { pub width: u32, pub height: u32, pub bgra: Vec<u8> }`: tight rows (`width * 4` bytes)
  - `pub(super) enum StartError { Unavailable(String), Gone }`: `Unavailable` means fall back to gdigrab, `Gone` means the window no longer exists
  - `pub(super) struct Capture`, with:
    - `pub(super) fn start(hwnd: u64) -> Result<Capture, StartError>`
    - `pub(super) fn size(&self) -> (u32, u32)`: the item size at start
    - `pub(super) fn latest(&self) -> Option<std::sync::Arc<Frame>>`: the newest frame, `None` until the first one
    - `impl Drop for Capture`: closes the session and frame pool, stops and joins its thread, and drops buffers

- [ ] **Step 1: Build the type-check harness first** (so Step 3 has a failing check to turn green). Create `.superpowers/tmp/wincheck/`:
  - `Cargo.toml`: package `wincheck`, edition 2024, `[lib] path = "src/lib.rs"`. Dependencies: the same `windows` version and full feature list as `src-tauri/Cargo.toml` after Step 2's edit, `image` with the Windows features, `log = "0.4"`, and `tauri = { path = "stubs/tauri" }`.
  - `stubs/tauri`: a crate named `tauri` whose lib is `pub struct AppHandle;`.
  - `src/lib.rs`: mirrors the module paths `win.rs` uses, with stub bodies taken from the real signatures:
    - `crate::app::capture::geometry::{MonitorInfo, Selection, to_physical}` (copy `geometry.rs` lines 8–14, 34–37 and 48–52, with `to_global` stubbed)
    - `crate::app::engines::ffmpeg(&AppHandle) -> Option<(PathBuf, PathBuf)>`
    - `crate::domain::convert::{Format, Killed, command}`: `Format { Mp4, Mov, Webm, Gif }` deriving `PartialEq, Eq, Clone, Copy, Debug`, `Killed(pub Child)` with a `Drop` that kills, and `command(&Path) -> Command`
    - `crate::shared::error::{AppError, AppResult}`: `AppError::Other(String)`, `From<std::io::Error>`, and `type AppResult<T> = Result<T, AppError>`
    - `pub mod app { pub mod recorder { #[path = "<abs>/src-tauri/src/app/recorder/plan.rs"] pub mod plan; #[path = "<abs>/…/wgc.rs"] mod wgc; #[path = "<abs>/…/win.rs"] mod win; } }`. Add whatever else `plan.rs` and `win.rs` import, copying real signatures.

  Run: `cargo check --manifest-path .superpowers/tmp/wincheck/Cargo.toml --target x86_64-pc-windows-msvc`
  Expected: FAIL only because `wgc.rs` doesn't exist yet. Before creating `wgc.rs`, comment out its `mod` line once and confirm `win.rs` and `plan.rs` check clean. That proves the stubs are right.

- [ ] **Step 2: Add the approved `windows` features** to `src-tauri/Cargo.toml:47` and the harness `Cargo.toml`.

- [ ] **Step 3: Implement `wgc.rs`**
  - `start`: if `!IsWindow(hwnd)` → `Gone`. If `!GraphicsCaptureSession::IsSupported()` → `Unavailable`. Create the D3D11 device with `D3D11_CREATE_DEVICE_BGRA_SUPPORT`: hardware, then WARP. Failure → `Unavailable`. Wrap it as `IDirect3DDevice` via `CreateDirect3D11DeviceFromDXGIDevice`. Make the item with `IGraphicsCaptureItemInterop::CreateForWindow`; failure → `Unavailable` (the window exists, see Global Constraints). Create `Direct3D11CaptureFramePool::CreateFreeThreaded` (`B8G8R8A8UIntNormalized`, 2 buffers, item size) and the session. Set `IsCursorCaptureEnabled(true)`, try `IsBorderRequired(false)` (on error: `log::debug!` and continue), then `StartCapture`.
  - Frames: on each frame, if `ContentSize` ≠ pool size, `Recreate` at the new size and skip the frame. Otherwise copy to a reused staging texture (`D3D11_USAGE_STAGING`, `CPU_ACCESS_READ`), `Map`, copy `width * 4` bytes per row honouring `RowPitch`, and `Unmap`. Store `Arc<Frame>` in `Arc<Mutex<Option<Arc<Frame>>>>`. No error inside the frame handler may panic: log it and skip the frame.
  - `Drop`: `Close()` the session and the pool, then drop the device. Leave no thread or callback running (if `FrameArrived` is used, remove its token before closing).
  - The module doc comment says why WGC is used (only the window, follows it) and what the yellow border does on Windows 10.

- [ ] **Step 4: Type-check on the Windows target**

Run: `cargo check --manifest-path .superpowers/tmp/wincheck/Cargo.toml --target x86_64-pc-windows-msvc`
Expected: `Finished` with no errors. An unused-code warning for `wgc` items is fine until Task 3.

- [ ] **Step 5: macOS build still fine**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: all pass (the `wgc` module is `cfg(windows)`).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/app/recorder/wgc.rs src-tauri/src/app/recorder/mod.rs
git commit -m "feat: Windows.Graphics.Capture session for a single window"
```

---

### Task 3: Record the window through WGC in `win.rs`, with gdigrab fallback

**Files:**
- Modify: `src-tauri/src/app/recorder/win.rs` (`Running`, `start`, `finish`, `halt`, module doc)
- Modify: `docs/agents/overview.md:28` and `docs/agents/architecture.md` (wherever they say Windows recording is "FFmpeg + WASAPI loopback": add that window recording grabs frames with WGC, region recording with gdigrab)

**Interfaces:**
- Consumes: Task 1's `plan::{output_size, compose, frames_due, pipe_args, FPS}`. Task 2's `wgc::{Capture, StartError, Frame}`.
- Produces: no new public items. `Running` keeps its API.

- [ ] **Step 1: Write the failing check.** Add the `win.rs` changes below and run the harness. It fails until the code is consistent. There is no runtime test on macOS, so the harness check plus Task 1's tests are this task's gate (ledger it as a ruling).

- [ ] **Step 2: Implement in `win.rs`**
  - `Selection::Window { id }`: `wgc::Capture::start(id)`.
    - `Ok(cap)` → pipe mode.
    - `Err(Unavailable(why))` → `log::warn!("recording: WGC unavailable ({why}); grabbing the screen rect")` and the existing rect path.
    - `Err(Gone)` → `Err(other("window", format!("{id} is gone")))`.
  - Pipe mode:
    - `(w, h) = plan::output_size(cap.size())`. Spawn FFmpeg with `plan::pipe_args(w, h, &video_file)`, `stdin(Stdio::piped())`, and stderr to the log as now.
    - Take its stdin into a pump thread with `cap`, a `stop: Arc<AtomicBool>` and the start `Instant`.
    - Loop until `stop`: `due = frames_due(now - t0)`. While `written < due`, write the current composed frame (`plan::compose(&f.bgra, (f.width, f.height), (w, h))`, recomposed only when `cap.latest()` returns a different `Arc` (`Arc::ptr_eq`); black `w*h*4` before the first frame) and `written += 1`. Then sleep until the next tick.
    - A write error (FFmpeg died) ends the loop. On exit it drops stdin and then `cap`.
    - Store it as `pump: Option<(Arc<AtomicBool>, JoinHandle<()>)>` on `Running`.
  - `finish`: in pipe mode (`pump.is_some()`), set `stop`, join the pump (which closes stdin), and **never write `q`**. Then wait for FFmpeg exactly as now. Rect mode keeps writing `q`.
  - `halt` (so also `Drop` and `fail`): stop and join the pump before dropping `grab`.
  - Early-exit check after spawn: unchanged, for both modes.
  - Update the module doc: the window path uses WGC + rawvideo pipe at ≤ 1920 px longest side, 30 fps with repeated frames, and gdigrab fallback.

- [ ] **Step 3: Type-check on the Windows target**

Run: `cargo check --manifest-path .superpowers/tmp/wincheck/Cargo.toml --target x86_64-pc-windows-msvc`
Expected: `Finished`, and no unused warnings for `wgc` or the new `plan` functions.

- [ ] **Step 4: The four checks**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib && mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/app/recorder/win.rs docs/agents/overview.md docs/agents/architecture.md
git commit -m "feat: record only the chosen window on Windows through Windows.Graphics.Capture"
```

- [ ] **Step 6: Hand the user the Windows device checklist** from the spec ("Windows 기기 확인 항목", items 1–8).
