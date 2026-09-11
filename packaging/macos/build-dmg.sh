#!/usr/bin/env bash
# Wraps the already-built xperience binary into SNES Xperience.app and a DMG.
# Usage: build-dmg.sh <path-to-xperience-binary> <version> <out-dmg-path>
set -euo pipefail

BIN="$1"
VERSION="$2"
OUT_DMG="$3"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

APP="$WORK/SNES Xperience.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/xperience"
chmod +x "$APP/Contents/MacOS/xperience"
cp "$HERE/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$VERSION/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"

# Drag-to-install convenience: a shortcut to /Applications alongside the app.
ln -s /Applications "$WORK/Applications"

rm -f "$OUT_DMG"
hdiutil create -volname "SNES Xperience" -srcfolder "$WORK" -ov -format UDZO "$OUT_DMG"
echo "wrote $OUT_DMG"
