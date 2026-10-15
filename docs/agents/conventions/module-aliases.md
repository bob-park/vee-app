---
title: Module & Path Aliases
scope: src/**/*.{ts,tsx}
applies_to: import statements across folders
related:
  - ./naming.md
  - ../structure.md
  - ../testing.md
---

# Module & Path Aliases

> Cross-folder imports use `@/...` (`src/*`). Same-folder imports are relative (`./x`). `../` imports are rejected by ESLint. Import grouping and order are enforced by the bobpark ESLint/Prettier config.

- `@/*` maps to `./src/*` in both `tsconfig.json` (`paths`) and `vite.config.ts` (`resolve.alias`). Keep them in sync.
- Only same-folder imports are relative. ESLint's `no-restricted-imports` forbids `../`.
- Type-only imports use `import type { … }`, because `verbatimModuleSyntax` is on.
- **`node --test` does not resolve `@/`.** A file under test (`src/utils/*.ts`, `src/shared/i18n/locale.ts`, the two panel files below) may import from `@/…` **only with `import type`**, which is erased at runtime. Its test file imports it relatively (`./theme.ts`).
- **The one exception:** `src/app/panel/_components/{motion,cardImage}.ts` import `../../../utils/motion.ts` relatively, because `_components/motion.test.ts` runs them under `node --test`. `eslint.config.mjs` has a file-scoped override that drops the `../` rule for exactly these two files. Don't widen it; prefer keeping tested logic in `src/utils/`.
- Don't override the import order locally. Run `yarn prettier` or `yarn lint --fix`.
