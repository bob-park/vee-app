---
title: TypeScript
scope: src/**/*.{ts,tsx}
applies_to: type declarations, component props, DTOs
related:
  - ./naming.md
  - ./module-aliases.md
---

# TypeScript

> `strict: true` and `verbatimModuleSyntax`. Use `interface` for extensible object shapes and `type` for unions, mapped types and DTOs. Component props are `Readonly<{...}>` or a named `interface`. Use `import type` for types.

- `strict`, `noUnusedLocals` and `noUnusedParameters` are on. Assume non-null and exhaustive checks.
  - A `switch` over a union returns in every case; TS then rejects a missing case.
  - See `UpdateRow` in `SettingsApp.tsx` (over `UpdateStatus`).
- **`interface`** is for object shapes that may be extended, such as `AppError` and `QueryMutationHandle<T>`.
- **`type`** is for unions, mapped types and DTO aliases, such as `Filter`, `SettingKey` and `UpdateStatus`.
- **Props:**
  - Inline: `Readonly<{ children: React.ReactNode }>`.
  - Named: `interface XxxProps { … }` declared next to the component.
- **DTOs mirror the Rust `serde` output exactly.**
  - Tagged enums become discriminated unions on the tag field, e.g. `status`.
  - Field names follow the Rust `rename_all`.
- **Dictionaries:** `en.ts` is the source type (`Dict = typeof en`); every other locale is annotated `: Dict`.
- **Unsafe types:**
  - Never use `any`.
  - Use `unknown` and narrow it, like `toAppError` in `shared/api`.
  - Cast with `as` only at a validated boundary.
