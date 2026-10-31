---
title: Tauri Commands & Events
scope: src-tauri/src/**/commands.rs, src-tauri/src/app/**, src/domain/*/apis/**
applies_to: adding a command, calling Rust from the frontend, emitting or listening to an event
related:
  - ./rust-modules.md
  - ./error-handling.md
  - ../libs/react-query.md
---

# Tauri Commands & Events

> A command is a thin adapter: lock state → call the domain API → emit events → return `AppResult<T>`. Register it in `lib.rs`, mirror it with one function in `src/domain/<name>/apis/<name>.ts` that goes through `call()`. Events are named `<domain>://<event>`. Command names, event names and window labels are a contract — don't rename them.

## Rust side

```rust
#[tauri::command]
pub fn set_setting(app: AppHandle, state: State<AppState>, key: String, value: String) -> AppResult<()> {
    super::set(&state.db.lock().unwrap(), &key, &value)?; // validation lives in the domain API
    if key == "locale" {
        tray::refresh(&app);                               // shell side effect
    }
    changed(&app);                                         // emits settings://changed to every window
    Ok(())
}
```

- **Naming:** snake_case `verb_noun` (`list_clips`, `set_setting`, `check_update`), unique across the whole app.
- **Logic:** none in the command. Validate and apply rules in the domain's `mod.rs`, which is unit-tested.
- **Inputs:** take raw input (`String`, primitives) and let the domain parse it. A bad value becomes `AppError::InvalidInput`. Never trust the webview.
- **Lock scope:** keep the `MutexGuard` inside one expression. Don't hold the DB lock across `await` or while calling code that locks again (`tray::refresh`, `retention::prune_now` read the DB).
- **Registration:** add every command to `tauri::generate_handler![…]` in `lib.rs`. An unregistered command fails at runtime, not at compile time.
- **Async:** use an `async` command only for real async work (`updater::check_update`) or a slow blocking call that must stay off the main thread (`office::get_libreoffice`, `#[tauri::command(async)]`).
- **Where:** domain commands in `domain/<name>/commands.rs`; panel/window commands in `app/window.rs` → `commands`; updater, autostart, disk-access, convert (`convert::commands`), engines (`engines::commands`: `get_engine_status`, `download_engine`, `cancel_engine_download`, `remove_engine`, each taking `engine` = `"ffmpeg" | "rhwp"`), LibreOffice (`office::commands`: `get_libreoffice`, `open_libreoffice_download`), drop-target commands in their `app/` file. `cancel_convert` sits with the other job commands in `convert::commands`.

## Frontend side

```ts
// src/domain/settings/apis/settings.ts
export async function setSetting(key: SettingKey, value: string) {
  return call<void>('set_setting', { key, value });
}
```

- `call()` (`src/shared/api`) is the **only** place that calls `invoke`. It rejects with `AppError`.
- Argument keys are camelCase in TS (`appId`); Tauri maps them to snake_case Rust params (`app_id`).
- DTO types in `apis/<name>.dto.ts` mirror the Rust `serde` output exactly (`rename_all = "camelCase"`, tagged enums).
- Window commands are wrapped in `src/domain/panel/apis/panel.ts` (`hidePanel`, `resizePanel`, `openSettings`, …).

## Events

| Event | Payload | Emitted by | Consumed by |
| ----- | ------- | ---------- | ----------- |
| `settings://changed` | — | `settings::commands::changed` (every settings write), `set_app_excluded` | `ThemeProvider`, `I18nProvider` (invalidate settings), `HistorySection` |
| `settings://opened` | — | `window::show_settings` | `SettingsApp` (reset to first tab) |
| `clips://changed` | — | delete/clear/pin/exclude commands, copy, retention pruning | `PanelApp` (reload), `HistorySection` |
| `clips://added` | — | clipboard watcher | `PanelApp` (reload, select the newest card), `HistorySection` |
| `panel://opened`, `panel://refocused`, `panel://closing`, `panel://closed`, `panel://drag-cancelled` | — | `app/window.rs` (to `panel` only) | `PanelApp` |
| `toast://show` | `{ … }` toast payload | `app/window.rs` (to `toast` only) | `ToastApp` |
| `toast://hide` | — | `app/window.rs` | `ToastApp` |
| `convert://open` | `{ slices, name }` | `app/convert.rs` (to `convert` only) | `ConvertApp` |
| `convert://close` | — | `app/convert.rs` (to `convert` only) | `ConvertApp` |
| `convert://jobs` | `ConvertJob[]` | `app/convert.rs` (to `panel` only) | `useConvertJobs` (`domain/convert/queries`) |
| `engines://status` | `{ engine, status: EngineStatus }` | `app/engines.rs` (every status change, download progress every 250 ms) | `useEngineStatus(engine)` (Settings Convert tab) |
| `editor://closed` | — | `app/editor.rs` `close` (every close path, to `editor` only) | `EditorApp` (reset all state, freeing the images) |
| `update://status` | `UpdateStatus` | `updater::set_status` | `useUpdateStatus` |

- Name new events `<domain>://<event>` in kebab-case. Use `emit_to(<label>, …)` when only one window cares.
- Emit an event whenever a change made in one window must reach others. Each window has its own cache ([react-query](../libs/react-query.md#multiple-windows)).
- Listen with `listen()` in a `useEffect`, and unlisten in its cleanup (`void unlisten.then((fn) => fn())`).

## Permissions

- Plugins used only from Rust (global-shortcut, autostart, updater) need no capability.
- A plugin called **from JS** needs its permission in `src-tauri/capabilities/default.json` (today: `dialog:default`).
- A new window label must be added to that file's `windows` list (today: `panel`, `toast`, `convert`, `settings`, `whats-new`).
