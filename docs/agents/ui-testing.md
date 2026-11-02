---
title: UI Testing (agent-desktop)
scope: src/**
applies_to: looking at or driving the running app, UI/UX testing, making UI testable
related:
  - ./build-run.md
  - ./testing.md
  - ./design.md
  - ./libs/i18n.md
---

# UI Testing (agent-desktop)

> After a UI change, drive the running **dev** app with `agent-desktop`: start it under its own identifier, read the app name from the config (dev: `vee-app`, release: `Vee`), then loop snapshot → act → snapshot using refs from the latest snapshot only. The panel is not reachable through the accessibility tree: ask the user to open it. Never seed data with `clipboard-set`. Reports go to `.ui-test-results/` (git-ignored).

## Setup

Read the app's name from the config at the start of every session, so a rename needs no doc change. The name agent-desktop sees depends on how the app was started:

| Run | App name in `list-apps` | Source |
| --- | ----------------------- | ------ |
| `yarn tauri dev` (unbundled binary) | Cargo package name: `vee-app` | `src-tauri/Cargo.toml` `[package].name` |
| Release bundle | `productName`: `Vee` | `src-tauri/tauri.conf.json` |

```bash
# macOS / Linux: dev
APP_NAME=$(sed -n 's/^name = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)
# macOS / Linux: release bundle
APP_NAME=$(jq -r .productName src-tauri/tauri.conf.json)
# release, no jq: through mise's Node
APP_NAME=$(mise exec -- node -p "require('./src-tauri/tauri.conf.json').productName")
```

```powershell
# Windows: dev
$APP_NAME = (Select-String -Path src-tauri/Cargo.toml -Pattern '^name = "(.*)"' | Select-Object -First 1).Matches.Groups[1].Value
# Windows: release bundle
$APP_NAME = (Get-Content -Raw src-tauri/tauri.conf.json | ConvertFrom-Json).productName
```

Confirm with `agent-desktop list-apps` before the first snapshot. Always quote `"$APP_NAME"`.

| Item | Value |
| ---- | ----- |
| Dev run | `yarn tauri dev --config '{"identifier":"com.bobpark.vee.dev"}'` (through mise, see [build-run](./build-run.md#gotchas)) |
| Release build | see [build-run](./build-run.md) |
| Results | `.ui-test-results/<YYYYMMDD-HHmm>/` (git-ignored) |

### Install (once)

If `agent-desktop version` fails, **ask the user before installing** it. The same goes for upgrading or removing it.

```bash
mise exec -- npm install -g agent-desktop
agent-desktop version
agent-desktop permissions --guide   # macOS: grant Accessibility to the terminal
```

- **macOS:** `agent-desktop permissions` must report Accessibility `granted`, and Screen Recording `granted` for `screenshot`. Otherwise stop and ask the user to run `agent-desktop permissions --guide`.
- **Windows:** the terminal and the app must run at the same privilege level (admin or not).

## Basic rules

- Before first use in a session, run `agent-desktop skills get desktop` and `agent-desktop skills get platform`, and follow them.
- Don't guess flags. Check `agent-desktop <command> --help`.
- Judge screen state from the **accessibility snapshot**. Use `screenshot` only for visual checks: layout, color, overlap, transparent windows.
- Every command prints a JSON envelope (`ok`, `data` / `error`). Always check `ok`.
- Headless actions only. **Don't use `--headed`** (real mouse and keyboard) without the user's explicit OK.

## Running a test

### Launch

1. Start the dev app under its own identifier (table above). The installed Vee shares the release identifier, so a plain `yarn tauri dev` would open its data. If the installed Vee is running, give the dev app another panel shortcut in its Settings.
2. Confirm it's up: `agent-desktop list-apps` shows `vee-app`.
3. The first `tauri dev` compile is slow. Use `agent-desktop wait` (see `--help`) for the window, never a fixed sleep.

### Windows in Vee

- Six webview windows: `panel`, `settings`, `toast`, `convert`, `editor`, `whats-new` ([overview](./overview.md)). All titles are `Vee`, so pick a window by `--window-id` from `list-windows`, not by title.
- All windows start hidden. To open Settings without input, and **only while the dev app above is running** (`list-apps` shows `vee-app`), launch the dev binary a second time: `src-tauri/target/debug/vee-app &`. Single-instance then shows Settings in the running process. Otherwise the binary may carry the installed identifier and touch the installed Vee's data.
- Settings tabs, the theme tiles and the language segment are all `radiobutton`s, named in the current language (`일반`/`General`, `라이트`/`Light`).
- The tray menu is a native OS element, not webview content. `--surface menubar` shows the app menu only, not the tray icon.

### Observe → act → verify

```bash
agent-desktop snapshot --app "$APP_NAME" -i --compact   # 1) observe: get refs
agent-desktop click @<snapshot_id>:e8                    # 2) act: refs from that snapshot only
agent-desktop snapshot --app "$APP_NAME" -i --compact   # 3) verify: new snapshot, check the result
agent-desktop find --app "$APP_NAME" --role radiobutton --name "Dark" --exact --first
agent-desktop focus-window --app "$APP_NAME"            # before every screenshot
agent-desktop screenshot --app "$APP_NAME" /path/to/shot.png
```

- Ref format is `@<snapshot_id>:<element>`. Short `@e8` fails with `INVALID_ARGS`. In PowerShell, quote it: `'@s8f3k2p9:e8'`.
- **A ref is valid only for the snapshot that produced it.** Take a new snapshot after every change; theme and language switches re-render the page.
- A successful `click` doesn't prove the effect. Check the result with `find`/`snapshot` (`value`, `states: checked`) or a screenshot.
- **`focus-window` before `screenshot`:** WebKit pauses CSS transitions in a background window, so a screenshot can show a stale state (e.g. a theme tile border stays on the old choice).
- For busy screens, get the structure with `--skeleton`, then zoom in with `--root <ref>`.
- Text: `type` or `set-value`. Keys: `press` (there is no `key` command).

### Errors

| Code | Response |
| ---- | -------- |
| `STALE_REF` | The UI changed. Take a new snapshot, retry with a new ref. |
| `AMBIGUOUS_TARGET` | Several windows match. Pick one from `error.details` and pass `--window-id`. |
| `INVALID_ARGS` | Check the ref format and arguments. |
| other | Retry only if `error.disposition.retry` is `safe`. Otherwise stop and record it. |

- Three failures on the same step: record it as **FAIL (blocked)** and move on. Never change the app code to get past it.

### Cleanup

- Restore any setting you changed, then quit with `agent-desktop close-app vee-app` and stop the `yarn tauri dev` process you started.

## Limits in Vee

- **The panel is not reachable.** `list-windows`/`snapshot`/`find` skip the `alwaysOnTop` panel window, even though its accessibility tree exists. `--headed press cmd+shift+v` goes to the foreground process, so global shortcuts never fire.
  - Ask the user to open the **dev** app's panel (its shortcut or tray icon, not the installed Vee's) and pin it so it stays.
  - Check it with `agent-desktop screenshot --screen 0`. This captures the whole desktop: crop to the panel, look only at that, and don't keep or share the full capture.
- **Don't seed data with `clipboard-set`** ([build-run](./build-run.md#gotchas)). It overwrites the user's clipboard and lands in every running Vee. Ask the user first if you need clipboard data.
- The panel and toast windows are transparent: check them with `screenshot` against a light and a dark desktop.

## What to check for a UI change

- Light **and** dark, 한국어 **and** English (Settings → General), and the panel if it changed ([design](./design.md)).

## Making UI testable

agent-desktop finds elements through the accessibility tree. **An element with no name in the tree counts as untestable.** For new UI:

- Clickable things are `<button>`, `<a>`, `<input>`, not a `div` with `onClick`. If a `div` is unavoidable, give it `role` and `tabIndex`.
- Icon-only buttons need `aria-label`. Accessible names are user-visible copy, so they come from the dictionaries ([i18n](./libs/i18n.md)), never a literal string.
- Inputs get a name through `<label htmlFor>` or `aria-label`.
- Expose state: `aria-checked` on toggles/checkboxes, `aria-expanded` on disclosure panels.
- Loading, toasts, and errors use `role="status"` or `role="alert"`, so a snapshot can verify them.
- Accessible names match the visible text and are unique within the app.
- Unnamed or duplicate names found while testing go in the report as **accessibility issues**. Don't fix them in the same change ([AGENTS.md](../../AGENTS.md#convention-enforcement)).

## Reports

Write `.ui-test-results/<YYYYMMDD-HHmm>/report.md`. Save the snapshot JSON and screenshot of each failed step in the same folder. A panel failure's `--screen 0` capture is saved **cropped to the panel only**; never save the full desktop.

```markdown
# UI test results — <APP_NAME> <version/commit>

- Environment: macOS 15.x / Windows 11 / …
- Run: dev | release build

| Check | Result | Notes |
| ----- | ------ | ----- |
| settings theme switch | PASS | |

## Failures
- step, expected vs actual, evidence (snapshot, screenshot files), repro steps

## Accessibility issues
- unnamed button: toolbar, 2nd icon

## UX suggestions
- (works, but hard to use — be specific)
```

- Mark PASS/FAIL **only with snapshot or screenshot evidence**. Anything unchecked is `UNVERIFIED`.
- Keep UX suggestions separate from bugs.

## Safety

- **Touch only the dev app.** Never drive or change the installed Vee or its data (`com.bobpark.vee`). If another app seems needed, stop and ask the user.
- Snapshots, screenshots, and clipboard contents may hold personal data. Never upload them to public repos, issues, or external services. `.ui-test-results/` stays git-ignored.
