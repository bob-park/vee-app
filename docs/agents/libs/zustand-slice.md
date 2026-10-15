---
title: Zustand Slice Pattern
scope: src/domain/*/store/**, src/shared/store/**
applies_to: client-only UI state shared across components of one window
related:
  - ../structure.md
  - ../conventions/react-sections.md
  - ./react-query.md
---

# Zustand Slice Pattern

> Zustand is for **client-only UI state**, such as an open panel, a selection or a draft. Data that lives in Rust/SQLite belongs in React Query, not Zustand. Each domain exposes a `SlicePattern<DomainState, BoundState>` slice that is combined in `rootStore.ts`. `immer` and `devtools` (dev only, store name `vee`) are enabled.

- `SlicePattern` comes from a module augmentation in `src/shared/store/types.d.ts` and is imported as if from `zustand`.
- There is one slice today: `src/domain/clip/store/` (`clip.state.ts` + `slice.ts`). It holds the panel's search — `query`, `filter`, the `@app` tag — with `setQuery`, `setFilter`, `setApp` and `resetSearch`. The panel window is only hidden, never destroyed, so this survives between openings and resets on restart.
- Everything else in the panel (selection, paging, drag, confirm state) is local `useState`/`useRef` in `PanelApp`; promote it to a slice only when a second component needs it.
- Domain slices go in `src/domain/<name>/store/`. Only cross-cutting UI state would live in `src/shared/store/` (there is none; errors are shown inline, not through a store).

## Adding a slice

```ts
// src/domain/clip/store/clip.state.ts
export interface ClipState {
  query: string;
  setQuery: (query: string) => void;
  // …
}

// src/domain/clip/store/slice.ts
const createClipSlice: SlicePattern<ClipState, BoundState> = (set) => ({
  query: '',
  setQuery: (query) =>
    set((state) => {
      state.query = query;
    }, false, 'clip/setQuery'),
});
export default createClipSlice;

// src/shared/store/rootStore.ts — add a new slice next to it
immer((...a) => ({ ...createClipSlice(...a), ...createFooSlice(...a) }))
export type BoundState = ClipState & FooState;
```

## Rules

- **Action names:** use `domain/actionName` as the devtools action type (third `set` argument).
- **Updating state:** inside `set`, either mutate the immer draft or return a new object. Don't mix the two styles in one slice.
- **Selectors:** select the narrowest value in components (`useStore((s) => s.query)`), under the `// store` section.
- **Windows:** the store is per window ([react-query](./react-query.md#multiple-windows)). State that must survive a restart or be shared between windows belongs in SQLite.
