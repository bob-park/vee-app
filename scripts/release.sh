#!/usr/bin/env bash
# Builds, signs and notarizes the macOS app for both architectures, then uploads
# the bundles and merges darwin entries into latest.json on a draft release.
set -euo pipefail
cd "$(dirname "$0")/.."

set -a
# shellcheck disable=SC1091
source "$HOME/.config/vee/sign.env"
set +a

REPO="bob-park/vee-app"
VERSION=$(jq -r .version src-tauri/tauri.conf.json)
TAG="v$VERSION"
BASE_URL="https://github.com/$REPO/releases/download/$TAG"

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  yarn tauri build --target "$target"
done

gh release view "$TAG" -R "$REPO" >/dev/null 2>&1 \
  || gh release create "$TAG" -R "$REPO" --draft --title "Vee $TAG" --notes "Vee $TAG"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
latest="$work/latest.json"
if ! gh release download "$TAG" -R "$REPO" -p latest.json -D "$work" 2>/dev/null; then
  jq -n --arg v "$VERSION" --arg d "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{version: $v, notes: ("Vee v" + $v), pub_date: $d, platforms: {}}' > "$latest"
fi

for entry in "aarch64-apple-darwin:darwin-aarch64:aarch64" "x86_64-apple-darwin:darwin-x86_64:x64"; do
  IFS=: read -r target platform arch <<< "$entry"
  bundle="src-tauri/target/$target/release/bundle"
  # Both architectures produce Vee.app.tar.gz; rename so they can live side by side.
  tarball="$work/Vee_${VERSION}_${arch}.app.tar.gz"
  cp "$bundle/macos/Vee.app.tar.gz" "$tarball"
  dmg=$(ls "$bundle"/dmg/*.dmg)
  gh release upload "$TAG" -R "$REPO" --clobber "$tarball" "$dmg"
  jq --arg p "$platform" --arg sig "$(cat "$bundle/macos/Vee.app.tar.gz.sig")" \
     --arg url "$BASE_URL/$(basename "$tarball")" \
     '.platforms[$p] = {signature: $sig, url: $url}' "$latest" > "$latest.tmp"
  mv "$latest.tmp" "$latest"
done

gh release upload "$TAG" -R "$REPO" --clobber "$latest"
echo "Draft $TAG updated with macOS builds."
echo "After the Windows upload, publish with: gh release edit $TAG -R $REPO --draft=false"
