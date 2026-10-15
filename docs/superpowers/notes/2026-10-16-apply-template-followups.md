# Apply-template follow-ups

## Task 7 (frontend tree move)
- `src/app/panel/_components/{motion.ts,cardImage.ts}` import `../../../utils/motion.ts` relatively: `_components/motion.test.ts` runs `floatCard` under `node --test`, which cannot resolve `@/`. This will trip the `../` ESLint rule in Task 8 — either move the DOM test, inject the pure helpers, or exempt these two files.
- `HistorySection` imports `Segmented` from `SettingsApp` (import cycle); candidate for `shared/components` (Task 9+).
- Screen and component files still use named exports; template convention is default export.
- `src/prefs.tsx` stays at `src/` (brief has no destination; removed in Task 8).
