---
title: React Query
scope: src/domain/*/queries/**, src/shared/components/queries/**, src/app/main.tsx
applies_to: reading or writing data through Tauri commands, caching, cross-window sync
related:
  - ../conventions/tauri-commands.md
  - ../conventions/error-handling.md
  - ../conventions/naming.md
---

# React Query

> Rust data goes through React Query hooks in `src/domain/<name>/queries/<name>.tsx`, which wrap `apis/<name>.ts`: settings, stats, source apps, update status, what's new, disk access. One `queryClient` per window (`RQProvider.tsx`). A change made in one window reaches the others through a Tauri event that invalidates or patches the cache. **Exception:** the panel's clip list is paged imperatively, not through React Query.

## Reads

```tsx
export function useSettings() {
  const { data, isLoading } = useQuery<Settings>({ queryKey: SETTINGS_QUERY_KEY, queryFn: getSettings });
  return { settings: data, isLoading };
}
```

- Return a small object with defaults applied where a default makes sense (`data ?? []`, `data ?? null`), not the raw query result.
- Query keys are `[domain, resource, ...specifier]`.
- Data that can change while nothing is listening (the settings tab was hidden, the tray ran a check) uses `staleTime: 0`, so it refetches on mount (`useStats`, `useKnownApps`, `useUpdateStatus`, `useDiskAccess`).
- One-off counts that must never be cached (`usePrunableCount`, `useAppClipCount`) return a `fetchXxx` function built on `queryClient.fetchQuery({ staleTime: 0 })`.

## Writes

```tsx
export function useSettingUpdate({ onSuccess, onError }: QueryMutationHandle<void> = {}) {
  const { mutate, isPending } = useMutation<void, AppError, { key: SettingKey; value: string }>({
    mutationKey: ['settings', 'update'],
    mutationFn: ({ key, value }) => setSetting(key, value),
    // set_setting emits `settings://changed`, which refetches the settings in every window.
    onSuccess: () => onSuccess?.(),
    onError: (err) => onError?.(err),
  });
  return { updateSetting: mutate, isLoading: isPending };
}
```

- When the command returns the new value, write it into the cache with `setQueryData`. When Rust emits an event for the change, let the event's listener refresh the cache instead (as here).
- Accept `QueryMutationHandle<T>` so callers can react to success or errors.
- Some writes still call `apis/` directly: shortcut saves (`ShortcutRecorder` needs a Promise), and autostart, exclusions, clear history and panel-height reset through `SettingsApp`'s `run()` helper; `UpdateRow` and the panel call the update check/install wrappers. Each emits an event that refreshes the caches. Prefer a mutation hook for new writes.

## Defaults (`RQProvider.tsx`)

- `staleTime` 60 s, `gcTime` 120 s, `refetchOnWindowFocus: false`, `retryOnMount: true`.
- `refetchOnReconnect` is omitted: there is no network between the webview and Rust.
- Devtools render only in dev **and** only in the `settings` window (`main.tsx`); in the panel, toast and what's-new windows the button would cover content.

## Multiple windows

Each Tauri window is a separate JS context, so it has **its own `queryClient` and Zustand store**. Nothing is shared automatically.

- When a command changes data another window shows, have Rust `emit` an event (`<domain>://<event>`).
- Every window listens once, in a provider or hook:
  - `settings://changed` → `ThemeProvider` and `I18nProvider` both call `invalidateQueries({ queryKey: SETTINGS_QUERY_KEY })`, so every window refetches settings (theme, language, shortcuts, sounds …) live.
  - `update://status` → `useUpdateStatus` calls `setQueryData` with the payload.
  - `clips://changed`, `clips://added`, `settings://changed` → `HistorySection` invalidates stats and app lists.

## The panel clip list

`PanelApp` loads clips with `listClips` / `listPinned` from `apis/clip.ts` directly, on purpose:

- **Superseded requests:** every reload bumps a `requestId` ref; a response whose id is stale is dropped, so typing fast never shows an older search's results.
- **FLIP animation:** before applying a new page, it measures card positions (`cardLefts`) so new copies slide in.
- **Selection:** a reload either keeps the selected clip (`followSelection`) or resets it, depending on why it reloaded.
- **Paging:** `loadMore` appends the next page while scrolling, guarded by the same `requestId`.

These are ordering and DOM-timing concerns a cache doesn't model. Keep this list imperative; don't move it into a query. Everything else in the panel (settings, `@app` suggestions, update status) uses hooks.

## First paint

`main.tsx` awaits `getSettings()`, seeds it with `queryClient.setQueryData(SETTINGS_QUERY_KEY, …)`, applies the theme and `<html lang>` **before** `render`, then mounts the providers. On failure the query fetches on mount instead.
