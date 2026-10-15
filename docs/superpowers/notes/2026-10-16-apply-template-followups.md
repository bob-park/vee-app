# Apply-template follow-ups

## Task 7 (frontend tree move)
- `src/app/panel/_components/{motion.ts,cardImage.ts}` import `../../../utils/motion.ts` relatively: `_components/motion.test.ts` runs `floatCard` under `node --test`, which cannot resolve `@/`. This will trip the `../` ESLint rule in Task 8 — either move the DOM test, inject the pure helpers, or exempt these two files.
- `HistorySection` imports `Segmented` from `SettingsApp` (import cycle); candidate for `shared/components` (Task 9+).
- Screen and component files still use named exports; template convention is default export.
- `src/prefs.tsx` stays at `src/` (brief has no destination; removed in Task 8).

## Task 8 (React Query, Zustand, providers)
- `ReactQueryDevtools` renders in the dev settings window only, in its bottom-right ~55pt corner. Dev settings screenshots differ from the Task 1 baseline there, so mask that corner when comparing.
- `ShortcutRecorder.onSave` needs a Promise, so the three shortcut saves still call `setShortcut`/`setSetting` directly. So do the other non-key/value writes (`setAutostart`, `setAppExcluded`, `resetPanelHeight`, `clearHistory`, `checkUpdate`/`installUpdate`), through `run()`. Every one of them emits `settings://changed` or `clips://*`, so the caches still refresh. Candidates for mutation hooks.
- `react-sections` order can't hold in `PanelApp`: the `useCallback`s (`reload`, `loadMore`) must come before the effects that list them as deps. The `callback` section sits before `useEffect` there.
