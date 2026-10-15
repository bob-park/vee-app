# Apply-template follow-ups

## Task 7 (frontend tree move)
- `src/app/panel/_components/{motion.ts,cardImage.ts}` import `../../../utils/motion.ts` relatively: `_components/motion.test.ts` runs `floatCard` under `node --test`, which cannot resolve `@/`. This will trip the `../` ESLint rule in Task 8 — either move the DOM test, inject the pure helpers, or exempt these two files.
- `HistorySection` imports `Segmented` from `SettingsApp` (import cycle); candidate for `shared/components` (Task 9+).
- Screen and component files still use named exports; template convention is default export.
- `src/prefs.tsx` stays at `src/` (brief has no destination; removed in Task 8).

## Task 8 (React Query, Zustand, providers)
- `ReactQueryDevtools` (copied from the template, `import.meta.env.DEV` only) draws its floating button in the bottom-right corner of every dev window, panel and toast included. Production builds leave it out, but dev screenshots no longer match the Task 1 baseline in that ~55pt corner. Later tasks should mask that corner, or the button should be turned off for vee's windows.
- `ShortcutRecorder.onSave` needs a Promise, so the three shortcut saves still call `setShortcut`/`setSetting` directly. So do the other non-key/value writes (`setAutostart`, `setAppExcluded`, `resetPanelHeight`, `clearHistory`, `checkUpdate`/`installUpdate`), through `run()`. Every one of them emits `settings://changed` or `clips://*`, so the caches still refresh. Candidates for mutation hooks.
- `shared/store/error` slice exists (template parity) but nothing reads or writes it yet.
- `react-sections` order can't hold in `PanelApp`: the `useCallback`s (`reload`, `loadMore`) must come before the effects that list them as deps. The `callback` section sits before `useEffect` there.
