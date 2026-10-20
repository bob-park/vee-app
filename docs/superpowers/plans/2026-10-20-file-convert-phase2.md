# Drag File Convert — Phase 2 (FFmpeg + video) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Video files dragged out of the panel convert through the donut (MP4, WEBM, MOV, GIF, MP3, 720p), using an FFmpeg the user downloads from the Settings "Convert" tab, with percent + remaining time on the card and a cancel button.

**Architecture:** `domain/convert` learns video sources and FFmpeg argument building, progress parsing and a cancellable run (no `tauri`). A new shell module `app/engines.rs` downloads, verifies (SHA-256), unpacks and removes FFmpeg under the app data dir with `curl` and the OS unzip tool, publishing `engines://ffmpeg` status. The donut gets per-slice availability (`needs`), dimming video slices while FFmpeg is missing. Jobs gain `percent`/`eta` and a cancel flag.

**Tech Stack:** Rust (Tauri 2.12, existing `sha2`), system `curl`, `ditto` (macOS) / `tar` (Windows 10 1803+), FFmpeg 9.0.2 CLI; React 19 + Tailwind 4/daisyUI 5.

**Spec:** `docs/superpowers/specs/2026-10-20-file-convert-design.md` (phase ②). Phase 1 plan: `docs/superpowers/plans/2026-10-20-file-convert-phase1.md` — its Global Constraints still hold.

## Global Constraints

- Branch `feature/file-convert` (continues phase 1). Commit messages `<prefix>(<scope>): <subject>`.
- **No dependency changes.** Downloads go through the system `curl` (macOS; `curl.exe` on Windows 10 1803+), unpacking through `ditto -x -k` (macOS) / `tar -xf` (Windows), hashing through the existing `sha2` crate.
- FFmpeg assets, pinned (version `9.0.2`):
  - macOS arm64: `https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/ffmpeg.zip` sha256 `c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924` size `28395699`; `…/ffprobe.zip` sha256 `fcbe839537485eaee7a7a8bc5cbc0f90d53617e80943e8a5b2e31cb851197ea6` size `28317701`.
  - Windows x64: `https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip` sha256 `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba` size `114768076` (contains `…/bin/ffmpeg.exe`, `…/bin/ffprobe.exe`).
- Install dir: `<app data dir>/engines/ffmpeg/` holding `ffmpeg`/`ffprobe` (`.exe` on Windows) and `meta.json` `{ "version": "9.0.2", "installedAt": <ms>, "size": <bytes on disk> }`. A dir without both binaries is "missing". Never touch anything outside it; never keep a partial download there (download to `<temp>/vee-ffmpeg-download/`, move in only after verify + unpack).
- Network only when the user presses Download / Update (docs/agents/overview.md must say so).
- Video sources (case-insensitive): `mp4 m4v mov mkv webm avi wmv flv mpg mpeg mts m2ts 3gp` (`ts` left out: usually TypeScript). Video targets in donut order: `mp4, webm, mov, gif, mp3, 720p`. A source's own container is left out (`mp4`/`m4v` → no `mp4`; `mov` → no `mov`; `webm` → no `webm`); `720p` always stays. Images and videos mixed in one card → unsupported toast (as phase 1).
- FFmpeg missing → video slices still shown, dimmed, with reason `t.convert.needsFfmpeg`; a drop on them does nothing (Rust rejects too).
- Setting `videoQuality`: `small` | `normal` | `original`, default `normal`.
- Encoding (exact args, `-y -nostats -progress pipe:1 -i <src>` first, `<out>` last):
  - `mp4` / `mov` / `720p`: `-c:v libx264 -crf {28|23|18} -preset {veryfast|medium|slow} -pix_fmt yuv420p -c:a aac -b:a {96k|128k|192k} -movflags +faststart` (`-movflags` for mp4/720p only); `mp4` / `mov` add `-vf scale=trunc(iw/2)*2:trunc(ih/2)*2` and `720p` adds `-vf scale=-2:'trunc(min(720,ih)/2)*2'`, so both sides are even for libx264 + yuv420p (ruling F1, replaces the earlier `scale=-2:'min(720,ih)'`).
  - `webm`: `-c:v libvpx-vp9 -crf {40|32|24} -b:v 0 -row-mt 1 -c:a libopus -b:a {64k|96k|160k}`.
  - `gif`: `-vf fps={10|12|15},scale={320|480|'min(800,iw)'}:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse -an`.
  - `mp3`: `-vn -c:a libmp3lame -q:a {5|2|0}`.
  (`{small|normal|original}` order everywhere.)
- Progress: `percent = out_time_us / duration_us`, clamped 0–99 until exit 0; duration from `ffprobe -v error -show_entries format=duration -of csv=p=0 <src>`; unknown duration → no percent (indeterminate bar). `eta` seconds = elapsed × (1 − p) / p once p ≥ 3 %.
- Cancel: ✕ on a running **video** job kills ffmpeg, removes its temp output, drops the job without a toast. Image jobs stay non-cancellable while running.
- Card running state (video): `${percent}%` on the right of the line, foot `t.convert.remaining(eta)`.
- User strings only in `src/shared/i18n/{en,ko}.ts` (copy table in Task 5). Colours only in `globals.css`. Code/comments English.
- Done = the four checks pass and the app is checked on macOS (light/dark, ko/en); Windows run by the user.

## Review Focus

- Download interrupted (network drop, app quit, cancel) → no `engines/ffmpeg/` change, temp removed, status back to missing (or the old install). Test in Task 3 (`install_from` with a bad hash leaves the install dir untouched).
- SHA-256 mismatch → install refused with an error status, nothing moved in. Test in Task 3.
- A video with no audio track → `mp3` fails with `Engine` (toast), never a 0-byte file left in the folder. Test in Task 2 (`run` cleans the temp output on a failed engine) — covered by phase 1's `run` cleanup; extend to the video path.
- ffprobe can't read the duration (live stream, odd container) → job still runs with an indeterminate bar. Test in Task 2 (`parse_duration("N/A")` → None).
- Cancel pressed right as ffmpeg exits → no toast for a cancelled job, no crash on killing an exited child. Test in Task 2 (cancel flag checked after wait).

---

### Task 1: Video formats and slice availability

**Files:** Modify `src-tauri/src/domain/convert/{mod,model}.rs`, `src-tauri/src/app/convert.rs` (call sites), `src/domain/convert/apis/convert.dto.ts`, `src/app/convert/ConvertApp.tsx`, `src/shared/i18n/{en,ko}.ts` (`convert.needsFfmpeg`).

**Interfaces:**
- Produces:
  - `Format` gains `Mp4, Webm, Mov, Mp3, Mp4720` (serde names `mp4 webm mov mp3 720p`; `Mp4720.ext() == "mp4"`); `fn id(self) -> &'static str` = serde name; `Gif` is shared by image and video targets.
  - `pub enum Engine { Ffmpeg }` (serde lowercase); `pub struct Slice { pub format: Format, pub needs: Option<Engine> }` (Serialize camelCase).
  - `pub struct Caps { pub heic: bool, pub ffmpeg: bool }`; `formats_for(paths, caps: Caps) -> Vec<Slice>` (image rules unchanged, image slices always `needs: None`; video slices `needs: Some(Ffmpeg)` when `!caps.ffmpeg`).
  - `convert://open` payload becomes `{ slices: Slice[], name }`; `take_drop` matches `format.id()` and rejects a slice with `needs.is_some()`.
  - TS: `Format` adds `'mp4' | 'webm' | 'mov' | 'mp3' | '720p'`; `interface Slice { format: Format; needs: 'ffmpeg' | null }`.
  - Donut labels: `format === '720p' ? '720p' : format.toUpperCase()`; a slice with `needs` uses `opacity-40`, never `fill-primary`, and shows `t.convert.needsFfmpeg` under its label at 9px; `convertDrop` is not called for it.

- [ ] **Step 1: Failing tests** (`mod.rs`):

```rust
#[test]
fn video_sources_get_video_slices_without_their_own_container() {
    let s = formats_for(&[PathBuf::from("a.MOV")], Caps { heic: true, ffmpeg: true });
    let f: Vec<Format> = s.iter().map(|s| s.format).collect();
    assert_eq!(f, vec![Format::Mp4, Format::Webm, Format::Gif, Format::Mp3, Format::Mp4720]);
    assert!(s.iter().all(|s| s.needs.is_none()));
}
#[test]
fn mp4_keeps_720p() {
    let f: Vec<Format> = formats_for(&[PathBuf::from("a.m4v")], Caps { heic: true, ffmpeg: true }).iter().map(|s| s.format).collect();
    assert!(!f.contains(&Format::Mp4) && f.contains(&Format::Mp4720));
}
#[test]
fn video_slices_need_ffmpeg_when_missing() {
    let s = formats_for(&[PathBuf::from("a.mkv")], Caps { heic: true, ffmpeg: false });
    assert!(!s.is_empty() && s.iter().all(|s| s.needs == Some(Engine::Ffmpeg)));
}
#[test]
fn images_and_videos_together_are_unsupported() {
    assert!(formats_for(&[PathBuf::from("a.png"), PathBuf::from("b.mp4")], Caps { heic: true, ffmpeg: true }).is_empty());
}
#[test]
fn format_ids_are_unique_and_720p_writes_mp4() {
    assert_eq!(Format::Mp4720.id(), "720p");
    assert_eq!(Format::Mp4720.ext(), "mp4");
}
```
  Plus a `take_drop` test in `app/convert.rs`: a session whose slice needs FFmpeg rejects that format with `InvalidInput`.
- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib convert` → FAIL. **Step 3:** Implement (existing image tests keep passing with `Caps { heic, ffmpeg: false }`). Caps at `begin`: `heic: cfg!(target_os = "macos")`, `ffmpeg: engines::ffmpeg(app).is_some()` — until Task 3 lands, a private `fn ffmpeg_installed(app) -> bool { false }` stub in `convert.rs`, replaced in Task 3. **Step 4:** tests + `yarn typecheck` → PASS.
- [ ] **Step 5: Commit** `feat(convert): add video formats and dim slices that need FFmpeg`

### Task 2: FFmpeg run — args, progress, cancel

**Files:** Create `src-tauri/src/domain/convert/video.rs`; modify `src-tauri/src/domain/convert/{mod,model}.rs`, `src-tauri/src/domain/settings/{mod,model,commands}.rs` (`videoQuality`), `src/domain/settings/apis/settings.dto.ts`.

**Interfaces:**
- Produces:
  - `pub enum Quality { Small, Normal, Original }` + `Quality::parse(&str) -> Quality` (unknown → Normal).
  - `pub fn ffmpeg_args(src: &Path, to: Format, q: Quality, out: &Path) -> Vec<OsString>` — exactly the Global Constraints args.
  - `pub fn parse_duration(stdout: &str) -> Option<f64>` (seconds; `N/A`/empty → None).
  - `pub fn progress_of(line: &str, duration_s: f64) -> Option<f64>` (0.0–1.0 from an `out_time_us=` line; other lines None).
  - `pub struct Run<'a> { pub ffmpeg: Option<(&'a Path, &'a Path)> /* ffmpeg, ffprobe */, pub quality: Quality, pub cancel: &'a AtomicBool, pub progress: &'a mut dyn FnMut(Option<f64>) }`.
  - `run(src, stem, to, dir, tmp, ctx: &mut Run) -> Result<PathBuf, JobError>` — dispatches by the **source** (`pub fn is_video(path) -> bool`, the video extension list): image sources go to `convert_image` as before, video sources to `video::convert(...)` (so `Gif` from a video uses FFmpeg); a video source with `ctx.ffmpeg == None` → `Engine`; `JobError` gains `Cancelled` (never shown: the worker drops the job silently).
  - settings: `videoQuality` default `normal`, validate `small|normal|original`; DTO field; TS `videoQuality: 'small' | 'normal' | 'original'`, `SettingKey` adds it.

- [ ] **Step 1: Failing tests** (`video.rs`):

```rust
#[test]
fn mp4_normal_args() {
    let a = ffmpeg_args(Path::new("/i.mov"), Format::Mp4, Quality::Normal, Path::new("/o.mp4"));
    let a: Vec<&str> = a.iter().map(|s| s.to_str().unwrap()).collect();
    assert_eq!(&a[..6], ["-y", "-nostats", "-progress", "pipe:1", "-i", "/i.mov"]);
    for w in [["-crf", "23"], ["-preset", "medium"], ["-b:a", "128k"], ["-movflags", "+faststart"]] {
        assert!(a.windows(2).any(|p| p == w), "{w:?}");
    }
    assert_eq!(*a.last().unwrap(), "/o.mp4");
}
#[test]
fn p720_scales_down_only_and_mov_has_no_faststart() {
    let a = ffmpeg_args(Path::new("i"), Format::Mp4720, Quality::Small, Path::new("o"));
    assert!(a.iter().any(|s| s == "scale=-2:'trunc(min(720,ih)/2)*2'"));
    let m = ffmpeg_args(Path::new("i"), Format::Mov, Quality::Original, Path::new("o"));
    assert!(!m.iter().any(|s| s == "-movflags") && m.iter().any(|s| s == "18"));
}
#[test]
fn mp3_drops_video_and_gif_drops_audio() {
    assert!(ffmpeg_args(Path::new("i"), Format::Mp3, Quality::Normal, Path::new("o")).iter().any(|s| s == "-vn"));
    assert!(ffmpeg_args(Path::new("i"), Format::Gif, Quality::Normal, Path::new("o")).iter().any(|s| s == "-an"));
}
#[test]
fn durations_and_progress_lines() {
    assert_eq!(parse_duration("12.500000\n"), Some(12.5));
    assert_eq!(parse_duration("N/A\n"), None);
    assert_eq!(progress_of("out_time_us=6250000", 12.5), Some(0.5));
    assert_eq!(progress_of("frame=10", 12.5), None);
    assert_eq!(progress_of("out_time_us=99000000", 12.5), Some(1.0));
}
```
  Plus an `#[ignore]`d smoke test `converts_a_generated_clip` that, when `VEE_FFMPEG_DIR` is set, generates a 1 s test clip (`-f lavfi -i testsrc=duration=1:size=320x240:rate=10 -f lavfi -i sine=duration=1`) and converts it to every video target, asserting each output is non-empty; and a cancel case (flag set → `Err(Cancelled)`, no file in `dir`, temp removed).
- [ ] **Step 2:** FAIL. **Step 3:** Implement `video::convert`: probe duration (ffprobe), spawn ffmpeg with stdout piped, read lines → `progress(progress_of(..))`, poll `cancel` between lines and every 200 ms via `try_wait`; on cancel kill + wait; on non-zero exit `Engine`; GIF uses `-filter_complex`-free `-vf` with the split/palette chain as given. Worker callers pass `Run` (Task 4 wires the real values; until then `app/convert.rs` passes `ffmpeg: None`, `Quality::Normal`, a never-set flag and a no-op closure). **Step 4:** PASS (run the ignored smoke test once by hand with a local FFmpeg if available; report).
- [ ] **Step 5: Commit** `feat(convert): run FFmpeg with progress and cancel`

### Task 3: FFmpeg install — download, verify, unpack, remove

**Files:** Create `src-tauri/src/app/engines.rs`; modify `src-tauri/src/app/mod.rs`, `src-tauri/src/lib.rs` (manage state, commands), `src-tauri/src/app/convert.rs` (replace the Task 1 stub).

**Interfaces:**
- Produces:
  - `#[derive(Serialize)] #[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")] pub enum FfmpegStatus { Missing { error: Option<InstallError>, download_size: u64 }, Downloading { received: u64, total: u64, speed: u64 /* bytes/s */, stage: Stage }, Installed { version: String, size: u64, installed_at: i64, path: String, update: bool, error: Option<InstallError> } }` (ruling: no separate Failed state, so a failed update keeps the install info; `download_size` is the sum of this platform's pinned asset sizes, ruling F2), with `Stage { Download, Verify, Unpack }` and `InstallError { Network, Checksum, Unpack, Disk }` (camelCase).
  - `pub fn ffmpeg(app: &AppHandle) -> Option<(PathBuf, PathBuf)>` — both binaries present.
  - Pure, tested: `pub fn assets() -> &'static [Asset]` (`Asset { url, sha256, size, binaries: &[&str] /* names to find after unpack */ }`, per-platform consts from Global Constraints); `pub fn sha256_file(path) -> io::Result<String>` (lowercase hex); `pub fn install_from(archives: &[(PathBuf, &Asset)], unpack: impl Fn(&Path, &Path) -> io::Result<()>, dest: &Path, version: &str, now: i64) -> Result<(), InstallError>` (verify every hash → unpack each into a temp dir → find the binaries by file name anywhere below → `chmod 755` (unix) → swap into `dest` via rename of a sibling dir → write `meta.json`); `pub fn read_installed(dest) -> Option<Meta>`.
  - Shell: `download(app)` spawns `curl -fL --retry 2 -o <tmp file> <url>` per asset, a 250 ms poller sets `Downloading { received: file size, … }`, then `install_from` with `unpack` = `ditto -x -k` / `tar -xf <zip> -C <dir>`; macOS also `xattr -dr com.apple.quarantine <dest>` (ignore failure). One download at a time.
  - Commands (in `engines.rs` `commands` submodule, registered in `lib.rs`): `get_ffmpeg_status() -> FfmpegStatus`, `download_ffmpeg()`, `cancel_ffmpeg_download()` (kills curl, removes temp, status → previous), `remove_ffmpeg()` (deletes `dest`). Event `engines://ffmpeg` with the status to every window on change (throttled to the 250 ms poll).
  - `update` in `Installed` = installed version ≠ pinned `FFMPEG_VERSION`; "업데이트" re-runs `download_ffmpeg`.

- [ ] **Step 1: Failing tests**: `install_from` with a fake archive whose hash matches (use a temp zip-free "archive": the test `unpack` closure copies a fixture dir with `ffmpeg`/`ffprobe` files) → `dest` has both + `meta.json` with the version; with a wrong hash → `Err(Checksum)` and `dest` untouched (pre-existing file inside stays); with an unpack that yields no binaries → `Err(Unpack)`, `dest` untouched; `sha256_file` of `b"abc"` = `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`; `read_installed` on a dir missing `ffprobe` → None.
- [ ] **Step 2:** FAIL. **Step 3:** Implement. **Step 4:** PASS; run `download_ffmpeg` once by hand on macOS through the app (Task 6) — not in unit tests.
- [ ] **Step 5: Commit** `feat(convert): download and install FFmpeg from settings`

### Task 4: Worker — video jobs, percent, ETA, cancel

**Files:** Modify `src-tauri/src/domain/convert/{mod,model}.rs` (`Job`/`JobView`), `src-tauri/src/app/convert.rs`, `src/domain/convert/apis/{convert.ts,convert.dto.ts}`.

**Interfaces:**
- Produces:
  - `Job` gains `progress: Option<f64>` (current file), `started: Option<i64>`, `cancel: Arc<AtomicBool>`; `JobView` gains `percent: Option<u8>` (overall: `(done + progress) / total`, `None` when the running file has no progress) and `eta: Option<u32>` (seconds, rule from Global Constraints, `None` below 3 %), `cancellable: bool` (running video job).
  - Pure `fn eta(elapsed_ms: i64, p: f64) -> Option<u32>` in `model.rs`, tested (`eta(10_000, 0.5) == Some(10)`, `eta(1_000, 0.02) == None`).
  - Worker passes `Run { ffmpeg: engines::ffmpeg(app), quality: Quality::parse(settings videoQuality), cancel: &job.cancel, progress: |p| store in job }`; `Err(Cancelled)` → remove the job, publish, no toast.
  - Command `cancel_convert(id: u64)` → sets the flag of a running video job (else `InvalidInput`); TS `cancelConvert(id)`.

- [ ] **Steps:** failing `eta` + `visible_jobs` percent tests → implement → PASS → **Commit** `feat(convert): show video progress and cancel running jobs`

### Task 5: Settings FFmpeg section, card percent/ETA, i18n

**Files:** Create `src/domain/convert/queries/engines.tsx` (`useFfmpegStatus`: seeds from `getFfmpegStatus`, replaced by `engines://ffmpeg`); modify `src/app/settings/_components/ConvertSection.tsx`, `src/app/panel/_components/Card.tsx`, `src/domain/convert/apis/convert.ts` (`getFfmpegStatus`, `downloadFfmpeg`, `cancelFfmpegDownload`, `removeFfmpeg`), `src/domain/convert/apis/convert.dto.ts` (`FfmpegStatus` union mirroring Task 3), `src/shared/i18n/{en,ko}.ts`.

- [ ] **Step 1: Settings** (mockup §4 "FFmpeg · 영상 변환", below the phase-1 section, its own `SettingSection` with a small caption `c.ffmpeg.caption`):
  - `missing` → `Row` `c.ffmpeg.title` / hint `c.ffmpeg.missingHint(size MB)`, `Button` `c.ffmpeg.download`.
  - `downloading` → `Row col` with title + hint `c.ffmpeg.progress(receivedMB, totalMB, speedMB)` (stage `verify`/`unpack` → `c.ffmpeg.verifying` / `c.ffmpeg.unpacking`), `Button variant="gray"` `c.cancel`, and a 6px bar (`bg-subtle` track, `bg-primary` fill, width = received/total).
  - `installed` → title + ok badge `c.ffmpeg.installed`, `Button variant="gray"` `c.ffmpeg.update` only when `update`, `Button variant="danger"` `c.ffmpeg.remove` (through the existing `ConfirmModal` with `c.ffmpeg.removeConfirm`); a meta grid (version, size MB, installed date via `toLocaleDateString(locale)`, path in mono 11px, `break-all`); then `Row` `c.ffmpeg.quality` with `Segmented` small/normal/original → `updateSetting('videoQuality')`.
  - any state with `error` → an error line `c.ffmpeg.errors[error]` in `text-error` under the row (missing or installed).
- [ ] **Step 2: Card**: running job with `percent !== null` → right side `${percent}%`, bar width `percent%`; foot `t.convert.remaining(eta)` when `eta !== null`; ✕ shown when `cancellable` → `cancelConvert(id)` (`aria-label={t.convert.cancel}`).
- [ ] **Step 3: i18n** — add exactly (both dicts):

| key | en | ko |
| --- | --- | --- |
| `convert.needsFfmpeg` | Needs FFmpeg | FFmpeg 필요 |
| `convert.remaining(s)` | `` `About ${s}s left` `` (≥ 60 → `` `About ${Math.round(s / 60)} min left` ``) | `` `남은 시간 약 ${s}초` `` (≥ 60 → `` `남은 시간 약 ${Math.round(s / 60)}분` ``) |
| `convert.cancel` | Cancel | 취소 |
| `settings.convert.ffmpeg.caption` | FFmpeg · Video conversion | FFmpeg · 영상 변환 |
| `settings.convert.ffmpeg.title` | FFmpeg | FFmpeg |
| `settings.convert.ffmpeg.missingHint(mb)` | `` `Needed to convert video and audio. About ${mb} MB` `` | `` `영상·소리 변환에 필요해요. 약 ${mb}MB` `` |
| `settings.convert.ffmpeg.download` | Download | 다운로드 |
| `settings.convert.ffmpeg.progress(r, t, s)` | `` `Downloading · ${r} / ${t} MB · ${s} MB/s` `` | `` `내려받는 중 · ${r} / ${t} MB · ${s} MB/s` `` |
| `settings.convert.ffmpeg.verifying` | Checking the download… | 내려받은 파일 확인 중… |
| `settings.convert.ffmpeg.unpacking` | Unpacking… | 압축 푸는 중… |
| `settings.convert.ffmpeg.installed` | Installed | 설치됨 |
| `settings.convert.ffmpeg.update` | Update | 업데이트 |
| `settings.convert.ffmpeg.remove` | Remove | 삭제 |
| `settings.convert.ffmpeg.removeConfirm` | Remove FFmpeg? Video conversion stops until you download it again. | FFmpeg를 삭제할까요? 다시 내려받기 전까지 영상 변환을 할 수 없어요. |
| `settings.convert.ffmpeg.meta` | `{ version: 'Version', size: 'Size', installedAt: 'Installed', path: 'Location' }` | `{ version: '버전', size: '크기', installedAt: '설치일', path: '위치' }` |
| `settings.convert.ffmpeg.quality` | Video quality | 영상 기본 화질 |
| `settings.convert.ffmpeg.qualityHint` | Used for MP4, WEBM, MOV and GIF | MP4 · WEBM · MOV · GIF 변환에 쓰여요 |
| `settings.convert.ffmpeg.qualities` | `{ small: 'Small', normal: 'Normal', original: 'Best' }` | `{ small: '작게', normal: '보통', original: '원본' }` |
| `settings.convert.ffmpeg.errors` | `{ network: "Couldn't download. Check the connection.", checksum: 'The download was damaged. Try again.', unpack: "Couldn't unpack the download.", disk: "Couldn't save FFmpeg on this Mac/PC." }` | `{ network: '내려받지 못했어요. 연결을 확인해 주세요.', checksum: '내려받은 파일이 손상됐어요. 다시 시도해 주세요.', unpack: '압축을 풀지 못했어요.', disk: 'FFmpeg를 저장하지 못했어요.' }` |

- [ ] **Step 4:** `yarn test && yarn typecheck && yarn lint` → PASS. **Commit** `feat(convert): manage FFmpeg in settings and show video progress`

### Task 6: Docs, checks and the running app

**Files:** `docs/agents/{overview,architecture,structure,configuration}.md`, `docs/agents/conventions/tauri-commands.md` (event `engines://ffmpeg`, new commands), `README.md` (FFmpeg download note).

- [ ] **Step 1:** Update docs (overview: network is used for update checks **and the FFmpeg download the user starts**).
- [ ] **Step 2:** Four checks pass.
- [ ] **Step 3 (with the user):** macOS `yarn tauri dev --config '{"identifier":"com.bobpark.vee.dev"}'`: download FFmpeg (progress, cancel mid-way, download again, installed meta, remove, download), drag a video card → dimmed slices before install, after install MP4/WEBM/GIF/MP3/720p land in Desktop and a Finder folder, percent + remaining time on the card for a ≥ 10 s clip, cancel mid-way (no file, no toast), quality setting changes output size. Light/dark, ko/en. Windows: the user repeats download + one MP4 + cancel.
- [ ] **Step 4: Commit** `docs: document FFmpeg download and video conversion`
