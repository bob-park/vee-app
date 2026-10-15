---
title: Branch Naming
scope: "**"
applies_to: starting a branch
related:
  - ./pr.md
  - ./versioning.md
  - ./commit-messages.md
---

# Branch Naming

> One long-lived branch: `master`. Work on `feature/<topic>` (lowercase kebab-case), branched from `master` and merged back into `master` through a PR. Every merge bumps the version.

| Branch | Purpose |
| ------ | ------- |
| `master` | The only long-lived branch. Every merge bumps the version ([versioning](versioning.md)); releases are built from it ([release](release.md)). |
| `feature/<topic>` | Any change — features, fixes, refactors, docs. Branch from `master`, PR back to `master`. |

Rules:

- `<topic>` is lowercase kebab-case (`feature/note-search`, `feature/apply-template`).
- No direct commits to `master` — always a PR. The only direct commit is the bot's `chore: bump version to X.Y.Z [skip ci]`.
- There is no separate release or hotfix branch: an urgent fix is just a `feature/*` PR whose merge bumps the patch version.
- Release tags are `v<X.Y.Z>` and are created by the release scripts.
