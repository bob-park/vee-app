# Auto Next Paste Shortcut Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Toggle auto next paste with a global shortcut (default `Cmd+Option+Shift+N`), report the result in a toast, and drop the General-tab switch.

**Architecture:** A fourth `Global` in `app/shortcut.rs` reuses the existing register / conflict / failure / recorder path. Its handler calls `auto_paste::toggle`, which flips the stored `autoNextPaste` value (unless macOS Input Monitoring is refused), re-syncs the key watch, emits `settings://changed` and shows a toast through a new optional `ToastPayload` field.

**Tech Stack:** Rust (Tauri 2, `tauri-plugin-global-shortcut`), React + TypeScript, existing i18n dictionaries.

**Spec:** `docs/superpowers/specs/2026-11-24-auto-next-paste-shortcut-design.md`

## Global Constraints

- No dependency changes (`package.json`, `src-tauri/Cargo.toml`).
- No migration: settings are key-value with defaults in `default_for`.
- Setting key: `autoNextPasteShortcut`; default `CommandOrControl+Alt+Shift+KeyN`.
- User-visible text only in `src/shared/i18n/{en,ko}.ts`; code and `docs/agents/` in English.
- Colors only through existing tokens/classes (`docs/agents/design.md`); no new component, no new window.
- Run `node`/`yarn` through mise (`mise exec -- yarn …`).
- Follow `docs/agents/conventions/` (rust-modules, tauri-commands, naming, typescript, react-sections) and `docs/agents/workflow/commit-messages.md`.
- Don't fix unrelated non-conforming code; note it instead.

## Review Focus

1. Turning on without Input Monitoring (macOS): the value stays `off` and the toast says access is needed — never "On". (Task 1 test `next_state_*`)
2. Windows (`access() == None`): toggling on works with no permission step. (Task 1 test)
3. Access revoked while on, then the shortcut is pressed: it turns **off** (never blocked from turning off). (Task 1 test)
4. The default shortcut fails to register at startup: the Shortcuts row shows the existing "in use" warning and re-recording works, because the key is in `Global::ALL` and `isGlobal`. (Task 1 `globals_map_to_their_setting_keys`, Task 2 manual check)
5. Toggled while the Settings window is open: the row hint flips at once (`settings://changed`), and recording another row's combo equal to this one is refused (`set_shortcut` conflict check covers all globals). (Task 2 manual check)

---

### Task 1: Rust — shortcut, toggle, toast payload

**Files:**
- Modify: `src-tauri/src/domain/settings/model.rs` (constant, `SettingsDto`)
- Modify: `src-tauri/src/domain/settings/mod.rs` (`default_for`, re-export, test)
- Modify: `src-tauri/src/domain/settings/commands.rs` (`get_settings`)
- Modify: `src-tauri/src/app/shortcut.rs` (`Global`, `register`, module doc, test)
- Modify: `src-tauri/src/app/auto_paste.rs` (`next_state`, `toggle`, tests)
- Modify: `src-tauri/src/app/window.rs` (`AutoPasteNote`, `ToastPayload`)

**Interfaces:**
- Produces (serialized to the frontend):
  - `SettingsDto.auto_next_paste_shortcut: String` → JSON `autoNextPasteShortcut`
  - `ToastPayload.auto_next_paste: Option<AutoPasteNote>` → JSON `autoNextPaste: "on" | "off" | "needsAccess" | null`
- Produces (Rust):
  - `settings::DEFAULT_AUTO_NEXT_PASTE_SHORTCUT: &str = "CommandOrControl+Alt+Shift+KeyN"`
  - `shortcut::Global::AutoNextPaste`, key `"autoNextPasteShortcut"`
  - `window::AutoPasteNote { On, Off, NeedsAccess }` (`#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]`, `#[serde(rename_all = "camelCase")]`)
  - `window::ToastPayload::auto_next_paste(note: AutoPasteNote) -> Self` (`ok = note != NeedsAccess`, `kind: Kind::Text`, everything else empty)
  - `auto_paste::next_state(on: bool, access: Option<bool>) -> AutoPasteNote`
  - `auto_paste::toggle(app: &AppHandle)`

- [ ] **Step 1: Write the failing tests**

`src-tauri/src/app/auto_paste.rs` tests:

```rust
#[test]
fn next_state_flips_and_refuses_to_turn_on_without_access() {
    use crate::app::window::AutoPasteNote::{NeedsAccess, Off, On};
    assert_eq!(next_state(false, None), On); // Windows: no permission to ask for
    assert_eq!(next_state(false, Some(true)), On);
    assert_eq!(next_state(false, Some(false)), NeedsAccess);
    assert_eq!(next_state(true, Some(true)), Off);
    assert_eq!(next_state(true, Some(false)), Off); // revoked while on: still turns off
    assert_eq!(next_state(true, None), Off);
}
```

`src-tauri/src/app/shortcut.rs`, add to `globals_map_to_their_setting_keys`:

```rust
assert_eq!(Global::AutoNextPaste.key(), "autoNextPasteShortcut");
assert_eq!(Global::ALL.len(), 4);
```

`src-tauri/src/domain/settings/mod.rs`, add to `auto_next_paste_defaults_off_and_takes_only_on_off`:

```rust
assert_eq!(get(&conn, "autoNextPasteShortcut"), "CommandOrControl+Alt+Shift+KeyN");
assert!(set(&conn, "autoNextPasteShortcut", "Alt+KeyX").is_err()); // globals go through set_shortcut
```

`src-tauri/src/app/window.rs` tests (add a `tests` item next to the existing ones):

```rust
#[test]
fn auto_next_paste_toast_serializes_its_state() {
    let json = |n| serde_json::to_value(ToastPayload::auto_next_paste(n)).unwrap();
    assert_eq!(json(AutoPasteNote::On)["autoNextPaste"], "on");
    assert_eq!(json(AutoPasteNote::On)["ok"], true);
    assert_eq!(json(AutoPasteNote::Off)["ok"], true);
    assert_eq!(json(AutoPasteNote::NeedsAccess)["autoNextPaste"], "needsAccess");
    assert_eq!(json(AutoPasteNote::NeedsAccess)["ok"], false);
    assert!(serde_json::to_value(ToastPayload::failed()).unwrap()["autoNextPaste"].is_null());
}
```

(If `serde_json` isn't already usable in this crate's tests, assert on the struct fields instead — do not add a dependency.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: compile errors for `next_state`, `Global::AutoNextPaste`, `AutoPasteNote`.

- [ ] **Step 3: Settings**

- `model.rs`: add `DEFAULT_AUTO_NEXT_PASTE_SHORTCUT` (doc comment: "Global: turns auto next paste on or off.") and `pub auto_next_paste_shortcut: String` in `SettingsDto` after `auto_next_paste`.
- `mod.rs`: re-export the constant; `default_for("autoNextPasteShortcut")` returns it. Do **not** add the key to `validate` (same as `captureShortcut`).
- `commands.rs`: fill `auto_next_paste_shortcut: get(&conn, "autoNextPasteShortcut")`.

- [ ] **Step 4: Toast payload**

In `window.rs` add `AutoPasteNote`, the field `pub auto_next_paste: Option<AutoPasteNote>` (doc: "Set when the toast reports auto next paste being toggled."), `auto_next_paste: None` in every existing constructor, and `ToastPayload::auto_next_paste`.

- [ ] **Step 5: Toggle**

In `auto_paste.rs`:

- `next_state` as pinned by the test.
- `pub fn toggle(app: &AppHandle)`:
  1. Read `autoNextPaste` (lock released before anything else), compute `next_state(on, platform::access())`.
  2. `NeedsAccess`: call `platform::request_access()` (macOS shows its prompt only the first time); store nothing.
  3. `On` / `Off`: `settings::set(&conn, "autoNextPaste", "on" | "off")`; on error `log::error!` and show `ToastPayload::failed()`, then return. Otherwise `sync(app)` and `settings::commands::changed(app)`.
  4. `window::show_toast(app, ToastPayload::auto_next_paste(note))`.
- Update the module doc's first paragraph: the setting is toggled by a global shortcut.

- [ ] **Step 6: Shortcut**

In `shortcut.rs`: add `Global::AutoNextPaste` to the enum, `ALL` (`[Global; 4]`), `key()`; in `register` add the arm `Global::AutoNextPaste => crate::app::auto_paste::toggle(app)`. Update the module doc line to list it. `register_stored` and `set_shortcut` need no change (they iterate `Global::ALL`).

- [ ] **Step 7: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: PASS, no warnings from the touched files.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src
git commit -m "feat(rust): toggle auto next paste with a global shortcut"
```

---

### Task 2: Frontend — shortcuts row, toast, remove the switch

**Files:**
- Modify: `src/domain/settings/apis/settings.dto.ts`
- Modify: `src/domain/clip/apis/clip.dto.ts`
- Modify: `src/shared/i18n/en.ts`, `src/shared/i18n/ko.ts`
- Modify: `src/app/settings/SettingsApp.tsx`
- Modify: `src/app/toast/ToastApp.tsx`

**Interfaces:**
- Consumes: `Settings.autoNextPasteShortcut: string`, `ToastPayload.autoNextPaste: 'on' | 'off' | 'needsAccess' | null` (Task 1).

- [ ] **Step 1: DTOs**

- `settings.dto.ts`: add `'autoNextPasteShortcut'` to `GlobalShortcutKey`; add `autoNextPasteShortcut: string` to `Settings` (doc: "Global: turns auto next paste on or off.").
- `clip.dto.ts`: add `autoNextPaste: 'on' | 'off' | 'needsAccess' | null` to `ToastPayload` (doc mirrors the Rust one).

- [ ] **Step 2: i18n** (exact copy)

Top level, next to `sidePinsTitle`:

| key | en | ko |
| --- | --- | --- |
| `autoNextPasteTitle` | `Auto next paste` | `자동 다음 붙여넣기` |
| `autoNextPasteOn` | `On` | `켜짐` |
| `autoNextPasteOff` | `Off` | `꺼짐` |
| `autoNextPasteNeedsAccess` | `Allow Input Monitoring to turn it on` | `켜려면 입력 모니터링을 허용해 주세요` |

Under `settings`:

| key | en | ko |
| --- | --- | --- |
| `autoNextPasteShortcutHint` | ``(on: boolean) => `Each paste moves the clipboard to the next card · ${on ? 'On' : 'Off'} now` `` | ``(on: boolean) => `붙여넣을 때마다 클립보드가 다음 카드로 넘어가요 · 현재 ${on ? '켜짐' : '꺼짐'}` `` |

Keep `settings.autoNextPaste` (row label) and `settings.autoNextPasteAccessHint`. Remove `settings.autoNextPasteHint` if `grep -rn autoNextPasteHint src` shows no other user.

- [ ] **Step 3: `SettingsApp.tsx`**

- Delete the General-tab `Row` + `Switch` for `autoNextPaste` (and the `requestInputAccess` import if it becomes unused; keep `inputGranted`, it feeds the hint below).
- Add `'autoNextPasteShortcut'` to `SHORTCUT_KEYS` right after `'recordShortcut'`, and to `isGlobal`.
- `shortcutLabel` gains the data it needs for the new case (signature: `shortcutLabel(s, key, autoPaste: { on: boolean; denied: boolean })`): label `s.autoNextPaste`; hint `s.autoNextPasteAccessHint` when `on && denied`, else `s.autoNextPasteShortcutHint(on)`. Pass `{ on: settings.autoNextPaste === 'on', denied: inputGranted === false }` at the call site.

- [ ] **Step 4: `ToastApp.tsx`**

- Title/summary: when `p.autoNextPaste !== null`, `[t.autoNextPasteTitle, on → t.autoNextPasteOn | off → t.autoNextPasteOff | needsAccess → t.autoNextPasteNeedsAccess]`, checked before the `pinsFit` branch.
- Tile: `on` keeps the default look (success colour + check badge); `off` shows the icon without the check badge and dimmed with an opacity utility (no new colour token); `needsAccess` is the existing `!p.ok` look.

- [ ] **Step 5: Static checks**

Run: `mise exec -- yarn typecheck && mise exec -- yarn lint && mise exec -- yarn test`
Expected: all pass (a missing i18n key in one dictionary fails `typecheck`).

- [ ] **Step 6: See it running**

Follow `docs/agents/build-run.md` (separate identifier if Vee is installed) and `docs/agents/ui-testing.md`. Check in light + dark and ko + en:

- Shortcuts tab shows the new row with `⌥⇧⌘N`, hint ends with the current state; General tab has no auto next paste row.
- Pressing the shortcut shows the On toast, again the Off toast; the row hint flips while Settings is open.
- With Input Monitoring not granted (macOS): the toast says access is needed and the hint still says Off.
- With it on: copy three items, paste three times → three different cards (feature still works).
- Recording another row with `⌥⇧⌘N` is refused; the default registers without the "in use" warning. If it does not register on this machine, stop and report (the default is a spec decision).

- [ ] **Step 7: Commit**

```bash
git add src
git commit -m "feat(settings): move auto next paste to a shortcut with a toast"
```

---

### Task 3: Docs and final verification

**Files:**
- Modify: `docs/agents/architecture.md` (Auto next paste section, ~line 95–97; global shortcuts mention)
- Modify: `docs/agents/configuration.md:58`
- Modify: `docs/agents/overview.md:28` only if its wording says "setting switch"

- [ ] **Step 1: Update the docs**

State that `autoNextPaste` is flipped by the global shortcut `autoNextPasteShortcut` (`shortcut::Global::AutoNextPaste` → `auto_paste::toggle`), that `sync` now also runs from `toggle`, that turning on is refused with a toast while Input Monitoring is denied, and that the General-tab switch is gone.

- [ ] **Step 2: Full check**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint
```
Expected: all four pass.

- [ ] **Step 3: Commit**

```bash
git add docs
git commit -m "docs: describe the auto next paste shortcut"
```
