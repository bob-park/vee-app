---
title: Directory Structure
scope: "**"
applies_to: creating files or folders, adding a domain, placing a component
related:
  - ./architecture.md
  - ./conventions/naming.md
  - ./conventions/module-aliases.md
  - ./conventions/rust-modules.md
---

# Directory Structure

> Frontend: `src/{app,domain/<name>/{apis,queries,components,store},shared,utils}`. Rust: `src-tauri/src/{lib.rs,app,domain/<name>/{mod,model,repository,commands}.rs,shared}`. A domain has the **same name** on both sides.

## Tree

```text
src/
├── app/                      # Screens (one per window label) + entry
│   ├── main.tsx              # picks the screen by window label, seeds settings before first paint, mounts providers
│   ├── globals.css           # Tailwind + daisyUI themes + vee tokens — the only stylesheet, the only place colors are defined
│   ├── panel/
│   │   ├── PanelApp.tsx      # `panel` window
│   │   └── _components/      # Card, Toolbar, ResizeHandle, ConfirmButton, FileIcon, motion.ts, cardImage.ts
│   ├── settings/
│   │   ├── SettingsApp.tsx   # `settings` window
│   │   └── _components/      # Tabs, SettingSection, Row, Segmented, Switch, Button, Dropdown, ShortcutRecorder,
│   │                         # HistorySection, AppPicker, ConfirmModal
│   ├── toast/ToastApp.tsx    # `toast` window
│   └── whats-new/WhatsNewApp.tsx  # `whats-new` window
├── domain/<name>/            # clip, source_app, settings, whats_new, updater, panel
│   ├── apis/                 # <name>.ts (call() per command) + <name>.dto.ts (wire types)
│   ├── queries/              # <name>.tsx — React Query hooks
│   ├── components/           # domain-scoped components (all empty today: .gitkeep)
│   └── store/                # Zustand slice (slice.ts + <name>.state.ts), or .gitkeep
├── shared/
│   ├── api/                  # call<T>() (index.ts) + AppError (common.dto.ts)
│   ├── components/           # cross-screen UI: suggest/Suggest (the `@app` list), highlight, queries/RQProvider
│   ├── i18n/                 # en.ts (base Dict), ko.ts, locale.ts, index.ts (dicts, relativeTime)
│   ├── providers/            # theme/ThemeProvider, i18n/I18nProvider (+ useI18n)
│   ├── queries/              # QueryMutationHandle
│   └── store/                # rootStore (combines slices), types.d.ts (SlicePattern augmentation)
└── utils/                    # pure functions only (no React, no I/O) + *.test.ts

src-tauri/
├── tauri.conf.json  capabilities/default.json  icons/  vendor/drag/
└── src/
    ├── main.rs  lib.rs       # lib.rs assembles: plugins, setup, window events, generate_handler!
    ├── app/                  # shell: state, window (+ window::commands), tray, updater, watcher, shortcut,
    │                         # sound, front_app, disk_access, i18n, autostart, retention
    ├── domain/
    │   ├── clip/             # mod.rs · model.rs · repository.rs · commands.rs
    │   ├── source_app/       # mod.rs · model.rs · repository.rs · commands.rs
    │   ├── settings/         # mod.rs · model.rs · repository.rs · commands.rs
    │   └── whats_new/        # mod.rs · commands.rs (stores two keys through the settings API; no table of its own)
    └── shared/               # db.rs, error.rs, png.rs, migrations/000N_*.sql
```

Frontend-only domains:

- `updater`: the Rust half is `app/updater.rs` (not a business domain).
- `panel`: wrappers for the window commands in `app/window.rs` (`hide_panel`, `set_panel_pinned`, `resize_panel`, …). Only `apis/` has files.

## Adding a domain

Do it in this order. Create all four frontend sub-folders even if some stay empty (`.gitkeep`) — a predictable layout beats a minimal file count.

**Rust**

1. `src-tauri/src/domain/<name>/model.rs` — types (`serde`), parsing/validation helpers.
2. If it needs tables: `shared/migrations/000N_<name>.sql`, appended to `MIGRATIONS` ([sqlite-migrations](./conventions/sqlite-migrations.md)).
3. `repository.rs` — SQL only, takes `&Connection`, returns `AppResult`.
4. `mod.rs` — the public API other code calls; validation lives here. Re-export what other modules need (`pub use model::…`, `pub use repository::…`). Unit-test it with `db::open_in_memory()`.
5. `commands.rs` — thin `#[tauri::command]`s; register them in `lib.rs` `generate_handler!` ([tauri-commands](./conventions/tauri-commands.md)).
6. `pub mod <name>;` in `domain/mod.rs`.

**Frontend**

1. `src/domain/<name>/apis/<name>.dto.ts` — wire types mirroring the Rust `serde` output.
2. `apis/<name>.ts` — one function per command via `call()`.
3. `queries/<name>.tsx` — React Query hooks ([react-query](./libs/react-query.md)).
4. `components/` — UI, using `useI18n()` for strings and design tokens ([design](./design.md)).
5. Client-only UI state → `store/slice.ts`, registered in `src/shared/store/rootStore.ts` ([zustand-slice](./libs/zustand-slice.md)).

Snake_case domain folders (`source_app`, `whats_new`) keep camelCase file names inside (`apis/sourceApp.ts`, `queries/whatsNew.tsx`).

## Placement rules

- `shared/` is for code used by **two or more** domains/screens. Promote from a domain or screen when the second user appears, not before.
- `utils/` holds pure functions only; anything touching React, the DOM or IPC goes elsewhere.
- Screen-only sub-components go in a `_components/` folder next to the screen. Do not import another screen's `_components/` — promote to `shared/components/` instead.
- Rust: anything that is not a business domain (windows, tray, updates, shortcuts, clipboard watching, OS integration) goes in `app/`, not `domain/`.
