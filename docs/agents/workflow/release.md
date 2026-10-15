---
title: Release
scope: scripts/**, docs/release.md
applies_to: cutting a release, touching release scripts or updater config
related:
  - ./versioning.md
  - ../configuration.md
---

# Release

> Releases are built, signed and uploaded **locally** from `master`: `scripts/release.sh` (macOS Apple Silicon: sign and notarize) and `scripts/release.ps1` (Windows: updater `.sig` only). `scripts/latest-json.mjs` then assembles `latest.json`. There is no CI build. The full procedure, in Korean, is in [docs/release.md](../../release.md).

## Steps

1. Merge to `master`; Actions bumps the version ([versioning](versioning.md)). Pull and confirm `version` in `src-tauri/tauri.conf.json`.
2. On macOS (Apple Silicon), run `scripts/release.sh`. It uploads `Vee_<v>_aarch64.dmg`, `Vee_<v>_aarch64.app.tar.gz` and its `.sig` to the draft release `v<v>`.
3. On Windows, run `pwsh scripts/release.ps1`. It uploads `Vee_<v>_x64-setup.exe` and its `.sig`.
4. Write the release notes; they become the update notes and the **what's-new window** text. Then run `node scripts/latest-json.mjs v<v>`.
5. Publish with `gh release edit v<v> -R bob-park/vee-app --draft=false`.

macOS may be published first and Windows added to the same release later ([docs/release.md](../../release.md#macos만-먼저-게시하기)).

## Rules for agents

- **Never run the release scripts or `gh release` commands without the user's explicit request.** They publish to users.
- If you change `productName`, update the file names in `release.sh` and `release.ps1`. `latest-json.mjs` matches on suffix only.
- Both scripts refuse to run when `v<v>` is already **published** with that platform's build. A shipped version is never overwritten; merge a new PR so the version is bumped instead. Adding a platform that is still missing from a published release is allowed.
- `latest-json.mjs` refuses to run while `plugins.updater.pubkey` is empty. Don't remove that guard.
- The updater only accepts a `latest.json` whose listed platforms are all complete. That's why the script includes only the platforms that were uploaded.
