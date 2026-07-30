#!/usr/bin/env bash

set -euo pipefail

LABEL="tech.kyber.server"
APP_NAME="${1:-Kyber-Server}"
PKG_NAME="${APP_NAME}.pkg"
DMG_NAME="${APP_NAME}.dmg"
DMG_VOL="Kyber Server"
IDENTIFIER="tech.kyber.server"
VERSION="${2:-1.0.0}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$(mktemp -d)"

log() { echo "[build] $*"; }
die() { echo "[build] ERROR: $*" >&2; exit 1; }

cleanup() { rm -rf "$BUILD_DIR"; }
trap cleanup EXIT

PLIST_SRC="${SCRIPT_DIR}/tech.kyber.server.plist"
PREINSTALL_SRC="${SCRIPT_DIR}/preinstall"
POSTINSTALL_SRC="${SCRIPT_DIR}/postinstall"
AGENT_MANAGER_SCRIPT_SRC="${SCRIPT_DIR}/kyber-agent-manager.sh"
APP_SRC="${SCRIPT_DIR}/../../${APP_NAME}.app"

for f in "$PLIST_SRC" "$PREINSTALL_SRC" "$POSTINSTALL_SRC" "$AGENT_MANAGER_SCRIPT_SRC" "$APP_SRC"; do
    [[ -e "$f" ]] || die "Required file not found: ${f}"
done

which pkgbuild     &>/dev/null || die "pkgbuild not found — run on macOS."
which productbuild &>/dev/null || die "productbuild not found — run on macOS."
which hdiutil      &>/dev/null || die "hdiutil not found — run on macOS."

PAYLOAD="${BUILD_DIR}/payload"
mkdir -p "${PAYLOAD}/Applications"
mkdir -p "${PAYLOAD}/Library/LaunchAgents"

PAYLOAD_APP="${PAYLOAD}/Applications/${APP_NAME}.app"
ditto "$APP_SRC" "$PAYLOAD_APP"
chmod -R 755 "$PAYLOAD_APP"

codesign -s - -f --deep "$PAYLOAD_APP"

PLIST_DST="${PAYLOAD}/Library/LaunchAgents/${LABEL}.plist"
cp "$PLIST_SRC" "$PLIST_DST"
/usr/libexec/PlistBuddy -c "Set :ProgramArguments:0 /Applications/${APP_NAME}.app/Contents/MacOS/kycontroller" "$PLIST_DST"
chmod 644 "$PLIST_DST"

SCRIPTS_DIR="${BUILD_DIR}/scripts"
mkdir -p "$SCRIPTS_DIR"

cp "$PREINSTALL_SRC"  "${SCRIPTS_DIR}/preinstall"
cp "$POSTINSTALL_SRC" "${SCRIPTS_DIR}/postinstall"
chmod 755 "${SCRIPTS_DIR}/preinstall" "${SCRIPTS_DIR}/postinstall"

PKG_OUT="${BUILD_DIR}/${PKG_NAME}"

pkgbuild \
    --root             "$PAYLOAD" \
    --scripts          "$SCRIPTS_DIR" \
    --identifier       "$IDENTIFIER" \
    --version          "$VERSION" \
    --install-location "/" \
    "$PKG_OUT"

log "Built ${PKG_NAME}"

DMG_STAGING="${BUILD_DIR}/dmg-staging"
mkdir -p "$DMG_STAGING"

cp "$PKG_OUT"                  "${DMG_STAGING}/${PKG_NAME}"
cp "$AGENT_MANAGER_SCRIPT_SRC" "${DMG_STAGING}/kyber-agent-manager.sh"
chmod 755 "${DMG_STAGING}/kyber-agent-manager.sh"

DMG_OUT="${SCRIPT_DIR}/../../${DMG_NAME}"

hdiutil create \
    -volname   "$DMG_VOL" \
    -srcfolder "$DMG_STAGING" \
    -ov \
    -format    UDZO \
    "$DMG_OUT"

log "Built ${DMG_NAME} at ${DMG_OUT}"
