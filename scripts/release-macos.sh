#!/usr/bin/env bash
# Builds the macOS app for Apple Silicon (Intel Macs are not supported), signs it with
# Developer ID and notarizes both the .app and the .dmg, then uploads the dmg, the updater
# bundle and its .sig to a draft release. latest.json is assembled afterwards by
# scripts/latest-json.mjs. `--build-only` stops after notarizing and uploads nothing.
# Notarization uses an App Store Connect API key: key ID and issuer ID come from the
# login keychain, the .p8 from ~/.config/apple.
set -euo pipefail
cd "$(dirname "$0")/.."
kc() { security find-generic-password -a "$USER" -s "$1" -w; }

build_only=false
[ "${1:-}" = "--build-only" ] && build_only=true

set -a
# shellcheck disable=SC1091
source "$HOME/.config/apple/.env" # APPLE_SIGNING_IDENTITY
# shellcheck disable=SC1091
source "$HOME/.config/vee/sign.env" # TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)
set +a

# Assign before export: `export X="$(kc …)"` would hide a failed lookup from set -e.
APPLE_API_KEY=$(kc tauri-apple-api-key)
APPLE_API_ISSUER=$(kc tauri-apple-api-issuer)
APPLE_API_KEY_PATH="$HOME/.config/apple/AuthKey_${APPLE_API_KEY}.p8"
[ -f "$APPLE_API_KEY_PATH" ] || { echo "missing $APPLE_API_KEY_PATH" >&2; exit 1; }
export APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH

REPO="bob-park/vee-app"
VERSION=$(node -p 'require("./src-tauri/tauri.conf.json").version')
TAG="v$VERSION"

if ! $build_only; then
  # A published release that already has this platform's build is shipped: overwriting it would
  # change what users installed without a version bump. (Adding a missing platform is fine.)
  shipped=$(gh release view "$TAG" -R "$REPO" --json isDraft,assets \
    -q 'select(.isDraft == false) | .assets[].name | select(endswith("_aarch64.app.tar.gz"))' 2>/dev/null || true)
  if [ -n "$shipped" ]; then
    echo "$TAG is already published with a macOS build ($shipped). Bump the version first (docs/release.md)." >&2
    exit 1
  fi
fi

yarn tauri build --bundles app,dmg --target aarch64-apple-darwin

bundle="src-tauri/target/aarch64-apple-darwin/release/bundle"
# Exact names: old versions' bundles stay in target/ after a version bump.
dmg="$bundle/dmg/Vee_${VERSION}_aarch64.dmg"
[ -f "$dmg" ] || { echo "missing $dmg" >&2; exit 1; }

# Tauri notarizes only the .app; notarize and staple the dmg too so Gatekeeper accepts it.
xcrun notarytool submit "$dmg" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" \
  --issuer "$APPLE_API_ISSUER" --wait
# notarytool exits 0 even when Apple rejects the file; stapling fails if there is no ticket.
xcrun stapler staple "$dmg"

if $build_only; then
  echo "Built and notarized $dmg (nothing uploaded)."
  exit 0
fi

gh release view "$TAG" -R "$REPO" >/dev/null 2>&1 \
  || gh release create "$TAG" -R "$REPO" --draft --title "Vee $TAG" --notes "Vee $TAG"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# The updater bundle is always named Vee.app.tar.gz; version it for the release.
tarball="$work/Vee_${VERSION}_aarch64.app.tar.gz"
cp "$bundle/macos/Vee.app.tar.gz" "$tarball"
cp "$bundle/macos/Vee.app.tar.gz.sig" "$tarball.sig"

gh release upload "$TAG" -R "$REPO" --clobber "$dmg" "$tarball" "$tarball.sig"
echo "Draft $TAG updated with the macOS build."
echo "After the Windows upload: node scripts/latest-json.mjs $TAG, then gh release edit $TAG -R $REPO --draft=false"
