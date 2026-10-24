# Engines Download Their Latest Release — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** FFmpeg and rhwp are no longer pinned: opening Settings → Convert looks up each engine's latest release, shows "Update" when the install differs, and Download/Update fetches that release (SHA-256 checked against the publisher's own checksum). Plus: LibreOffice "Not found" becomes a badge.

**Architecture:** `app/engines.rs` drops the pinned `SPECS` assets for a per-platform resolver that shells out to `curl` (no new crates) and parses the answer with pure, unit-tested functions. Each `Slot` keeps the last resolved `Release`; `on_disk` derives `update` / `downloadSize` from it. A new async command `check_engine_updates` runs the resolvers and re-emits `engines://status`; the Convert tab calls it on mount.

**Tech Stack:** Rust (Tauri 2, `serde_json`, system `curl`), React 19 + React Query, Tailwind 4/daisyUI 5.

**Spec:** Design agreed in chat on 2026-10-24 (no spec file): check on every Convert-tab mount; silent on failure; size blank until known; checksum from the source, not pinned; no new dependencies; README/docs privacy line updated.

**Already on the branch (`feature/ffmpeg-9-and-paste-tools`, uncommitted):** FFmpeg moved to martin-riedl.de (macOS) / GyanD (Windows) 9.0.2 archives; `unpack_into` accepts archives holding a subset of the binaries and `stage` checks all arrived; Windows LibreOffice version read from `program\version.ini` (`MsiProductVersion`). Commit these first (Task 0).

## Global Constraints

- No dependency changes in `Cargo.toml` / `package.json`.
- Sources (exact):
  - rhwp: `https://api.github.com/repos/edwardkim/rhwp/releases/latest`; tag `v<ver>`; asset `rhwp-v<ver>-macos-aarch64.tar.gz` / `rhwp-v<ver>-windows-x86_64.zip`.
  - FFmpeg Windows: `https://api.github.com/repos/GyanD/codexffmpeg/releases?per_page=30`; first release whose `tag_name` is digits and dots only (`9.0.2`; skip `2026-10-22-git-…`); asset `ffmpeg-<tag>-essentials_build.zip`.
  - FFmpeg macOS: `https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/{ffmpeg,ffprobe}.zip` redirects to `…/download/macos/arm64/<id>_<ver>/<name>.zip`; checksum at `<that url>.sha256` (`<64 hex>  ffmpeg.zip`); size from `content-length` of a HEAD on that url.
- GitHub checksum is the asset's `digest` field `sha256:<64 hex>`; an asset without it is rejected.
- A resolved `Release` is accepted only if: version is digits/dots, every url starts with `https://github.com/` or `https://ffmpeg.martin-riedl.de/download/`, every sha256 is 64 lowercase hex.
- Every `curl` goes through `domain::convert::command` (no console window) with `-sf --connect-timeout 10 --max-time 20`.
- User-visible text only in `src/shared/i18n/{en,ko}.ts`.

## Review Focus

- Offline when the tab opens → no error text, installed engine still "Installed" without Update; Missing shows the hint without a size. (Task 2 test: `on_disk` with `latest: None`.)
- Download pressed before any check finished (or the check failed) → resolve inline; if that fails too, status shows the `network` error. (Task 2 test.)
- Check finishing while a download runs → status stays `Downloading`; only `latest` is stored. (Task 2 test via a `downloading` guard in `apply_latest`.)
- A GitHub API answer missing `digest`, or a dev tag newest in GyanD → rejected / skipped. (Task 1 tests.)
- Rate limit (GitHub 403) → treated as failure, silent. (Covered by `-f` + Task 1 `None` tests on empty input.)

---

### Task 0: Commit the work already on the branch

- [ ] **Step 1:** `cargo test --manifest-path src-tauri/Cargo.toml --lib engines office` → PASS.
- [ ] **Step 2:** Commit `src-tauri/src/app/{engines,office}.rs`:
  `fix(convert): read LibreOffice's version without opening a console on Windows` (office.rs) and `feat(convert): move FFmpeg to 9.0.2` (engines.rs) — two commits.

### Task 1: Release resolvers (pure parsers + curl glue)

**Files:** Modify `src-tauri/src/app/engines.rs` (replace `AssetKind`, `Asset`, `FFMPEG_ASSETS`, `RHWP_ASSETS`, `EngineSpec.version/assets`).

**Interfaces — Produces:**
- `pub struct Asset { pub url: String, pub sha256: String, pub size: u64 }` (every asset is an archive; `AssetKind` deleted).
- `#[derive(Debug, Clone, PartialEq)] pub struct Release { pub version: String, pub assets: Vec<Asset> }` with `fn size(&self) -> u64` (sum) and `fn valid(&self) -> bool` (Global Constraints rule).
- `EngineSpec { dir_name, binaries }` only.
- `fn github_asset(release: &serde_json::Value, name: &str) -> Option<Asset>` — finds `assets[].name == name`, takes `browser_download_url`, `size`, `digest` stripped of `sha256:`.
- `fn parse_rhwp(json: &str, platform: &str) -> Option<Release>` — `platform` is `"macos-aarch64.tar.gz"` / `"windows-x86_64.zip"`.
- `fn parse_gyan(json: &str) -> Option<Release>`.
- `fn parse_riedl_url(url: &str) -> Option<String>` → version from `…/<id>_<ver>/<name>.zip`.
- `fn parse_sha256_file(text: &str) -> Option<String>`; `fn parse_content_length(headers: &str) -> Option<u64>` (last `content-length:` line, case-insensitive).
- `fn resolve(id: EngineId) -> Option<Release>` — per-platform `#[cfg]`; curl helpers `fn curl_text(args: &[&str]) -> Option<String>`; result filtered by `valid()`. Linux: `None`.

- [ ] **Step 1: Failing tests** (fixtures inline, trimmed real JSON):

```rust
#[test] fn rhwp_latest_release_from_the_github_api() {
    let json = r#"{"tag_name":"v0.8.7","assets":[{"name":"rhwp-v0.8.7-macos-aarch64.tar.gz","size":12941874,
      "browser_download_url":"https://github.com/edwardkim/rhwp/releases/download/v0.8.7/rhwp-v0.8.7-macos-aarch64.tar.gz",
      "digest":"sha256:6ed46aa63f6d10c447842999f73abc59cf9586c5645fefc5e763bd6a6d43662d"}]}"#;
    let r = parse_rhwp(json, "macos-aarch64.tar.gz").unwrap();
    assert_eq!((r.version.as_str(), r.size()), ("0.8.7", 12_941_874));
    assert!(r.valid());
    assert_eq!(parse_rhwp(&json.replace(r#","digest":"sha256:6ed46aa63f6d10c447842999f73abc59cf9586c5645fefc5e763bd6a6d43662d""#, ""), "macos-aarch64.tar.gz"), None);
    assert_eq!(parse_rhwp(json, "windows-x86_64.zip"), None);
    assert_eq!(parse_rhwp("", "macos-aarch64.tar.gz"), None);
}
#[test] fn gyan_skips_git_builds_for_the_newest_release_version() { /* releases [2026-10-22-git-bf0e831606, 9.0.2, 9.0.1] → version "9.0.2", url ends "ffmpeg-9.0.2-essentials_build.zip" */ }
#[test] fn riedl_redirect_and_checksum_files() {
    assert_eq!(parse_riedl_url("https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/ffmpeg.zip").as_deref(), Some("9.0.2"));
    assert_eq!(parse_riedl_url("https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffmpeg.zip"), None);
    assert_eq!(parse_sha256_file("c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924  ffmpeg.zip\n").as_deref(), Some("c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924"));
    assert_eq!(parse_content_length("HTTP/2 302\r\ncontent-length: 19\r\n\r\nHTTP/2 200\r\nContent-Length: 28395699\r\n"), Some(28_395_699));
}
#[test] fn a_release_from_elsewhere_or_with_a_bad_version_is_invalid() { /* http:// url, "9.0-dev", short sha → !valid() */ }
```

- [ ] **Step 2:** `cargo test … --lib engines` → FAIL (not defined).
- [ ] **Step 3:** Implement. macOS `resolve(Ffmpeg)`: for each of `ffmpeg`, `ffprobe`: `curl -sfI -o /dev/null -w %{redirect_url} <latest>` → url; `parse_riedl_url`; `curl -sf <url>.sha256`; `curl -sfI <url>` → size; both versions must match. Order of `assets` = `binaries` order.
- [ ] **Step 4:** Rename ignored `installs_pinned_ffmpeg`/`installs_pinned_rhwp` → `installs_latest_ffmpeg`/`installs_latest_rhwp` (resolve, then the existing download+install). Run `cargo test … --lib engines` → PASS (ignored ones skipped); delete `pinned_assets_cover_the_binaries`, rewrite `download_size_sums_the_pinned_assets` as part of Task 2.
- [ ] **Step 5:** Commit `feat(convert): resolve FFmpeg and rhwp's latest release`.

### Task 2: Status from the latest release, check command, download uses it

**Files:** Modify `src-tauri/src/app/engines.rs`, `src-tauri/src/lib.rs` (register command), `src/domain/convert/apis/convert.dto.ts`, `src/domain/convert/apis/convert.ts`.

**Interfaces:**
- Consumes: `Release`, `resolve` (Task 1).
- Produces:
  - `EngineStatus::Missing { error, download_size: Option<u64> }` (TS `downloadSize: number | null`).
  - `pub fn on_disk(spec: &EngineSpec, dir: &Path, latest: Option<&Release>, error: Option<InstallError>) -> EngineStatus` — `update = latest.is_some_and(|r| r.version != m.version)`; `download_size = latest.map(Release::size)`.
  - `Slot.latest: Mutex<Option<Release>>`.
  - `fn apply_latest(app, id, release: Option<Release>)` — stores it when `Some` (keeps the old one on failure); re-emits `on_disk(…, None)` unless a download is running (`cancel` slot is `Some`).
  - `#[tauri::command(async)] pub fn check_engine_updates(app: AppHandle)` — resolves both engines on two threads, `apply_latest` each.
  - `download`: release = `latest.clone()` or `resolve(id)` on the download thread (stored on success); `None` → final status with `Some(InstallError::Network)`. `fetch_and_install(app, id, &release, cancel)`; `install_from(…, &release.version, …)`; `total = release.size()`.
  - TS: `checkEngineUpdates(): Promise<void>` → `call<void>('check_engine_updates')`.

- [ ] **Step 1: Failing tests:**

```rust
#[test] fn update_and_size_come_from_the_latest_release() {
    // installed 9.0.1 in a temp dir; latest Some("9.0.2", size 10) → Installed { update: true }
    // latest Some("9.0.1") → update false; latest None → update false
    // empty dir: latest None → Missing { download_size: None }; Some(size 10) → Missing { download_size: Some(10) }
}
```

- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement; update every `on_disk` caller (`manage` passes `None`). **Step 4:** `cargo test … --lib` → PASS; `cargo clippy … --lib --tests` no new warnings.
- [ ] **Step 5:** Commit `feat(convert): check the engines' latest release from the Convert tab`.

### Task 3: Convert tab — run the check on mount, size-less hint, LibreOffice badge

**Files:** Modify `src/domain/convert/queries/engines.tsx`, `src/app/settings/_components/ConvertSection.tsx`, `src/app/settings/_components/badge.ts`, `src/shared/i18n/{en,ko}.ts`.

**Interfaces:** Consumes `checkEngineUpdates` (Task 2).
- `export function useEngineUpdateCheck(): void` in `queries/engines.tsx` — `useEffect(() => void checkEngineUpdates().catch(() => {}), [])`; called once at the top of `ConvertSection`.
- `missingHint: (mb: number | null) => string` — ko: `mb ? \`영상·소리 변환에 필요해요. 약 ${mb}MB\` : '영상·소리 변환에 필요해요.'` (rhwp the same pattern); en mirrors. Caller passes `status.downloadSize === null ? null : Math.round(status.downloadSize / MB)`.
- `export const BADGE_OFF = 'bg-badge-bg text-badge-fg shrink-0 rounded-md px-2 py-0.5 text-[11px] font-semibold whitespace-nowrap';` — LibreOffice not found renders `<span className={BADGE_OFF}>{l.missing}</span>` in place of the muted text (button stays).

- [ ] **Step 1:** Implement. **Step 2:** `yarn test && yarn typecheck && yarn lint` → PASS.
- [ ] **Step 3:** `yarn tauri dev` (own identifier if Vee is installed, see `docs/agents/build-run.md`): Convert tab in light/dark × ko/en — Update button appears for an older install (edit `engines/ffmpeg/meta.json` version to `9.0.1` in the dev data dir), Missing hint shows the size after the check, LibreOffice badge renders.
- [ ] **Step 4:** Commit `feat(settings): show LibreOffice not found as a badge` (badge) and fold the hook/hint into Task 2's area as `feat(convert): …` if preferred.

### Task 4: Docs

**Files:** `README.md` (lines on FFmpeg source, network use), `docs/agents/{architecture,configuration,overview,testing,build-run}.md`, `docs/development.md` if it names shaka/pins.

- [ ] **Step 1:** Replace "shaka-project … 8.1.2 / pinned" with the three sources and "latest release, checked on opening the Convert tab, SHA-256 from the publisher"; README privacy line: Vee also connects when the Convert tab opens to check FFmpeg/rhwp versions. Test names in `testing.md` → `installs_latest_*`.
- [ ] **Step 2:** `git grep -n "8\.1\.2\|shaka\|pinned" -- docs README.md` → only history files under `docs/superpowers/`.
- [ ] **Step 3:** Commit `docs: engines download their latest release`.

### Task 5: Verify

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` → all PASS.
- [ ] `cargo test … --lib installs_latest -- --ignored --nocapture` on macOS → both install.
- [ ] Windows (user): Convert tab opens with no console window; FFmpeg/rhwp download + one video and one HWP conversion.
