---
title: Pull Requests
scope: "**"
applies_to: opening a PR or titling one
related:
  - ./branching.md
  - ./versioning.md
  - ./commit-messages.md
---

# Pull Requests

> Every PR targets `master`. The PR title (or a `major`/`minor` label) decides the version bump when it merges.

## Base Branch

Always `master` ([branching](branching.md)).

## Title

A short, imperative summary. **Do not** prefix it with `feat:`/`fix:` — those belong on commits.

Merging into `master` runs `.github/workflows/bump-version.yml`:

| PR title contains / label | Bump |
| ------------------------- | ---- |
| `[major]` or label `major` | `major` += 1, minor and patch reset to 0 |
| `[minor]` or label `minor` | `minor` += 1, patch reset to 0 |
| anything else | `patch` += 1 |

Examples:

```text
fix drag image offset on Windows
[minor] add clip notes
[major] change history storage format
```

Both merge commits (`Merge pull request #N …`) and squash merges (`Title (#N)`) are recognized. A push to `master` with no PR number bumps the patch version.

## Body

- What changed and why, how it was verified (the four checks in [testing](../testing.md); for UI, light/dark × ko/en).
- Follow-ups found but deliberately not fixed in this PR.
