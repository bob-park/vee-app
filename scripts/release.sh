#!/usr/bin/env bash
# Builds, signs and notarizes the macOS app for Apple Silicon (Intel Macs are not
# supported), then uploads the dmg, the updater bundle and its .sig to a draft
# release. latest.json is assembled afterwards by scripts/latest-json.mjs.
set -euo pipefail
cd "$(dirname "$0")/.."

set -a
# shellcheck disable=SC1091
source "$HOME/.config/vee/sign.env"
set +a

REPO="bob-park/vee-app"
VERSION=$(node -p 'require("./src-tauri/tauri.conf.json").version')
TAG="v$VERSION"

yarn tauri build --target aarch64-apple-darwin

gh release view "$TAG" -R "$REPO" >/dev/null 2>&1 \
  || gh release create "$TAG" -R "$REPO" --draft --title "Vee $TAG" --notes "Vee $TAG"

bundle="src-tauri/target/aarch64-apple-darwin/release/bundle"
# Exact names: old versions' bundles stay in target/ after a version bump.
dmg="$bundle/dmg/Vee_${VERSION}_aarch64.dmg"
[ -f "$dmg" ] || { echo "missing $dmg" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# The updater bundle is always named Vee.app.tar.gz; version it for the release.
tarball="$work/Vee_${VERSION}_aarch64.app.tar.gz"
cp "$bundle/macos/Vee.app.tar.gz" "$tarball"
cp "$bundle/macos/Vee.app.tar.gz.sig" "$tarball.sig"

gh release upload "$TAG" -R "$REPO" --clobber "$dmg" "$tarball" "$tarball.sig"
echo "Draft $TAG updated with the macOS build."
echo "After the Windows upload: node scripts/latest-json.mjs $TAG, then gh release edit $TAG -R $REPO --draft=false"
