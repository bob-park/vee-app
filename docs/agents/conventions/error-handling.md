---
title: Error Handling
scope: src-tauri/src/**, src/shared/api/**, src/domain/*/queries/**
applies_to: returning an error from Rust, handling a failed command in the UI
related:
  - ./tauri-commands.md
  - ../libs/react-query.md
---

# Error Handling

> Rust returns `AppResult<T>` = `Result<T, AppError>`. `AppError` serializes to `{ code, message }`. `call()` always rejects with that shape. The frontend branches on `code` where it must, and otherwise shows `message` inline next to the action that failed. There is no global error banner.

## Rust

`src-tauri/src/shared/error.rs`:

| Variant | `code` | `message` | Use for |
| ------- | ------ | --------- | ------- |
| `InvalidInput(String)` | `INVALID_INPUT` | the string | bad values from the webview (validation in the domain API) |
| `Db(rusqlite::Error)` | `DB` | SQLite's text | any SQLite failure (`?` converts automatically) |
| `Io(std::io::Error)` | `IO` | the OS text | filesystem failures |
| `PinLimit` | `PIN_LIMIT` | `pin_limit` | pinning a 4th clip (`clip::set_pinned`) |
| `ResizeOff` | `RESIZE_OFF` | `resize_off` | resizing the panel while the setting is off |
| `Other(String)` | `OTHER` | the string | plugin or OS errors (e.g. shortcut, autostart, updater, clipboard, drag); callers pass `e.to_string()` because `shared` must not import `tauri` |

- `Display` strings are **bare** (`#[error("{0}")]`): `message` is exactly the text the command returned before `AppError` existed. Don't add prefixes; the settings window shows `message` to the user.
- Add a variant only when the frontend must **branch** on it. Give it a stable SCREAMING_SNAKE `code` in `AppError::code()` and a serialization test.
- Background work that has no caller (watcher, pruner, `whats_new::save`) logs with `log::warn!`/`log::error!` instead of returning.

## Frontend

- `src/shared/api/common.dto.ts` defines `interface AppError { code: string; message: string }`.
- `call()` (`src/shared/api/index.ts`) always rejects with an `AppError`, never a raw string or `Error`. A non-`AppError` rejection becomes `{ code: 'UNKNOWN', message }`.
- **Branch on `code`**, never on `message`. Example: `PanelApp` shows the pin-limit notice on `PIN_LIMIT`; `ResizeHandle` drops a `RESIZE_OFF` race silently.
- **Show the text inline** where the action lives: `SettingsApp` keeps one error line under the tabs, filled by `errorText(e)` (`src/utils/errorText.ts`) from a mutation's `onError` or the `run()` helper. `ShortcutRecorder` shows its own.
- Mutation hooks accept `QueryMutationHandle<T>` (`onSuccess`, `onError(err: AppError)`) from `src/shared/queries`.
- **Queries (reads)** use the query's `error`/`isError` where a failed read matters (e.g. `useWhatsNew` dismisses the window on failure).
