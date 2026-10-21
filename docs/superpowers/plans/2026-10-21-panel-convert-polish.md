# Panel & Convert Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the donut afterimage and the panel flash, shorten the copy debounce, add a Permissions settings tab, pinned-card copy shortcuts, LibreOffice-only document conversion (except HWP), and visible hardware-encoder diagnostics.

**Architecture:** Seven independent changes on one branch. Each is a thin edit to an existing module; no new domain. Rust owns window/encoder state; the frontend reads it through `call()` wrappers and React Query, refreshed by events.

**Tech Stack:** Tauri 2, Rust, React 19 + TypeScript, React Query, Tailwind 4 + daisyUI 5, `node:test`.

**Spec:** `docs/superpowers/specs/2026-10-21-panel-convert-polish-design.md` (Korean). Read it first; the "현재 코드에서 확인한 것" section has the line references used below.

## Global Constraints

- Branch `feature/panel-convert-polish`. Commit messages `<prefix>: <subject>` per `docs/agents/workflow/commit-messages.md`, ending with the session's attribution lines.
- No dependency changes in `package.json` / `src-tauri/Cargo.toml` (including new `windows` crate features) without asking the user first.
- Every user-visible string goes in `src/shared/i18n/{en,ko}.ts`. Code and comments in English.
- Follow `docs/agents/conventions/*` and `docs/agents/libs/*` (thin Tauri commands, `call()` wrappers, section comments in components, colors only from `globals.css` tokens).
- Do not edit shipped migrations. No migration is needed (settings are key/value rows).
- Done means: `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` all pass, and UI changes are checked in `yarn tauri dev` in light/dark × ko/en (`docs/agents/build-run.md` for running next to an installed Vee).
- HWP / HWPX stay on rhwp. Convert key stays a Segmented of `alt | ctrl | shift`.

## Review Focus

- **Re-opening the donut right after closing it** (key up, key down within ~50 ms): the late hide from the first close must not hide the second open. → Task 2 test `stale_hide_is_ignored`.
- **Releasing the mouse over the blank-but-still-visible overlay** after key-up: must count as a normal drop (copy), not a convert. → Task 2 test `closed_overlay_is_not_a_drop_target`.
- **Pinned-copy combo pressed with fewer pinned cards than its number**: nothing happens, nothing is typed into search. → Task 5 test `pinned_copy_without_card_still_swallows_key` (the action is returned; `PanelApp` ignores a missing card).
- **Pinned-copy combo that equals another panel shortcut** (e.g. user sets `CommandOrControl+KeyK`): the recorder refuses it via `taken`, and Rust `validate` still accepts only the `+`-joined form. → Task 5 Rust test.
- **`txt` + `rtf` dragged together** used to be "unsupported"; after merging families it must offer the shared targets. → Task 6 test `plain_and_rich_text_now_share_a_family`.

---

### Task 1: Copy debounce 0.3 s

**Files:**
- Modify: `src-tauri/src/app/watcher.rs:121`
- Modify: `src-tauri/src/app/state.rs:35`, `src-tauri/src/app/state.rs:55` (comments only)

- [ ] **Step 1:** Set `const QUIET: Duration = Duration::from_millis(300);`. Update the two comments that say "0.5s" to "0.3s". Leave the 1200 ms own-write window as is.
- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib app::watcher app::state` → PASS.
- [ ] **Step 3:** Commit `perf(watcher): record a copy after 0.3s of quiet`.

---

### Task 2: Donut afterimage — blank the overlay before hiding it

The hidden webview shows its last painted frame on the next `show()`. Make that frame empty: on close the frontend renders nothing, waits two animation frames, then asks Rust to hide.

**Files:**
- Modify: `src-tauri/src/app/convert.rs` (`Convert` state, `show`, `close`, `dropped_on_overlay`, `commands`)
- Modify: `src-tauri/src/lib.rs` (register command)
- Modify: `src/app/convert/ConvertApp.tsx`
- Modify: `src/domain/convert/apis/convert.ts`
- Modify: `src-tauri/capabilities/*` only if commands are allow-listed per window (check how `convert_drop` is allowed and mirror it).

**Interfaces:**
- Produces (Rust, `app/convert.rs`):
  - `Convert.overlay: Mutex<Overlay>` with `struct Overlay { seq: u64, open: bool }` (default `{0, false}`).
  - Pure helpers, unit-tested:
    - `fn opened(o: &mut Overlay) -> u64` — `seq += 1; open = true;` returns new seq.
    - `fn closed(o: &mut Overlay) -> Option<u64>` — if `open`: `open = false`, returns `Some(seq)`; else `None`.
    - `fn may_hide(o: &Overlay, seq: u64) -> bool` — `!o.open && o.seq == seq`.
  - Event `convert://close` payload changes from `()` to `{ seq: u64 }`.
  - Command `hide_convert(app: AppHandle, seq: u64)` — on the main thread, hides the overlay if `may_hide`.
  - `const HIDE_FALLBACK_MS: u64 = 200;` — `close` also schedules the same hide after this delay.
- Produces (TS): `hideConvert(seq: number): Promise<void>` in `src/domain/convert/apis/convert.ts`.

- [ ] **Step 1: Write failing tests** in `app/convert.rs` `mod tests`:
  - `stale_hide_is_ignored`: `opened` → `closed` gives `Some(1)`; `opened` again; `may_hide(&o, 1)` is `false`.
  - `hide_after_close_is_allowed`: `opened`, `closed` → `may_hide(&o, 1)` is `true`; a second `closed` returns `None`.
  - `closed_overlay_is_not_a_drop_target`: after `closed`, `o.open` is `false` (this is the flag `dropped_on_overlay` now checks instead of `is_visible()`).
- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml --lib app::convert` → FAIL (helpers missing).
- [ ] **Step 3: Implement.**
  - `show()`: call `opened` (inside the main-thread closure, after the `live` check) before emit + `overlay.show()`.
  - `close()`: `if let Some(seq) = closed(..)` emit `convert://close { seq }` and spawn the fallback (`sleep(HIDE_FALLBACK_MS)` then `run_on_main_thread` → same hide path). If already closed, do nothing. No immediate `overlay.hide()`.
  - `dropped_on_overlay`: replace the `is_visible()` check with the `open` flag read **before** calling `close`.
  - `lib.rs` startup hide stays as is (nothing painted yet).
  - `ConvertApp.tsx`: on `convert://close` → `setOpen(null)`, `setHot(null)`, then `requestAnimationFrame(() => requestAnimationFrame(() => void hideConvert(seq).catch(() => undefined)))`. Keep `slicesRef` untouched (the late drop event still reads it). Update the comment at the listener to say why.
- [ ] **Step 4:** Run the Rust tests → PASS. `yarn typecheck` → PASS.
- [ ] **Step 5: Verify in the app (macOS):** `yarn tauri dev`. Drag an image card, hold the convert key (donut shows image formats), release key, then drag an `.hwp` file card and hold the key → the first frame is the HWP donut, no image donut flash. Repeat key down/up quickly several times → donut never stays stuck invisible or visible. Releasing the mouse right after key-up copies normally.
- [ ] **Step 6:** Commit `fix(convert): blank the donut before hiding it so the next open has no afterimage`. Tell the user Windows needs a check on their PC.

---

### Task 3: Panel flash after a cancelled or unconverted drag (macOS)

**Files:**
- Modify: `src-tauri/src/app/window.rs` (`end_drag`, `finish_drag`), `src-tauri/src/lib.rs:81-87`, `src-tauri/src/app/state.rs` (if a field is added)

- [ ] **Step 1: Reproduce first** with `superpowers:systematic-debugging`. Add temporary `log::info!` lines in `finish_drag` (result, `on_overlay`, pinned) and in the `Focused(false)` handler in `lib.rs` (with `now_ms()` and `dragging`). In `yarn tauri dev` repeat: drag a card, press and release the convert key, drop on desktop; drag and press Esc; drag back onto the panel. Note which sequence logs a `Focused(false)` → `hide_panel` right after a `Cancel` re-show.
- [ ] **Step 2: Fix according to the finding.** Expected cause (spec): a focus-loss queued during the drag is handled after `end_drag` cleared `dragging`. Expected fix: `AppState.drag_ended_ms: AtomicU64` set in `end_drag`, and the `Focused(false)` branch skips the hide when `within_grace(now_ms(), ended, DRAG_FOCUS_GRACE_MS)` with `const DRAG_FOCUS_GRACE_MS: u64 = 300;`. If the logs show a different cause (e.g. `set_focus` not sticking), fix that instead and write down why in the commit body. The outcome must be: Cancel → panel slides back and stays; Dropped (not pinned) → panel closes once.
- [ ] **Step 3:** If `within_grace(now: u64, ended: u64, grace: u64) -> bool` is added, unit-test it in `state.rs` or `window.rs`: `within_grace(1000, 900, 300)` true, `within_grace(1300, 900, 300)` false, `within_grace(900, 0, 300)` false when `ended == 0` (never dragged).
- [ ] **Step 4:** Remove the temporary logs. Run `cargo test … --lib` → PASS. Re-run the Step 1 sequences 10× each → no flash.
- [ ] **Step 5:** Commit `fix(panel): stop the panel flashing after a cancelled drag`.

---

### Task 4: Permissions tab with badges

**Files:**
- Modify: `src/app/settings/_components/Tabs.tsx` (type + `TABS` entry with an icon, e.g. a shield path `M12 3l7 3v6c0 4.5-3 8-7 9-4-1-7-4.5-7-9V6l7-3Z`)
- Create: `src/app/settings/_components/PermissionsSection.tsx`
- Create: `src/app/settings/_components/badge.ts` exporting `BADGE` (moved from `ConvertSection.tsx:29`)
- Modify: `src/app/settings/SettingsApp.tsx` (remove `DiskAccessRow` from history, render permissions tab), `src/app/settings/_components/ConvertSection.tsx` (remove Finder row, import `BADGE`)
- Modify: `src/shared/i18n/{en,ko}.ts`

**Interfaces:**
- `SettingsTab` gains `'permissions'`, placed between `'convert'` and `'history'` in `TABS`.
- `PermissionsSection({ settings, run }: { settings: Settings; run: (p: Promise<unknown>) => void })`.
- i18n: `settings.tabs.permissions` = `'권한'` / `'Permissions'`.

- [ ] **Step 1: Implement.** `PermissionsSection` renders one `SettingSection` with the Full Disk Access row (`useDiskAccess`, granted → `<span className={BADGE}>{s.diskAccessGranted}</span>`, else the existing button) and the Finder row moved verbatim from `ConvertSection` (keeps `dim={settings.dragConvert !== 'on'}`). It returns `null` when both `granted === null` and `!access`.
- [ ] **Step 2:** Hide the tab where it would be empty: `Tabs` takes an optional `hidden?: SettingsTab[]`; `SettingsApp` passes `['permissions']` when `!isMac` (same `navigator.userAgent.includes('Mac')` check `ConvertSection` uses). Arrow-key navigation iterates the visible tabs only.
- [ ] **Step 3:** Run `yarn typecheck && yarn lint && yarn test` → PASS.
- [ ] **Step 4: Verify in the app:** Permissions tab shows both rows with green badges when granted; History tab no longer has the disk row; Convert tab no longer has the Finder row. Light/dark × ko/en.
- [ ] **Step 5:** Commit `feat(settings): move macOS permissions into their own tab`.

---

### Task 5: Pinned-card copy shortcuts

**Files:**
- Modify: `src-tauri/src/domain/settings/model.rs`, `mod.rs` (defaults, validate, tests), `commands.rs` (DTO fill)
- Modify: `src/domain/settings/apis/settings.dto.ts`
- Modify: `src/utils/keys.ts`, `src/utils/keys.test.ts`
- Modify: `src/app/panel/PanelApp.tsx` (`onKeyDown`)
- Modify: `src/app/settings/SettingsApp.tsx` (shortcuts tab)
- Modify: `src/shared/i18n/{en,ko}.ts`

**Interfaces:**
- Setting keys `pinnedCopy1Shortcut`, `pinnedCopy2Shortcut`, `pinnedCopy3Shortcut`; defaults `CommandOrControl+Digit1` / `Digit2` / `Digit3` as `DEFAULT_PINNED_COPY_SHORTCUTS: [&str; 3]` in `model.rs`.
- `SettingsDto` fields `pinned_copy1_shortcut`, `pinned_copy2_shortcut`, `pinned_copy3_shortcut` (camelCase `pinnedCopy1Shortcut` …). TS `Settings` and `SettingKey` gain the same.
- `KeyInput.pinnedCopy: 0 | 1 | 2 | null` — index of the matching combo. `KeyAction` gains `{ type: 'copyPinned'; index: 0 | 1 | 2 }`.
- i18n `settings.pinnedCopyShortcut` = `'고정 카드 {n} 복사'` / `'Copy pinned card {n}'` (or three keys if the i18n helper has no interpolation — check `docs/agents/libs/i18n.md`), hint `'패널이 열려 있을 때 {n}번째 고정 카드를 복사해요'` / `'Copies the nth pinned card while the panel is open'`.

- [ ] **Step 1: Failing Rust test** in `settings/mod.rs` tests: `validate("pinnedCopy1Shortcut", "CommandOrControl+Digit1")` ok, `"Digit1"` err, `""` err; `get` on an empty DB returns `CommandOrControl+Digit2` for `pinnedCopy2Shortcut`.
- [ ] **Step 2: Failing TS tests** in `keys.test.ts`:
  - `pinned_copy_combo_copies_that_card`: `pinnedCopy: 1` → `{ type: 'copyPinned', index: 1 }`; with `repeat: true` → `null`.
  - `pinned_copy_without_card_still_swallows_key`: the action is returned regardless of how many pins exist (`keys.ts` doesn't know the count).
  - `pinned_copy_loses_to_composition_and_confirm`: `isComposing: true` → `null`; `confirming: true` → cancel/confirm as today.
  - Checked after `resetCombo`, before the `metaOrCtrl` fixed combos.
- [ ] **Step 3:** Run `cargo test … --lib domain::settings` and `yarn test` → FAIL.
- [ ] **Step 4: Implement.** Rust: defaults in `get`, add the three keys to the existing `"panelPinShortcut" | "resetSearchShortcut"` validate arm, DTO. TS: dto, `keys.ts`. `PanelApp.onKeyDown`: compute `const accel = acceleratorFromEvent(e, isMac)` once, `pinnedCopy` = index of `accel` in the three settings or `null`; on `copyPinned` → `if (pinned[action.index]) copy(pinned[action.index], false)`. Settings shortcuts tab: three `ShortcutRecorder` rows saving via `setSetting`, each `taken` = `PANEL_KEYS` + global + panel pin + reset search + the other two pinned combos; also add the three pinned combos to the existing three recorders' `taken`.
- [ ] **Step 5:** Run all four checks → PASS.
- [ ] **Step 6: Verify in the app:** with 0, 1 and 3 pinned cards press ⌘1/⌘2/⌘3 — copies exactly like a double-click (toast, sound, panel closes; stays when the panel is pinned), nothing typed into search. Re-record ⌘2 as ⌥2 in Settings and confirm the panel follows (settings://changed). ko/en labels.
- [ ] **Step 7:** Commit `feat(panel): copy pinned cards with Cmd+1/2/3`.

---

### Task 6: Documents through LibreOffice only (HWP stays on rhwp)

**Files:**
- Modify: `src-tauri/src/domain/convert/model.rs` (`DocTargets`, tables, `Caps`), `mod.rs` (`formats_for`, `doc_source`, `convert_doc`, tests), `office.rs` (filters, remove textutil), `src-tauri/src/app/convert.rs:136-142` (`Caps` literal)

**Interfaces:**
- `pub type DocTargets = &'static [(Format, Engine)];` (no more `None` = textutil).
- `TEXT_TARGETS = [Pdf, Docx, Odt, Txt, Rtf, Html]` all `Engine::LibreOffice`. `PLAIN_TARGETS` is removed; `txt`, `html`, `htm` map to `TEXT_TARGETS` in `doc_source`.
- `Caps.textutil` is removed.
- `office::filter` gains `Txt => "txt:Text (encoded):UTF8"`, `Rtf => "rtf:Rich Text Format"`, `Html => "html:XHTML Writer File:UTF8"`. HTML **sources** may need `--infilter=HTML (StarWriter)` so Writer (not Writer/Web) loads them; add it in `soffice_args` only if Step 5 shows docx/pdf from HTML fails without it.
- Removed: `textutil_args`, `convert_textutil`, `TEXTUTIL_TIMEOUT`.

- [ ] **Step 1: Update tests first** in `domain/convert/mod.rs`:
  - `text_documents_leave_out_their_own_format`: `a.docx` → `[Pdf, Odt, Txt, Rtf, Html]`; `a.txt` → `[Pdf, Docx, Odt, Rtf, Html]`; `a.htm` → `[Pdf, Docx, Odt, Txt, Rtf]`.
  - `text_slices_need_libreoffice_when_missing`: `a.txt` with `libreoffice: false` → every slice `needs == Some(LibreOffice)` (not empty — Windows now gets them too).
  - `plain_and_rich_text_now_share_a_family`: `["a.txt", "b.rtf"]` → `[Pdf, Docx, Odt, Html]`.
  - Remove `plain_follows_textutil` and the textutil part of `textutil_slices_are_never_dimmed…`; drop `["a.txt","b.rtf"]` from `mixed_families_are_unsupported`.
  - `office.rs`: `soffice_args` for `Format::Txt` contains `"txt:Text (encoded):UTF8"`; remove textutil tests.
- [ ] **Step 2:** Run `cargo test … --lib domain::convert` → FAIL.
- [ ] **Step 3: Implement** the table/type changes and delete the textutil path. `convert_doc` matches `Engine::Rhwp | Engine::LibreOffice` only.
- [ ] **Step 4:** Run `cargo test … --lib` → PASS.
- [ ] **Step 5: Real LibreOffice check.** LibreOffice is not installed on this Mac (`/Applications/LibreOffice.app` missing). Ask the user to install it (or skip with a note). With it installed, run the `#[ignore]`d pattern used in `video.rs` (env-var-gated test) or `yarn tauri dev`: convert a `.docx`, `.txt`, `.html`, `.rtf` sample to every slice once and confirm each output opens. Add `--infilter` for HTML only if needed.
- [ ] **Step 6:** Commit `feat(convert): run every non-HWP document conversion through LibreOffice`.

---

### Task 7: Hardware encoder diagnostics and badge

**Files:**
- Modify: `src-tauri/src/domain/convert/video.rs` (probe/encode stderr, `Run`, `HwEncoder` serde)
- Modify: `src-tauri/src/app/convert.rs` (`Convert` state, `work`, new command), `src-tauri/src/lib.rs`
- Modify: `src/domain/convert/apis/convert.ts`, `convert.dto.ts`, `src/domain/convert/queries/engines.tsx` (or `convert.tsx`)
- Modify: `src/app/settings/_components/ConvertSection.tsx` (hwAccel row)
- Modify: `src/shared/i18n/{en,ko}.ts`

**Interfaces:**
- `HwEncoder` derives `Serialize` with `#[serde(rename_all = "lowercase")]` → `"videotoolbox" | "nvenc" | "qsv" | "amf"`.
- `fn stderr_tail(bytes: &[u8], n: usize) -> String` in `video.rs` — last `n` non-empty lines, lossy UTF-8, joined by `" | "`.
- Probe: stderr piped; on failure `log::warn!("convert: {} probe failed: {}", e.name(), stderr_tail(.., 5))`.
- Encode: add `-hide_banner -loglevel error` to `ffmpeg_args` (update its tests), pipe stderr into a thread collecting it; on non-success `log::warn!` with `stderr_tail(.., 10)` before returning `Engine`.
- `Run.fell_back: bool` — set to `true` in `convert()` when the hardware attempt failed and software ran.
- `#[derive(Serialize)] #[serde(rename_all = "camelCase")] pub struct VideoEncoder { pub encoder: Option<HwEncoder>, pub fell_back: bool }` in `app/convert.rs`; `encoder: None` means software (CPU).
- `Convert.last_encoder: Mutex<Option<VideoEncoder>>`, written by `work` after any H.264 job that ran FFmpeg (`encoder = hw`, `fell_back = ctx.fell_back`), then `emit("convert://encoder", ())`.
- Command `async fn get_video_encoder(app: AppHandle) -> Option<VideoEncoder>`: `None` if FFmpeg missing; `{ None, false }` if `videoHwAccel` is off; else `last_encoder` if set; else `hw_encoder(ffmpeg)` inside `tauri::async_runtime::spawn_blocking` → `{ that, false }`.
- TS: `getVideoEncoder(): Promise<VideoEncoder | null>`; `interface VideoEncoder { encoder: 'videotoolbox' | 'nvenc' | 'qsv' | 'amf' | null; fellBack: boolean }`; `useVideoEncoder()` query invalidated on `convert://encoder` and `settings://changed` (follow `docs/agents/libs/react-query.md#multiple-windows`).
- Badge text: `VideoToolbox`, `NVENC`, `Quick Sync`, `AMF`, `CPU` (literal brand names, kept in i18n as one map `settings.convert.ffmpeg.encoders`); fallback `"{hw} → CPU"` via i18n `encoderFellBack`.

- [ ] **Step 1: Failing tests** in `video.rs`: `stderr_tail(b"a\n\nb\nc\n", 2) == "b | c"`; `stderr_tail(b"", 3) == ""`; `ffmpeg_args` expectations now start with `-hide_banner -loglevel error`.
- [ ] **Step 2:** Run `cargo test … --lib domain::convert::video` → FAIL.
- [ ] **Step 3: Implement** the Rust side as in Interfaces.
- [ ] **Step 4: Frontend.** In the hwAccel `Row`, show `<span className={BADGE}>` with the encoder label before the `Switch` when `useVideoEncoder()` returns a value (warning style is not needed; the arrow text carries the fallback).
- [ ] **Step 5:** Run all four checks → PASS.
- [ ] **Step 6: Verify (macOS):** with FFmpeg installed, Convert tab shows `VideoToolbox`; switch off → `CPU`; convert a clip to MP4 and confirm the badge stays correct after the job. Without FFmpeg, no badge.
- [ ] **Step 7:** Commit `feat(convert): show which video encoder runs and log why hardware fails`. Ask the user to convert an MP4 on the Windows PC and send the `convert:` log lines (`tauri_plugin_log` default LogDir target: `%LOCALAPPDATA%\com.bobpark.vee\logs\` on Windows; confirm the path in the plugin docs before telling the user).

---

### Task 8: Final verification

- [ ] **Step 1:** `superpowers:verification-before-completion`: run `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` and paste the results.
- [ ] **Step 2:** One `yarn tauri dev` pass over Tasks 2–7 in light/dark × ko/en.
- [ ] **Step 3:** List for the user what still needs their Windows PC: donut afterimage, encoder badge + logs, txt/rtf/html conversion.
