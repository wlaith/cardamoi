#!/bin/sh
set -eu
VERSION=2.6.5.5623
# One of: osx-arm64, osx-x64, linux-x64, windows-x64
TARGET=${1:-osx-arm64}
DEST="$(dirname "$0")/../src-tauri/vendor/prowlarr"
case "$TARGET" in
  osx-arm64|osx-x64|linux-x64) EXT=tar.gz ;;
  windows-x64) EXT=zip ;;
  *) echo "unknown target: $TARGET" >&2; exit 1 ;;
esac
URL="https://github.com/Prowlarr/Prowlarr/releases/download/v$VERSION/Prowlarr.master.$VERSION.${TARGET%-*}-core-${TARGET#*-}.$EXT"

rm -rf "$DEST"
mkdir -p "$DEST"
if [ "$EXT" = zip ]; then
  TMP=$(mktemp -d)
  curl -fL "$URL" -o "$TMP/prowlarr.zip"
  unzip -q "$TMP/prowlarr.zip" -d "$TMP"
  cp -R "$TMP/Prowlarr/." "$DEST"
  rm -rf "$TMP"
else
  curl -fL "$URL" | tar -xz -C "$DEST" --strip-components 1
fi

if [ "$(uname)" = Darwin ]; then
  # Apple Silicon kills unsigned code: ad-hoc sign it.
  find "$DEST" -type f \( -name Prowlarr -o -name '*.dylib' \) -exec codesign --force --sign - {} \; 2>/dev/null
  # Copy after signing, or macOS can hang on the re-signed file.
  cp -R "$DEST" "$DEST.fresh"
  rm -rf "$DEST"
  mv "$DEST.fresh" "$DEST"
fi

echo "Prowlarr $VERSION ($TARGET) is in $DEST"
