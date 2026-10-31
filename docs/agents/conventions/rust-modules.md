---
title: Rust Modules & Layering
scope: src-tauri/src/**
applies_to: adding or moving Rust code, deciding which module owns logic
related:
  - ../architecture.md
  - ../structure.md
  - ./tauri-commands.md
  - ./error-handling.md
---

# Rust Modules & Layering

> `lib.rs` only assembles. Shell code goes in `app/`, business logic in `domain/<name>/`, infrastructure in `shared/`. Domain core and `shared/` never import `tauri`, so they are unit-testable with an in-memory DB.

## Dependency direction

```
lib.rs → app/*, domain/*/commands.rs → domain/*/{mod,model,repository}.rs → shared/*
```

- `domain/<name>/{mod,model,repository}.rs` and `shared/**` **must not** `use tauri`.
  - Check with `grep -rn "tauri::\|use tauri" src-tauri/src/domain src-tauri/src/shared | grep -v commands.rs`. The output must be empty.
- A domain never calls another domain's `repository` directly. Use the other domain's `mod.rs` exports; they are its public API (e.g. `source_app` calls `clip::delete_unpinned_from_app`, `whats_new` calls `settings::set_raw`).
- `app/` may call a domain's public API (e.g. `watcher` calls `clip::upsert` and `source_app::is_excluded`). Domains never call `app/`. The exception is `commands.rs`, which may trigger shell side effects such as refreshing the tray, re-registering a shortcut or playing a sound.
- Allowed exception: `app/` (`window.rs`, `autostart.rs`) calls `settings::commands::changed` to emit `settings://changed` after changing a setting itself. It still points app → domain.

## Files in a domain

| File | Holds | Visibility |
| ---- | ----- | ---------- |
| `mod.rs` | public API: validation + rules, returns `AppResult<T>`; re-exports model types and the repository functions other code may call | `pub fn`, `pub use` |
| `model.rs` | types, `parse`/`as_str` helpers, constants, `serde` attributes | private `mod`, re-exported from `mod.rs` |
| `repository.rs` | SQL for this domain's tables; takes `&Connection`, returns `AppResult` | `pub` |
| `commands.rs` | `#[tauri::command]` adapters | `pub` (registered in `lib.rs`) |

- Repository functions take only `&Connection` plus plain values. Exceptions: `clip::upsert` and `clip::stats` also take the images dir, because image clips own files there.
- A domain without its own tables needs no `repository.rs` or `model.rs` (`whats_new` is `mod.rs` + `commands.rs`).
- Add `service.rs` only when logic spans several repositories or domains. Don't scaffold it in advance.
- Split a file once it holds two unrelated responsibilities. Otherwise, prefer fewer files.
- App-shell commands live next to their shell code: `app/window.rs` has a `pub mod commands` submodule; `app/updater.rs`, `app/autostart.rs` and `app/disk_access.rs` hold their few commands directly.

## Platform code

- macOS and Windows variants sit side by side behind `#[cfg(target_os = "macos")]` / `#[cfg(windows)]` with the same signature (see `app/front_app.rs`, `app/sound.rs`).
- Change both together. If you can only build one platform, say so in the PR.

## Style

- Format with `cargo fmt --manifest-path src-tauri/Cargo.toml`; `src-tauri/rustfmt.toml` (`max_width = 140`, `use_small_heuristics = "Max"`) sets the style, and `--check` must stay clean.
- Log with the `log` macros (`log::info!`, `log::warn!`), never `println!`. `tauri-plugin-log` routes them to stdout and the log file. Never log clipboard contents.
- Never use `unwrap()` on I/O or DB results outside tests. Return `AppResult` and let the command surface the error. `Mutex::lock().unwrap()` is fine, since poisoning means a prior panic.
- A module opens with a `//!` doc line that says what it owns.
- Mark known shortcuts with `// ponytail:` plus their ceiling and the upgrade path.
- Tests live in a `#[cfg(test)] mod tests` at the bottom of the file they test ([testing](../testing.md)).
