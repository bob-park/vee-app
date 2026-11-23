# Auto Next Paste Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** With the `autoNextPaste` setting on, every Cmd+V (Ctrl+V on Windows) moves the clipboard to the next card in panel order, so consecutive pastes walk down the history.

**Architecture:** Vee never pastes and never intercepts the key. A listen-only key watch (macOS `CGEventTap`, Windows `WH_KEYBOARD_LL`) signals a worker thread, which waits 150 ms and then writes the card after the "cursor" (the card Vee knows is on the clipboard) to the clipboard without touching, toasting or sounding. The cursor lives in `AppState`; the clipboard watcher clears it on every real change and sets it when a copy is recorded.

**Tech Stack:** Rust (Tauri 2, rusqlite, `objc2-core-graphics`, `objc2-core-foundation`, `windows`), React + React Query, `src/shared/i18n`.

**Spec:** `docs/superpowers/specs/2026-11-23-auto-next-paste-design.md`

## Global Constraints

- Branch `feature/auto-next-paste`. Commit messages follow `docs/agents/workflow/commit-messages.md`.
- No new crates. The only `Cargo.toml` change allowed is adding features `CFMachPort` and `CFRunLoop` to `objc2-core-foundation` (Task 3), and only after the user has approved it.
- Setting key `autoNextPaste`, values `on` / `off`, default `off`.
- Delay before advancing: 150 ms. Watcher suppression after an advance: 500 ms.
- An advance never calls `clip::touch`, never shows a toast, plays a sound, hides the panel or emits `clips://changed`.
- No key watch is installed while the setting is `off`. Nothing about pressed keys is stored or logged.
- `domain/` stays free of `tauri`. Platform code sits behind `#[cfg(target_os = "macos")]` / `#[cfg(windows)]`; both sides must compile.
- Code and `docs/agents/` in English. Every user-visible string lives in `src/shared/i18n/{en,ko}.ts`.
- Run `node`/`yarn` through mise (`mise exec -- yarn …`).
- Read before coding: `docs/agents/conventions/{rust-modules,tauri-commands,naming,react-sections}.md`, `docs/agents/libs/{react-query,i18n}.md`, `docs/agents/design.md`.

## Review Focus

1. **Copy, then paste right away.** The fresh copy must not be replaced by an old card. Pinned by `no_cursor_means_no_advance` (Task 2) and by the watcher clearing the cursor in `on_clipboard_change`, before the 300 ms debounce (Task 2 step 5).
2. **Copy from a password manager or an excluded app, then paste.** Vee must not advance. Same mechanism: the change clears the cursor and nothing sets it again. Pinned by `no_cursor_means_no_advance`.
3. **A card that can no longer be written** (a files clip whose file was deleted). The advance skips it and writes the one after. Pinned by `skips_cards_that_fail_to_write` (Task 2).
4. **The user copied a pinned card.** The next paste continues at the top of the history, and pins never appear in the sequence. Pinned by `next_after_a_pinned_clip_is_the_newest_unpinned` (Task 1).
5. **macOS disables the tap** (`TapDisabledByTimeout` / `TapDisabledByUserInput`) or the permission is granted after the switch was turned on. The feature must come back without relaunching where the OS allows. No unit test can reach this; Task 3 re-enables the tap in the callback and Task 5 verifies by hand.

Known limit, not fixed here: macOS matches the physical V key (key code 9), so a layout that moves V (Dvorak) is not detected. Mark it with a `shortcut:` comment.

---

### Task 1: Next card query

**Files:**
- Modify: `src-tauri/src/domain/clip/repository.rs` (function next to `touch`, tests in the existing `mod tests`)
- Modify: `src-tauri/src/domain/clip/mod.rs` (re-export, the way `touch` is exported)

**Interfaces:**
- Produces: `clip::next_after(conn: &Connection, id: i64) -> AppResult<Option<i64>>`. The first unpinned clip that sorts after clip `id` in `last_used_at DESC, id DESC`. `None` when `id` is the last one or no longer exists.

- [ ] **Step 1: Write the failing tests** (reuse the `db()`, `text()`, `upsert`, `set_pinned`, `touch`, `delete` helpers already in `mod tests`)

```rust
#[test]
fn next_after_walks_the_panel_order() {
    let (c, d) = db();
    let a = upsert(&c, d.path(), text("a"), None, 1).unwrap();
    let b = upsert(&c, d.path(), text("b"), None, 2).unwrap();
    let top = upsert(&c, d.path(), text("c"), None, 3).unwrap();
    assert_eq!(next_after(&c, top).unwrap(), Some(b));
    assert_eq!(next_after(&c, b).unwrap(), Some(a));
    assert_eq!(next_after(&c, a).unwrap(), None);
}

#[test]
fn next_after_breaks_ties_by_id() {
    let (c, d) = db();
    let a = upsert(&c, d.path(), text("a"), None, 5).unwrap();
    let b = upsert(&c, d.path(), text("b"), None, 5).unwrap();
    assert_eq!(next_after(&c, b).unwrap(), Some(a));
}

#[test]
fn next_after_skips_pinned_clips() {
    let (c, d) = db();
    let a = upsert(&c, d.path(), text("a"), None, 1).unwrap();
    let pin = upsert(&c, d.path(), text("pin"), None, 2).unwrap();
    let top = upsert(&c, d.path(), text("c"), None, 3).unwrap();
    set_pinned(&c, pin, true, 4).unwrap();
    assert_eq!(next_after(&c, top).unwrap(), Some(a));
}

#[test]
fn next_after_a_pinned_clip_is_the_newest_unpinned() {
    let (c, d) = db();
    upsert(&c, d.path(), text("a"), None, 1).unwrap();
    let top = upsert(&c, d.path(), text("b"), None, 2).unwrap();
    let pin = upsert(&c, d.path(), text("pin"), None, 3).unwrap();
    set_pinned(&c, pin, true, 4).unwrap();
    touch(&c, pin, 10).unwrap(); // what copying a pin does
    assert_eq!(next_after(&c, pin).unwrap(), Some(top));
}

#[test]
fn next_after_a_deleted_clip_is_none() {
    let (c, d) = db();
    upsert(&c, d.path(), text("a"), None, 1).unwrap();
    assert_eq!(next_after(&c, 9999).unwrap(), None);
}
```

- [ ] **Step 2: Run** `cargo test --manifest-path src-tauri/Cargo.toml --lib next_after` — FAIL, `next_after` not found.
- [ ] **Step 3: Implement `next_after`** as one query: join `clips c` with the cursor row `cur` (`cur.id = ?1`), `c.pinned = 0`, `(c.last_used_at, c.id) < (cur.last_used_at, cur.id)`, `ORDER BY c.last_used_at DESC, c.id DESC LIMIT 1`, `.optional()`.
- [ ] **Step 4: Run** the same command — 5 passed.
- [ ] **Step 5: Commit** `feat(rust): find the clip after a given one in panel order`

---

### Task 2: Setting, cursor and the advance step

**Files:**
- Modify: `src-tauri/src/domain/settings/model.rs` (`SettingsDto.auto_next_paste: String`)
- Modify: `src-tauri/src/domain/settings/mod.rs` (`get` default `off`; `validate`: `on` | `off`)
- Modify: `src-tauri/src/domain/settings/commands.rs` (`get_settings` fills the field)
- Modify: `src-tauri/src/app/state.rs` (cursor, short suppression)
- Modify: `src-tauri/src/app/watcher.rs` (`Handler::on_clipboard_change`, `capture`)
- Modify: `src-tauri/src/app/window.rs` (`copy_clip`), `src-tauri/src/app/editor.rs` (the copy at ~line 212/252)
- Create: `src-tauri/src/app/auto_paste.rs`; Modify: `src-tauri/src/app/mod.rs` (`pub mod auto_paste;`)

**Interfaces:**
- Consumes: `clip::next_after` (Task 1), `clip::content`, `clip::formats`, `window::write_clipboard`.
- Produces:
  - `AppState::set_paste_cursor(&self, id: Option<i64>)`, `AppState::paste_cursor(&self) -> Option<i64>`
  - `AppState::suppress_watcher_for(&self, ms: u64)` — sets (not extends) the suppression deadline
  - `auto_paste::advance_with(conn: &Connection, cursor: Option<i64>, write: impl FnMut(&ClipContent, Vec<(String, Vec<u8>)>) -> AppResult<()>) -> AppResult<Option<i64>>` — the new cursor, `None` when nothing was written
  - `auto_paste::advance(app: &AppHandle)` — the full step, called by the worker in Task 3
  - `auto_paste::settle(rx: &Receiver<()>, delay: Duration)` — sleeps `delay`, then drains every signal that arrived meanwhile

- [ ] **Step 1: Write the failing tests**

In `domain/settings/mod.rs` tests (use the file's existing in-memory-db pattern):

```rust
#[test]
fn auto_next_paste_defaults_off_and_takes_only_on_off() {
    let conn = db::open_in_memory().unwrap();
    assert_eq!(get(&conn, "autoNextPaste"), "off");
    set(&conn, "autoNextPaste", "on").unwrap();
    assert_eq!(get(&conn, "autoNextPaste"), "on");
    assert!(set(&conn, "autoNextPaste", "yes").is_err());
}
```

In `app/state.rs` tests:

```rust
#[test]
fn short_suppression_replaces_the_long_one() {
    let state = AppState::new(db::open_in_memory().unwrap(), PathBuf::new());
    state.suppress_watcher();
    state.suppress_watcher_for(0);
    assert!(!state.watcher_suppressed());
}

#[test]
fn paste_cursor_starts_unknown() {
    let state = AppState::new(db::open_in_memory().unwrap(), PathBuf::new());
    assert_eq!(state.paste_cursor(), None);
    state.set_paste_cursor(Some(7));
    assert_eq!(state.paste_cursor(), Some(7));
}
```

In `app/auto_paste.rs` tests (in-memory db plus a temp dir, like the clip repository tests; clips `a`, `b`, `c` upserted at times 1, 2, 3):

```rust
#[test]
fn writes_the_card_after_the_cursor() {
    // cursor = c → writes Text("b"), returns Some(b)
}

#[test]
fn no_cursor_means_no_advance() {
    // cursor = None → `write` is never called, returns None
}

#[test]
fn stays_on_the_last_card() {
    // cursor = a → `write` is never called, returns None
}

#[test]
fn skips_cards_that_fail_to_write() {
    // cursor = c, `write` fails for Text("b") → writes Text("a"), returns Some(a)
}

#[test]
fn settle_drops_presses_made_during_the_wait() {
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(()).unwrap();
    tx.send(()).unwrap();
    settle(&rx, Duration::from_millis(10));
    assert!(rx.try_recv().is_err());
}
```

Write each body with real assertions on the `write` calls (collect them in a `Vec`) and the returned cursor.

- [ ] **Step 2: Run** `cargo test --manifest-path src-tauri/Cargo.toml --lib` — the new tests fail to compile.
- [ ] **Step 3: Implement the setting** (`model.rs`, `mod.rs` `get`/`validate`, `commands.rs` `get_settings`).
- [ ] **Step 4: Implement the state** — `paste_cursor: Mutex<Option<i64>>` (initially `None`) with its two accessors, and `suppress_watcher_for`.
- [ ] **Step 5: Wire the cursor**
  - `watcher.rs` `Handler::on_clipboard_change`: inside the existing "not suppressed" branch, `set_paste_cursor(None)` before sending. This runs before the debounce, which is what keeps a just-made copy safe.
  - `watcher.rs` `capture`: after `clip::upsert` succeeds, `set_paste_cursor(Some(id))`.
  - `window.rs` `copy_clip`: on a successful `write_clipboard`, `set_paste_cursor(Some(id))`.
  - `editor.rs`: where the edited image is written and its new `clip_id` is known, `set_paste_cursor(Some(clip_id))`.
  - Do not touch `finish_drag` (it writes nothing to the clipboard). The recorder's `write_clipboard` call has no clip of its own on the clipboard path: set the cursor there only if a clip id for the written content is in hand, else leave the cursor as the watcher left it.
- [ ] **Step 6: Implement `advance_with`, `settle` and `advance`**
  - `advance_with`: from `cursor`, loop `next_after`; load `clip::content` + `clip::formats`; call `write`; on `Err` log at `warn` and continue from that id; on `Ok` return its id.
  - `advance(app)`: return early when any Vee webview window is focused (`app.webview_windows().values().any(|w| w.is_focused().unwrap_or(false))`). Hold the db lock only for the queries: the simplest correct shape is to resolve content inside the lock and write outside it, so call `advance_with` with a `write` closure that calls `window::write_clipboard`. If that would hold the lock across a slow image write, restructure so the lock is released first. After a successful write: `set_paste_cursor(Some(id))`, then `suppress_watcher_for(500)`.
- [ ] **Step 7: Run** `cargo test --manifest-path src-tauri/Cargo.toml --lib` — all pass.
- [ ] **Step 8: Commit** `feat(rust): track the clipboard card and advance to the next one`

---

### Task 3: Key watch and permission

**Files:**
- Modify: `src-tauri/Cargo.toml` — `objc2-core-foundation` features `+ "CFMachPort", "CFRunLoop"` (**user-approved only**)
- Modify: `src-tauri/src/app/auto_paste.rs`
- Modify: `src-tauri/src/lib.rs` (start at setup next to `watcher::spawn`; register the three commands)
- Modify: `src-tauri/src/domain/settings/commands.rs` (`set_setting`: `if key == "autoNextPaste" { auto_paste::sync(&app); }`)
- Check: `src-tauri/capabilities/` per `docs/agents/conventions/tauri-commands.md`

**Interfaces:**
- Consumes: `auto_paste::advance`, `auto_paste::settle` (Task 2).
- Produces:
  - `auto_paste::spawn(app: AppHandle)` — starts the worker thread, then calls `sync`
  - `auto_paste::sync(app: &AppHandle)` — installs the key watch when the setting is `on` (and, on macOS, access is granted) and removes it otherwise; safe to call repeatedly
  - `auto_paste::is_paste(key_is_v: bool, primary: bool, other_mods: bool, repeat: bool) -> bool`
  - Commands: `get_input_access() -> Option<bool>` (`None` off macOS), `request_input_access()`, `open_input_access_settings() -> AppResult<()>` — same shape as the three in `app/screen_access.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn only_a_plain_first_press_of_the_paste_key_counts() {
    assert!(is_paste(true, true, false, false));
    assert!(!is_paste(false, true, false, false)); // another key
    assert!(!is_paste(true, false, false, false)); // no Cmd/Ctrl
    assert!(!is_paste(true, true, true, false)); // Cmd+Shift+V and friends
    assert!(!is_paste(true, true, false, true)); // key repeat
}
```

- [ ] **Step 2: Run** `cargo test --manifest-path src-tauri/Cargo.toml --lib only_a_plain` — FAIL.
- [ ] **Step 3: Implement `is_paste`, the worker and `spawn`** — worker loop: `rx.recv()`, `settle(&rx, 150 ms)`, `advance(&app)`. The `Sender` is what the platform callbacks use; the callbacks do nothing but `is_paste` and `send`.
- [ ] **Step 4: Implement the macOS watch**
  - Access: `CGPreflightListenEventAccess()` / `CGRequestListenEventAccess()`; settings URL `x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent`.
  - Tap: `CGEvent::tap_create` with the session tap location, head insert, `ListenOnly`, key-down mask, on a dedicated thread; `CFMachPort::new_run_loop_source`, add to the thread's run loop, run it.
  - `is_paste` inputs: key code field == 9 (`shortcut:` comment for the layout limit); flags masked to Command/Shift/Control/Alternate equal Command alone; the autorepeat field.
  - In the callback, re-enable the tap on `TapDisabledByTimeout` / `TapDisabledByUserInput`.
  - `tap_create` returns `None` without access: `sync` then does nothing and a later `sync` retries.
  - Removing the watch: disable the tap and stop that thread's run loop.
- [ ] **Step 5: Implement the Windows watch** — `SetWindowsHookExW(WH_KEYBOARD_LL, …)` on a dedicated thread with a `GetMessageW` loop. `is_paste` inputs: `vkCode == 0x56` on `WM_KEYDOWN`; Ctrl down and Shift/Alt/Win up via `GetAsyncKeyState`; repeat = V already down (cleared on its key-up). Always `CallNextHookEx`. Removing: post `WM_QUIT` to the thread, which unhooks on the way out.
- [ ] **Step 6: Wire** `spawn` in `lib.rs`, `sync` in `set_setting`, the commands in the invoke handler (and capabilities if the convention doc requires it). `request_input_access` and `get_input_access` also call `sync`, so access granted while the setting is on takes effect at the next check.
- [ ] **Step 7: Run** `cargo test --manifest-path src-tauri/Cargo.toml --lib` and `cargo check --manifest-path src-tauri/Cargo.toml` — pass, no warnings from the new code. The Windows side can't be compiled on this Mac: say so in the task report.
- [ ] **Step 8: Commit** `feat(rust): watch for paste and move the clipboard to the next card`

---

### Task 4: Settings UI

**Files:**
- Modify: `src/domain/settings/apis/settings.dto.ts` (`autoNextPaste: 'on' | 'off'`)
- Modify: `src/domain/settings/apis/settings.ts` (`getInputAccess`, `requestInputAccess`, `openInputAccessSettings`)
- Modify: `src/domain/settings/queries/settings.tsx` (`useInputAccess`, a copy of `useScreenAccess` with key `['settings', 'inputAccess']`)
- Modify: `src/app/settings/SettingsApp.tsx` (General tab)
- Modify: `src/app/settings/_components/PermissionsSection.tsx`
- Modify: `src/shared/i18n/en.ts`, `src/shared/i18n/ko.ts`

**Interfaces:**
- Consumes: commands `get_input_access`, `request_input_access`, `open_input_access_settings`; setting key `autoNextPaste` (Tasks 2–3).

- [ ] **Step 1: Add the strings** under `settings` in both dictionaries

| key | en | ko |
| --- | --- | --- |
| `autoNextPaste` | Auto next paste | 자동 다음 붙여넣기 |
| `autoNextPasteHint` | Each paste moves the clipboard to the next card in the panel. | 붙여넣을 때마다 클립보드가 패널의 다음 카드로 넘어갑니다. |
| `autoNextPasteAccessHint` | Allow Input Monitoring in the Permissions tab to use this | 사용하려면 권한 탭에서 입력 모니터링을 허용해 주세요 |
| `inputAccess` | Input Monitoring | 입력 모니터링 |
| `inputAccessHint` | Needed to notice a paste for auto next paste | 자동 다음 붙여넣기가 붙여넣기를 알아채는 데 필요해요 |
| `inputAccessRelaunchHint` | Quit and reopen Vee after allowing | 허용한 뒤 Vee를 다시 실행해 주세요 |

- [ ] **Step 2: General tab row** — after the launch-at-login row: `Row` (label `autoNextPaste`; hint `autoNextPasteAccessHint` when the setting is `on` and `useInputAccess().granted === false`, else `autoNextPasteHint`) with a `Switch` (`ariaLabel` = label). On change: `updateSetting({ key: 'autoNextPaste', value })`, and when turning on with `granted === false`, also `run(requestInputAccess())`.
- [ ] **Step 3: Permissions row** — mirror the screen-access row: shown when `granted !== null`, `dim` when `settings.autoNextPaste !== 'on'`, button `c.allow` → `requestInputAccess()` the first time and `c.openSystemSettings` → `openInputAccessSettings()` after, badge `c.allowed` when granted, hint switching to `inputAccessRelaunchHint` after a request while still not granted. Add `granted === null` for this row to the section's early-return condition.
- [ ] **Step 4: Run** `mise exec -- yarn typecheck && mise exec -- yarn lint && mise exec -- yarn test` — pass.
- [ ] **Step 5: Commit** `feat(settings): add the auto next paste switch and its permission row`

---

### Task 5: Docs and verification in the running app

**Files:**
- Modify: `docs/agents/overview.md` (Rust side list; the permissions it prompts for), `docs/agents/architecture.md` (the paste watch next to the clipboard watcher), `docs/agents/configuration.md` (Input Monitoring)

- [ ] **Step 1: Update the three docs** — English, each a few lines: what the watch is, that it is listen-only and installed only while `autoNextPaste` is on, the cursor rule, the permission.
- [ ] **Step 2: Run the four checks** — `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `mise exec -- yarn test`, `mise exec -- yarn typecheck`, `mise exec -- yarn lint`. All pass.
- [ ] **Step 3: Drive the dev app** (`docs/agents/build-run.md`, `docs/agents/ui-testing.md`; use the separate identifier if Vee is installed). Record what was seen for each:
  1. Switch off: copy three texts, paste three times in a text editor → the same (last) text three times.
  2. Switch on, access granted: copy A, B, C; paste three times → C, B, A; a fourth paste → A again. The panel order is unchanged afterwards and no toast or sound appeared.
  3. Pick the third card in the panel, paste twice → that card, then the card below it.
  4. Copy a new text and paste immediately → the new text; the next paste → the card below it.
  5. Copy from an excluded app, paste twice → the same excluded content both times.
  6. Two pastes as fast as possible → same card twice, then the next card (none skipped).
  7. Check 150 ms is enough in a browser text field, a terminal and an editor; raise the constant if any of them pastes the next card instead of the current one, and note the value chosen.
  8. Revoke Input Monitoring with the switch on → the hint appears, plain paste still works. Grant it again → note whether a relaunch was needed; if it never is, drop `inputAccessRelaunchHint` and its use.
  9. Settings General and Permissions tabs in light and dark, Korean and English.
- [ ] **Step 4: Commit** `docs: describe the paste watch and its permission`
