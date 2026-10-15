# Apply Template Structure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reshape vee-app to match `template-rust-app` (docs, CI bump, libraries, frontend and Rust directory structure, Tailwind styling) with no visible or behavioral change.

**Architecture:** Behavior-preserving refactor in phases that each compile and pass tests: tooling → CI → Rust `shared/` → Rust `app/` shell moves → Rust `domain/` split + `AppError` → frontend tree move → React Query/Zustand/providers → Tailwind rewrite per window → docs. Template code is copied and adapted, never re-invented.

**Tech Stack:** Tauri 2, Rust 2024, rusqlite, thiserror · React 19, TypeScript 6, Vite 8, Tailwind 4 + daisyUI 5, React Query 5, Zustand 5 + immer, classnames · ESLint 9/Prettier with bobpark configs.

**Spec:** `docs/superpowers/specs/2026-10-16-apply-template-design.md`

**Template root (read-only reference):** `/Users/hwpark/Documents/rust-workspace/template-rust-app` — referred to below as `$T`.

## Global Constraints

- No feature, behavior, or visual change. Bugs or convention violations found along the way go in `docs/superpowers/notes/2026-10-16-apply-template-followups.md`, not fixed.
- Tauri command names, event names (`settings://changed`, `clips://changed`, `clips://added`, `panel://*`, `toast://*`, `update://status`, `settings://opened`), window labels (`panel`, `settings`, `toast`, `whats-new`) and setting keys stay exactly as they are.
- SQLite schema and `user_version` numbering (1–5) unchanged. A DB with `user_version` > 5 still opens (no `SchemaTooNew`).
- Dependencies: add exactly `@tanstack/react-query ^5`, `classnames ^2.5.1`, `immer ^11.1.21`, `zustand ^5.0.15`; dev `@tailwindcss/vite ^4`, `tailwindcss ^4`, `daisyui ^5.7.50`, `@tanstack/react-query-devtools ^5`, `eslint ^9`, `prettier ^3`, `@bob-park/eslint-config-bobpark 0.3.0-RC4-20260630`, `@bob-park/prettier-config-bobpark 0.4.0-RC4-20260630`; `typescript` → `^6`; Cargo `thiserror = "2"`. Nothing else is added or removed.
- Every vee-only dependency stays (global-shortcut, dialog, drag (vendored), clipboard-rs, sha2, base64, objc2*, windows*, image, sys-locale).
- Every commit compiles and passes `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`. `yarn lint` must pass from Task 8 on.
- macOS and Windows code paths (`#[cfg(...)]`) move intact.
- Code, comments, and `docs/agents/` in English; user-facing strings only in `src/shared/i18n/{en,ko}.ts`; human docs may be Korean.
- yarn must be 4.18.1 (`yarn --version`; otherwise `corepack yarn@4.18.1 …`).
- `GITHUB_NPM_AUTH_TOKEN` (read:packages) must be set in the shell for `yarn install`.

## Review Focus

1. An existing user DB at `user_version = 5` with clips, pins, excluded apps and settings opens after the refactor with every row intact; a DB at `user_version = 6+` (from a newer build) still opens. → tests in Task 4.
2. Errors the settings window shows as text (invalid shortcut, invalid setting, disk-access) read exactly as before, not `[object Object]`. → Rust `AppError` display test in Task 3, frontend `errorText` test in Task 6.
3. Pinning a 4th clip still shows the pin-limit notice; resize while resize is off is still silently dropped. → `PIN_LIMIT`/`RESIZE_OFF` code tests in Task 3 and Task 6.
4. Changing theme/language/settings in the settings window updates the panel, toast and what's-new windows live (`settings://changed`). → manual check in Task 9 and Task 12.
5. Tailwind's preflight and daisyUI defaults must not leak into the look: transparent panel/toast window background, button resets, focus ring, scrollbars hidden where they are now, `.card[data-id]`/`.update`/`.ghost` hooks still present for `motion.ts`/`Panel.tsx` (FLIP animation, drag ghost, outside-click). → screenshot comparison in Tasks 10–12 plus the `grep` check in Task 12.

---

### Task 1: Baseline screenshots (no commit)

**Files:** none in repo. Output: `/private/tmp/vee-baseline/` (outside the repo).

- [ ] **Step 1:** On `master` (`git stash`-free: use `git worktree add /private/tmp/vee-master master`), run `yarn install && yarn tauri dev` there.
- [ ] **Step 2:** Capture each window in 4 combos (light/dark × ko/en): panel (with text, link, image, files clips, one pinned, `@app` suggest open once, filter chip set), settings (every tab, a confirm modal, a dropdown open), toast, what's-new (seed via `whatsNewVersion`/`whatsNewNotes` settings rows or by calling the dev command). Use `screencapture -x -l <windowid>` or ⇧⌘4-space. Name `<window>-<tab/state>-<theme>-<locale>.png`.
- [ ] **Step 3:** Note the window size used; Tasks 10–12 capture at the same size. Remove the worktree after Task 12.

### Task 2: Tooling — deps, lint/format, Tailwind wiring, alias

**Files:**
- Modify: `package.json`, `.yarnrc.yml`, `vite.config.ts`, `tsconfig.json`, `src-tauri/Cargo.toml`
- Create: `eslint.config.mjs` (copy `$T/eslint.config.mjs`)

- [ ] **Step 1:** `package.json`: add the deps from Global Constraints, `typescript ^6`, scripts `"lint": "eslint ./src"`, `"prettier": "prettier --write \"src/**/*.{ts,tsx,css}\""`, and `"prettier": "@bob-park/prettier-config-bobpark"`. Keep `license`, `version`, `@tauri-apps/plugin-dialog`.
- [ ] **Step 2:** `.yarnrc.yml` = `$T/.yarnrc.yml` verbatim (keeps `nodeLinker: node-modules`).
- [ ] **Step 3:** `vite.config.ts`: add `tailwindcss()` plugin and the `@` alias exactly as `$T/vite.config.ts`, keeping vee's existing server/build options. `tsconfig.json`: add `"paths": { "@/*": ["./src/*"] }` and any `$T` options vee lacks; keep `allowImportingTsExtensions`.
- [ ] **Step 4:** `src-tauri/Cargo.toml`: add `thiserror = "2"` under `[dependencies]`.
- [ ] **Step 5:** Run `yarn install && yarn typecheck && yarn test && cargo test --manifest-path src-tauri/Cargo.toml --lib`. Expected: all pass (no code uses the new deps yet; lint not run yet).
- [ ] **Step 6:** Commit `chore: add template libraries, lint and Tailwind tooling`.

### Task 3: CI bump workflow

**Files:**
- Create: `.github/workflows/bump-version.yml`, `bump-version-up.sh`, `scripts/bump-version-up.test.mjs` (from `$T`)
- Modify: `scripts/release.sh`, `scripts/release.ps1`

- [ ] **Step 1:** Copy the three files. In `bump-version-up.sh` set `CRATE_NAME="vee-app"` and the header comment to "on every merge into master". Adapt the test fixture crate name likewise.
- [ ] **Step 2:** Workflow: `branches: [master]` only; header comment "Every merge into master bumps the app version"; delete the "Decide whether to bump" step and the `if: steps.should-bump.outputs.bump == 'true'` guards (always bump). Drop the `head` output from "Get PR details".
- [ ] **Step 3:** `release.sh`/`release.ps1`: port `$T`'s "already published with this platform's build" guard (the `shipped=$(gh release view …)` block and its PowerShell equivalent from `$T/scripts/release.ps1`) using `REPO="bob-park/vee-app"` and vee's asset names. Do not add `--target main`.
- [ ] **Step 4:** `yarn test` → `bump-version-up.test.mjs` passes. `bash bump-version-up.sh patch` on a scratch copy of the four version files bumps 0.8.1→0.8.2 (then `git checkout` them).
- [ ] **Step 5:** Commit `ci: bump the version on every merge into master`.

### Task 4: Rust `shared/` — `AppError` and `db`

**Files:**
- Create: `src-tauri/src/shared/{mod.rs,error.rs,db.rs}`, `src-tauri/src/shared/migrations/{0001_init.sql,0002_clip_thumbs.sql,0003_clip_formats.sql,0004_pins_and_exclusions.sql,0005_pin_order.sql}`
- Modify: `src-tauri/src/lib.rs` (`mod shared;`)

**Interfaces — Produces:**
```rust
// shared/error.rs
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")] InvalidInput(String),           // code "INVALID_INPUT"
    #[error("{0}")] Db(#[from] rusqlite::Error),     // "DB"
    #[error("{0}")] Io(#[from] std::io::Error),      // "IO"
    #[error("pin_limit")] PinLimit,                  // "PIN_LIMIT"
    #[error("resize_off")] ResizeOff,                // "RESIZE_OFF"
    #[error("{0}")] Other(String),                   // "OTHER" — plugin/OS errors, message = their to_string()
}
impl AppError { pub fn code(&self) -> &'static str }
impl Serialize for AppError // { code, message: self.to_string() }
pub type AppResult<T> = Result<T, AppError>;
// No From<tauri::Error>: shared/ must not import tauri. Callers convert with AppError::Other(e.to_string()).
// shared/db.rs
pub fn open(path: &Path) -> AppResult<Connection>;         // create_dir_all(parent), WAL, migrate
#[cfg(test)] pub fn open_in_memory() -> AppResult<Connection>;
```
Display strings are deliberately bare (`"{0}"`) so `message` equals today's `e.to_string()` text.

- [ ] **Step 1: Split SQL.** Each `SCHEMA_Vn` from `store.rs` becomes `migrations/000n_*.sql` with its statements unchanged, minus `BEGIN;`, `COMMIT;` and `PRAGMA user_version = n;` (the migrator wraps each in a transaction and sets the version).
- [ ] **Step 2: Write failing tests** in `db.rs`:
  - `opens_a_database_from_a_newer_app`: open, set `user_version` 99, reopen → `Ok`, version still 99.
  - `fresh_database_reaches_version_5`: `open_in_memory()` → `user_version == 5`, tables `apps, clips, settings, clip_thumbs, clip_formats` exist, `clips` has `pinned`, `pinned_at`; `apps` has `excluded`.
  - `migrations_match_the_shipped_schema`: a DB built by today's `store.rs` `SCHEMA_V1..V5` (inline copies in the test) and one built by `open_in_memory()` have identical `SELECT type,name,sql FROM sqlite_master ORDER BY name`.
  - `open_creates_missing_parent_dirs` (from `$T`).
  - In `error.rs`: `serializes_code_and_message` — `PinLimit` → `{"code":"PIN_LIMIT","message":"pin_limit"}`; `InvalidInput("invalid setting a=b")` → message `"invalid setting a=b"`.
- [ ] **Step 3:** Run `cargo test --lib shared::` → FAIL (not implemented).
- [ ] **Step 4:** Implement `error.rs` (adapt `$T`) and `db.rs` (adapt `$T`; `MIGRATIONS` = the 5 files; remove the `SchemaTooNew` branch; keep `PRAGMA journal_mode=WAL` from `Store::init`).
- [ ] **Step 5:** `cargo test --lib` → all pass (store.rs still used by the app; `shared` may carry `#[allow(dead_code)]` until Task 6 removes it).
- [ ] **Step 6:** Commit `refactor(rust): add shared AppError and file-based migrations`.

### Task 5: Rust `app/` — move shell modules

Pure moves + path fixes; no logic change.

**Files:**
- Create `src-tauri/src/app/mod.rs` and move: `windows.rs`→`app/window.rs`, `tray.rs`→`app/tray.rs`, `updater.rs`→`app/updater.rs`, `watcher.rs`→`app/watcher.rs`, `sound.rs`→`app/sound.rs`, `disk_access.rs`→`app/disk_access.rs`, `source_app.rs`→`app/front_app.rs` (OS frontmost-app integration; the name `source_app` is reserved for the domain).
- Create `app/state.rs`: `AppState` + `now_ms()` from `lib.rs` (move its test with it).
- Create `app/i18n.rs`: `tray::Labels`, `labels(locale)`, `resolve_locale(setting)` from `tray.rs` (with their tests).
- Create `app/shortcut.rs`: `register_shortcut`, `register_stored_shortcut` from `settings.rs`.
- Create `app/autostart.rs`: `set_autostart` command from `settings.rs`.
- Create `app/retention.rs`: `prune_now`, `spawn_pruner`, `PRUNE_EVERY` from `settings.rs`.
- Modify `lib.rs`: `mod app;` and updated paths; still uses `store.rs`/`settings.rs`/`whats_new.rs` for now.

- [ ] **Step 1:** Do the moves with `git mv` so history follows; fix `use crate::…` paths.
- [ ] **Step 2:** `cargo test --lib` → same test count as before Task 5 (record it first with `cargo test --lib 2>&1 | grep "test result"`), all pass. `cargo clippy --manifest-path src-tauri/Cargo.toml` no new warnings.
- [ ] **Step 3:** Commit `refactor(rust): move the app shell into app/`.

### Task 6: Rust `domain/` split + `AppError` everywhere (+ frontend error adapter)

**Files:**
- Create `src-tauri/src/domain/mod.rs` and:
  - `domain/clip/{mod.rs,model.rs,repository.rs,commands.rs}` — clips, thumbs, formats, pins, stats, prune/clear. Commands: `list_clips, list_pinned, delete_clip, clear_history, copy_clip, set_pinned, get_stats, start_drag`.
  - `domain/source_app/{mod.rs,model.rs,repository.rs,commands.rs}` — `apps` table. Commands: `list_apps, list_known_apps, count_app_clips, set_app_excluded, list_excluded_apps`.
  - `domain/settings/{mod.rs,model.rs,repository.rs,commands.rs}` — `get`, defaults, `validate`, `cutoff_for`, `copy_sound`, `SettingsDto`. Commands: `get_settings, set_setting, set_shortcut, count_prunable, preview_sound`.
  - `domain/whats_new/{mod.rs,commands.rs}` — `save`, `due` (pure on `&Connection`); `show_if_due`/`dismiss` (window work) move to `app/window.rs` or stay in `commands.rs` as thin adapters.
- Panel/window commands `hide_panel, set_panel_pinned, resize_panel, reset_panel_height, reveal_panel, open_settings` → `app/window.rs`; `get_disk_access, open_disk_access_settings` stay in `app/disk_access.rs`; updater commands stay in `app/updater.rs`.
- Delete `store.rs`, `settings.rs`, `whats_new.rs` (root).
- Modify `app/state.rs`: `AppState { db: Mutex<Connection>, images_dir: PathBuf, prev_app_pid, suppress_until, dragging, panel_pinned }` (field names otherwise unchanged).
- Modify `lib.rs`: only plugins, setup, `on_window_event`, `generate_handler!` (same 32 commands, new paths).
- Frontend: Create `src/shared/api/{index.ts,common.dto.ts}` (copy `$T`), `src/utils/errorText.ts` + `src/utils/errorText.test.ts`; modify `src/api.ts` to call through `call()`; `src/panel/Panel.tsx:225` compares `e.code === "PIN_LIMIT"`; `src/settings/Settings.tsx:155` uses `errorText(e)`.

**Interfaces — Produces:**
```rust
// repository functions take the connection (and images dir where files are touched):
domain::clip::repository::upsert(conn: &Connection, images: &Path, clip: NewClip, app_id: Option<i64>, now: i64) -> rusqlite::Result<i64>
// … one free fn per former Store method, same name and arguments, `&self` → `conn: &Connection` (+ `images: &Path` for upsert/delete/clear/prune/stack/preview_png/content).
domain::clip::set_pinned(conn, id, pinned, now) -> AppResult<()>   // Err(AppError::PinLimit) past MAX_PINNED
domain::settings::get(conn: &Connection, key: &str) -> String
app::window::resize_panel(...) -> AppResult<()>                     // Err(AppError::ResizeOff) when off
// all #[tauri::command]s return AppResult<T> (or plain T where they returned plain T before)
```
```ts
// src/utils/errorText.ts — import type only (node --test)
export function errorText(e: unknown): string; // AppError → e.message; string → e; else String(e)
```

- [ ] **Step 1: Move tests first.** Every test in `store.rs`, `settings.rs`, `whats_new.rs` moves to the module owning the code, using a helper `fn db() -> (Connection, tempfile::TempDir)` built on `shared::db::open_in_memory()` + a temp images dir. Add:
  - `clip::tests::fourth_pin_is_pin_limit` — `set_pinned` on a 4th clip → `Err(AppError::PinLimit)`, nothing changed.
  - `shared::db::tests::v5_database_keeps_user_data` — build a v5 DB with the old inline SQL, insert one clip, one pinned clip, one excluded app, a `theme=dark` setting; open via `shared::db::open` and read all four back through the new domain APIs.
  - `utils/errorText.test.ts`: `{code:"INVALID_INPUT",message:"invalid setting a=b"}` → `"invalid setting a=b"`; `"x"` → `"x"`.
- [ ] **Step 2:** `cargo test --lib` and `yarn test` → FAIL (modules missing).
- [ ] **Step 3:** Implement the split. `setup` opens with `shared::db::open(&data_dir.join("vee.db"))` and `create_dir_all(images)`; the DB-open-failure dialog stays exactly as now. `.map_err(|e| e.to_string())` chains disappear in favor of `?` into `AppError`; plugin/OS errors become `AppError::Other(e.to_string())`.
- [ ] **Step 4:** Run all four checks except lint → pass; test count ≥ before + 3.
- [ ] **Step 5:** `yarn tauri dev` smoke: copy text, open panel, pin 4 clips (notice shows), type a bad value through devtools `__TAURI__.core.invoke('set_setting',{key:'theme',value:'x'})` → rejects `{code:"INVALID_INPUT",message:"invalid setting theme=x"}`.
- [ ] **Step 6:** Commit `refactor(rust): split domains and return AppError from commands`.

### Task 7: Frontend tree move (no styling or state change)

**Files (moves; CSS files move along untouched for now):**
- `src/main.tsx` → `src/app/main.tsx` (update `index.html` script src); `theme.css` → `src/app/theme.css` (temporary).
- `panel/Panel.tsx` → `app/panel/PanelApp.tsx`; `Card.tsx, FileIcon.tsx, Toolbar.tsx, ResizeHandle.tsx, cardImage.ts, motion.ts, panel.css` → `app/panel/_components/`.
- `settings/Settings.tsx` → `app/settings/SettingsApp.tsx`; `AppPicker, ConfirmModal, Dropdown, HistorySection, ShortcutRecorder, Tabs, settings.css` → `app/settings/_components/`.
- `toast/` → `app/toast/{ToastApp.tsx,toast.css}`; `whatsnew/` → `app/whats-new/{WhatsNewApp.tsx,whatsnew.css}`.
- Pure helpers + their tests → `src/utils/`: `keys.ts, selection.ts, fileThumb.ts, accelerator.ts, dropdownKeys.ts, notes.ts`, and the pure parts of `motion.ts` (`nextTilt, ghostLeaves, cursorCentredFrame, dragImageLayout` → `utils/motion.ts` with `motion.test.ts`); DOM parts stay in `app/panel/_components/motion.ts`.
- `i18n/` → `src/shared/i18n/` (`en.ts, ko.ts, index.ts`; `resolveLocale` → `locale.ts` with `$T`'s signature `resolveLocale(pref, systemLanguage)` + `locale.test.ts`).
- `api.ts` → per-domain wrappers, folder names equal to Rust: `domain/clip/apis/{clip.ts,clip.dto.ts}`, `domain/source_app/apis/{sourceApp.ts,sourceApp.dto.ts}`, `domain/settings/apis/{settings.ts,settings.dto.ts}`, `domain/updater/apis/{updater.ts,updater.dto.ts}`, `domain/whats_new/apis/{whatsNew.ts,whatsNew.dto.ts}`, plus `domain/panel/apis/panel.ts` for window commands (`hidePanel, setPanelPinned, resizePanel, resetPanelHeight, revealPanel, openSettings`) and `domain/settings/apis/settings.ts` also holds `getDiskAccess, openDiskAccessSettings, setAutostart, previewSound`. Each `domain/<name>/` gets `apis/ queries/ components/ store/` (`.gitkeep` where empty). Function names = today's `api.*` keys.
- Imports use `@/` everywhere except files under test (relative, `import type` for `@/`).
- `yarn test` glob already covers `src/**/*.test.ts`.

- [ ] **Step 1:** `git mv` everything; replace `api.x(...)` with the named import from its domain.
- [ ] **Step 2:** `yarn typecheck && yarn test && yarn build` → pass; `yarn tauri dev` smoke: all four windows open and look identical to baseline (CSS untouched).
- [ ] **Step 3:** Commit `refactor(frontend): move to app/domain/shared/utils layout`.

### Task 8: React Query, Zustand, providers, lint clean

**Files:**
- Create from `$T` (adapted): `src/shared/components/queries/RQProvider.tsx`, `src/shared/queries/index.ts`, `src/shared/store/{rootStore.ts,types.d.ts,error/…}`, `src/shared/providers/theme/ThemeProvider.tsx`, `src/shared/providers/i18n/I18nProvider.tsx`, `src/utils/theme.ts(+test)`.
- Create: `domain/settings/queries/settings.tsx`, `domain/clip/queries/clip.tsx`, `domain/source_app/queries/sourceApp.tsx`, `domain/updater/queries/updater.tsx`, `domain/whats_new/queries/whatsNew.tsx`, `domain/clip/store/{slice.ts,clip.state.ts}` (panel query/filter/app tag).
- Delete: `src/prefs.tsx`.

**Interfaces — Produces:**
```ts
export const SETTINGS_QUERY_KEY = ['settings'];           // get_settings → Settings DTO (single object, as today)
export function useSettings(): { settings: Settings | undefined; isLoading: boolean };
export function useSettingUpdate(): { updateSetting: (v: { key: SettingKey; value: string }) => void; isLoading: boolean };
// ThemeProvider/I18nProvider read useSettings(); both invalidate SETTINGS_QUERY_KEY on 'settings://changed'.
export function useI18n(): { locale: Locale; t: Dict };   // replaces usePrefs().t / .locale
// Zustand panel slice (kept between openings, never persisted):
interface ClipState { query: string; filter: Filter; app: App | null; setQuery(q: string): void; setFilter(f: Filter): void; setApp(a: App | null): void; resetSearch(): void }
```
Decisions:
- **Panel clip paging stays imperative** (direct `listClips/listPinned` calls with the existing `requestId`, FLIP and selection logic) — moving it into `useInfiniteQuery` would risk the race/animation behavior. Settings, stats, apps (`@` suggest, known/excluded), update status, what's-new, disk access, prunable counts use React Query.
- `main.tsx` seeds the settings query before first paint (`getSettings()` → `setQueryData`, apply `data-theme`/`lang`), replacing `PrefsProvider`'s `return null` gate; `data-window` attribute stays.
- `HistorySection`'s listeners (`clips://changed`, `clips://added`, `settings://changed`) invalidate the stats/excluded-apps queries instead of calling `refresh`.
- No error slice UI is added: vee shows errors where it shows them today. Skip `$T`'s `ErrorAlert`/`MutationCache.onError` toast.

- [ ] **Step 1:** Write `utils/theme.test.ts` (from `$T`) and `shared/i18n/locale.test.ts` → run → FAIL.
- [ ] **Step 2:** Implement providers, queries, slice; replace every `usePrefs()` use.
- [ ] **Step 3:** `yarn lint` — fix all findings in moved code (formatting via `yarn prettier`). Order components per `docs/agents/conventions/react-sections.md` (copied in Task 13; read it from `$T` now).
- [ ] **Step 4:** All four checks pass. `yarn tauri dev`: change theme and language in settings → panel/toast/what's-new follow live (Review Focus 4); panel keeps search/filter/app tag between openings and the reset shortcut clears them.
- [ ] **Step 5:** Commit `refactor(frontend): React Query, Zustand and template providers`.

### Task 9: `globals.css` and Tailwind base

**Files:** Create `src/app/globals.css`; delete `src/app/theme.css`; modify `src/app/main.tsx` import.

- [ ] **Step 1:** Start from `$T/src/app/globals.css` (palette already matches vee). Add the vee tokens it lacks as `--vee-*` per theme + `@theme inline` colors: `badge-bg, badge-fg, ok-bg, ok-fg, toast-bg, toast-fg, toast-border` and `--font-mono: Menlo, Consolas, monospace`, `--radius-box: 12px` equivalence. Values copied from today's `theme.css`.
- [ ] **Step 2:** Port `theme.css` base rules into `@layer base`: `* { box-sizing; margin:0 }`, `html,body,#root { height:100%; overflow: clip }`, transparent body for `html[data-window="panel"|"toast"]`, button reset, `:focus-visible` ring. The shared `.suggest*` popup becomes a component in `src/shared/components/suggest/Suggest.tsx` (used by panel search and `AppPicker`) styled with Tailwind classes.
- [ ] **Step 3:** Neutralize Tailwind preflight differences vs today (check: `img` display, `button` padding, `input` appearance, `color-scheme`) by comparing against baseline.
- [ ] **Step 4:** Checks pass; screenshots of all windows match baseline. Commit `style: move theme tokens into globals.css`.

### Task 10: Tailwind — toast and what's-new

**Files:** `app/toast/ToastApp.tsx` (delete `toast.css`), `app/whats-new/WhatsNewApp.tsx` (delete `whatsnew.css`); keyframes `toast-in`, `toast-out`, `toast-check-in` move to `globals.css` (`@keyframes` + `@theme` `--animate-*`).

- [ ] **Step 1:** Rewrite with Tailwind utilities (`classnames` for conditional classes), daisyUI only where identical.
- [ ] **Step 2:** Screenshot both windows in 4 combos, compare side by side with baseline → no visible difference (including toast enter/exit animation timing: 180 ms exit).
- [ ] **Step 3:** Checks pass. Commit `style(toast,whats-new): rewrite with Tailwind`.

### Task 11: Tailwind — settings window

**Files:** `app/settings/SettingsApp.tsx` and `_components/*` (delete `settings.css`). Reuse `$T/src/shared/components/settings/*` only where the result is pixel-identical; otherwise build vee-shaped components there.

- [ ] **Step 1:** Rewrite. Error text keeps `role="alert"` and the shortcut-recorder padding exception (commit 9084588).
- [ ] **Step 2:** Screenshots: every tab, open dropdown, confirm modal, app picker suggest, shortcut recording/error state, 4 combos → match baseline.
- [ ] **Step 3:** Checks pass. Commit `style(settings): rewrite with Tailwind`.

### Task 12: Tailwind — panel

**Files:** `app/panel/PanelApp.tsx`, `_components/{Card,FileIcon,Toolbar,ResizeHandle}.tsx` (delete `panel.css`).

- [ ] **Step 1:** Keep the DOM hooks other code reads: class `card` + `data-id` on cards, class `update` on the update banner, class `ghost` on the drag ghost (style it in `globals.css` `@layer components` since `motion.ts` adds it imperatively). `cardImage.ts` copies computed styles, so drag images follow automatically — verify by dragging.
- [ ] **Step 2:** `grep -rn '"\.card\[data-id\]"\|closest("\.update")\|classList.add("ghost")' src` still finds the three uses, and each target class is present in the JSX/CSS.
- [ ] **Step 3:** Screenshots: empty, many clips, pinned area, each filter, `@app` suggest, app tag chip, reset chip, pin notice, update banner + confirm, delete confirm, resize handle hover, 4 combos → match baseline. Exercise: open/close animation, FLIP on new copy, hover select, drag a text/file/image card out, scroll-load page 2.
- [ ] **Step 4:** `ls src/**/*.css` → only `src/app/globals.css`. Checks pass. Commit `style(panel): rewrite with Tailwind`.

### Task 13: Docs

**Files:**
- Create: `AGENTS.md`, `CLAUDE.md`, `docs/agents/**` (copy `$T`, then adapt)
- Modify: `docs/development.md`, `docs/release.md`, `README.md`
- Create: `docs/superpowers/notes/2026-10-16-apply-template-followups.md` (if anything was noted)

- [ ] **Step 1:** Adapt per spec §2: `overview.md` (Vee clipboard history app, 4 windows, tray-only, macOS Apple Silicon + Windows), `structure.md`/`architecture.md` (real trees from Tasks 5–12, domains `clip, source_app, settings, whats_new` + frontend `updater`, `panel`), `tech-stack.md` (vee-only deps, TS 6 note), `libs/react-query.md` (the imperative panel paging exception, `settings://changed` invalidation), `conventions/sqlite-migrations.md` (no `SchemaTooNew`: newer DBs open), `conventions/error-handling.md` (vee's `AppError` variants/codes), `workflow/{branching,versioning,release,pr}.md` (single `master`; `feature/<topic>` from and to master; no hotfix/develop), `CLAUDE.md` last bullet → "GitHub Actions runs it on every merge into master". Remove template-only content (placeholder renaming, Greeting, app menu).
- [ ] **Step 2:** `docs/development.md`: add `GITHUB_NPM_AUTH_TOKEN` setup (GitHub PAT with `read:packages`; macOS `export` in shell profile; Windows `setx GITHUB_NPM_AUTH_TOKEN …`), `yarn lint`, `yarn prettier`. `docs/release.md`: step 1 becomes "merge to master; Actions bumps the version — pull and confirm `tauri.conf.json`", add the token to "준비". Update paths in `README.md`.
- [ ] **Step 3:** Link check: `grep -rnoE '\]\([^)#]+' AGENTS.md docs/agents | sed …` — every relative link resolves to an existing file. `grep -rn "develop\|main\b\|template-rust-app\|Greeting" AGENTS.md CLAUDE.md docs/agents` → no stale hits.
- [ ] **Step 4:** Commit `docs: adopt template agent docs for vee`.

### Task 14: Final verification

- [ ] **Step 1:** `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` → all pass; paste the result lines into the PR.
- [ ] **Step 2:** Fresh-profile check: move `~/Library/Application Support/<identifier>` aside, launch, confirm first-run works; restore it, launch, confirm existing history/pins/settings intact (Review Focus 1).
- [ ] **Step 3:** Full 4-combo screenshot pass of all windows vs baseline once more; attach a few pairs to the PR.
- [ ] **Step 4:** Remove `/private/tmp/vee-master` worktree. Open the PR (title without `[minor]`/`[major]` → patch bump on merge unless the user says otherwise).
