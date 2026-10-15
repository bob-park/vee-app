---
title: Commit Message Convention
scope: "**"
applies_to: writing a commit message
related:
  - ./branching.md
  - ./pr.md
---

# Commit Message Convention

> `<prefix>: <subject>` or `<prefix>(<scope>): <subject>` — one line, imperative mood, lowercase prefix. The optional scope names the area (`panel`, `settings`, `watcher`, `rust`, `frontend`). Optional body after a blank line explains *why*.

| Prefix | Use for |
| ------ | ------- |
| `feat` | New feature. |
| `fix` | Bug fix on existing functionality. |
| `refactor` | Restructuring with no behavior change. |
| `style` | Styling-only change (CSS/Tailwind classes) with no visual or behavior change. |
| `build` | Build tooling, dependencies, release/version scripts. |
| `ci` | GitHub Actions workflows. |
| `docs` | Documentation or comment-only changes. |
| `test` | Adding or modifying tests. |
| `chore` | Maintenance with no code impact — e.g. the bot's `chore: bump version to X.Y.Z [skip ci]`. |

Examples:

```text
feat(panel): select cards on hover
fix(settings): open the settings window on the current desktop
refactor(rust): split domains and return AppError from commands
ci: bump the version on every merge into master
docs: list full disk access under the history tab
```

- `[skip ci]` is reserved for the version-bump bot; it stops the bump workflow from re-triggering.
- Body lines wrap at ~72 characters.
