---
title: Naming
scope: "**"
applies_to: creating any file, type, function, command, or event
related:
  - ./module-aliases.md
  - ./tauri-commands.md
  - ../structure.md
---

# Naming

> Domains share one lowercase name across `src/domain/<name>` and `src-tauri/src/domain/<name>` (`clip`, `source_app`, `settings`, `whats_new`). Rust is snake_case. TS functions are camelCase and mirror the command (`list_clips` → `listClips`). Components use `PascalCase.tsx`, hooks `useXxx`, DTOs `*.dto.ts`.

## Frontend files

| Kind | Pattern | Example |
| ---- | ------- | ------- |
| Command wrappers | `apis/<domain>.ts` | `apis/clip.ts`, `apis/sourceApp.ts` |
| Wire types | `apis/<domain>.dto.ts` | `apis/updater.dto.ts` |
| Query hooks | `queries/<domain>.tsx` | `queries/settings.tsx` |
| Components | `PascalCase.tsx`, default export | `Switch.tsx`, `Suggest.tsx` |
| Zustand | `store/slice.ts` + `store/<domain>.state.ts` | `domain/clip/store/` |
| Pure utils | `camelCase.ts` + `camelCase.test.ts` | `utils/selection.ts` |
| Screen | `<Name>App.tsx` under `src/app/<screen>/` | `panel/PanelApp.tsx` |

- File names inside a snake_case domain folder are camelCase: `domain/source_app/apis/sourceApp.ts`.
- The screens and most panel/settings components still use **named** exports (`export function PanelApp`). New components use a default export; don't convert existing ones in an unrelated change.

## Identifiers

- Query hooks: `useXxx` for reads (`useSettings`, `useStats`, `useKnownApps`) and `useXxxUpdate` / verb-based names for mutations (`useSettingUpdate`). A mutation hook returns `{ <verb>Xxx, isLoading }` (`{ updateSetting, isLoading }`).
- Query keys: `[domain, resource, ...specifier]` with the domain in camelCase, e.g. `['clip', 'stats']`, `['sourceApp', 'known']`, `['settings']`. Export a `XXX_QUERY_KEY` const when other code (providers, `main.tsx`, invalidating components) needs it.
- i18n keys: nested by area (`t.settings.theme`, `t.filters.all`). camelCase leaves; functions for interpolation (`version: (v) => …`).

## Rust

- Modules and files: snake_case. Types: PascalCase. Functions: snake_case.
- Commands: `verb_noun`, unique app-wide. Domain API functions read as nouns or verbs on the domain (`clip::list`, `clip::set_pinned`, `settings::get`).
- Events: `<domain>://<kebab-event>` (`settings://changed`, `panel://drag-cancelled`).
- Settings keys in the `settings` table are **camelCase** (`soundName`, `panelPinShortcut`, `confirmDelete`) — existing keys are stored on users' machines; never rename one. A new user-editable key also needs a default in `settings::get`, a rule in `settings::validate`, a field in `SettingsDto` / `get_settings`, and matching `Settings` / `SettingKey` entries in `settings.dto.ts`.
