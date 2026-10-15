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

## Task 9 (globals.css, Tailwind base)
- `--border` clash: daisyUI themes set `--border: 1px`, but the unlayered legacy block in `globals.css` redefines `--border` as a color for the per-screen CSS. Until that legacy block is deleted (Tasks 10–12), any daisyUI component added would read a color as its border width and break silently.
- The daisyUI `exclude` list hides components whose names collide with per-screen classes (`badge, button, card, dropdown, kbd, link, modal, select, stack, stat, tab, toast`). Remove each entry once no screen uses that class name; keep `rootcolor, rootscrollgutter, rootscrolllock, scrollbar`.
- The second `@layer base` block in `globals.css` holds temporary preflight reverts (`input` padding, `::placeholder`, `ul` list-style, `img` max-width). Delete each one as the screens using that element move to Tailwind.
- `docs/development.md` "프로젝트 구조" still lists `src/theme.css`; the whole tree there is stale since Task 4.

## Task 10 (toast, what's-new on Tailwind)
- With `toast` out of the daisyUI `exclude` list, daisyUI emits its `.toast` component CSS (the word "toast" appears in the source, e.g. the window label). No element uses that class, so it is dead CSS only.
- The shared prettier config sorts Tailwind classes without `tailwindStylesheet`, so vee's own token utilities (`bg-toast-bg`, `shadow-toast`, `animate-toast-in`, …) sort as unknown classes ahead of the built-ins. Cosmetic; set `tailwindStylesheet: './src/app/globals.css'` if the order matters.
- `yarn prettier` is `prettier --write`, so `yarn prettier --check` also rewrites files.
