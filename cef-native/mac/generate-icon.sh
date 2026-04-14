#!/usr/bin/env bash
# Generate hodos.icns from a PNG source. Invoked by CMakeLists.txt on macOS.
# Args: <source-png> <dest-icns>
set -euo pipefail

SRC="$1"
DEST="$2"

if [[ ! -f "$SRC" ]]; then
    echo "generate-icon.sh: source PNG not found: $SRC" >&2
    exit 1
fi

TMP="$(mktemp -d)"
ICONSET="$TMP/hodos.iconset"
mkdir -p "$ICONSET"

sips -z 16 16     "$SRC" --out "$ICONSET/icon_16x16.png"      > /dev/null
sips -z 32 32     "$SRC" --out "$ICONSET/icon_16x16@2x.png"   > /dev/null
sips -z 32 32     "$SRC" --out "$ICONSET/icon_32x32.png"      > /dev/null
sips -z 64 64     "$SRC" --out "$ICONSET/icon_32x32@2x.png"   > /dev/null
sips -z 128 128   "$SRC" --out "$ICONSET/icon_128x128.png"    > /dev/null
sips -z 256 256   "$SRC" --out "$ICONSET/icon_128x128@2x.png" > /dev/null
sips -z 256 256   "$SRC" --out "$ICONSET/icon_256x256.png"    > /dev/null
sips -z 512 512   "$SRC" --out "$ICONSET/icon_256x256@2x.png" > /dev/null
sips -z 512 512   "$SRC" --out "$ICONSET/icon_512x512.png"    > /dev/null
sips -z 1024 1024 "$SRC" --out "$ICONSET/icon_512x512@2x.png" > /dev/null

iconutil -c icns "$ICONSET" -o "$DEST"
rm -rf "$TMP"
