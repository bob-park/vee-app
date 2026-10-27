# Source this file (don't run it): exports the Tauri updater signing key and its password
# from the login keychain (services tauri-updater-vee / tauri-updater-vee-pw).
# Used by scripts/release-macos.sh; source it by hand before a manual `yarn tauri build`.
# shellcheck shell=bash

kc() { security find-generic-password -a "$USER" -s "$1" -w; }

APP_NAME=vee
# Assign before export: `export X="$(kc …)"` would hide a failed lookup from set -e.
TAURI_SIGNING_PRIVATE_KEY=$(kc "tauri-updater-$APP_NAME")
TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$(kc "tauri-updater-$APP_NAME-pw")
export TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
