---
title: Versioning
scope: "**"
applies_to: reading or changing the app version, cutting a release
related:
  - ./pr.md
  - ./release.md
---

# Versioning

> Plain semver `X.Y.Z`. `src-tauri/tauri.conf.json` is the source of truth. `bump-version-up.sh` keeps four files in sync; **never edit versions by hand**. GitHub Actions runs it on every merge into `master`.

## Files carrying the version

`src-tauri/tauri.conf.json` (source of truth) · `package.json` · `src-tauri/Cargo.toml` (`[package]`) · `src-tauri/Cargo.lock` (the `vee-app` entry).

## `bump-version-up.sh`

```bash
./bump-version-up.sh [patch|minor|major]   # default: patch
```

- The script validates the version and the argument first. It writes all four files or none: after writing, it checks each file. If one wasn't updated (e.g. `CRATE_NAME` no longer matches the crate, so `Cargo.lock` had no match), it restores the backups and exits non-zero.
- `CRATE_NAME="vee-app"` must equal `[package] name` in `src-tauri/Cargo.toml`.
- It uses `perl -i`, so it works on macOS and on the Ubuntu runner.
- It is tested by `scripts/bump-version-up.test.mjs`.
- No `-rcN`/date suffixes: the Tauri updater compares semver, and prerelease tags would break update ordering.

## Workflow (`.github/workflows/bump-version.yml`)

1. **Trigger:** a push to `master`, which happens when a PR merges. The bot's own commit carries `[skip ci]` and is skipped.
2. **Reading the PR:** the workflow finds the PR number from the merge or squash commit, then reads the PR title and labels ([pr](pr.md#title)). No PR number → patch.
3. **Commit:** it runs the script and commits `chore: bump version to X.Y.Z [skip ci]` to `master`.
4. **Close merges:** runs are serialized (`concurrency`). Each run checks out the branch head, so a merge that lands right after another bumps on top of it instead of being rejected.

**Repository setup:**

- Settings → Actions → Workflow permissions must be **Read and write**.
- If `master` is protected, allow GitHub Actions to bypass the protection.

## Releases

Release tags are `vX.Y.Z`, cut from `master` after the bump commit by the release scripts ([release](release.md)). Pull first so the local `tauri.conf.json` has the bumped version.
