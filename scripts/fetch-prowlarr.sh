#!/bin/sh
set -eu
VERSION=2.6.5.5623
ARCH=${1:-arm64}
DEST="$(dirname "$0")/../src-tauri/vendor/prowlarr"
URL="https://github.com/Prowlarr/Prowlarr/releases/download/v$VERSION/Prowlarr.master.$VERSION.osx-core-$ARCH.tar.gz"

rm -rf "$DEST"
mkdir -p "$DEST"
curl -fL "$URL" | tar -xz -C "$DEST" --strip-components 1
# Apple Silicon kills unsigned code: ad-hoc sign it.
find "$DEST" -type f \( -name Prowlarr -o -name '*.dylib' \) -exec codesign --force --sign - {} \; 2>/dev/null
# Copy after signing, or macOS can hang on the re-signed file.
cp -R "$DEST" "$DEST.fresh"
rm -rf "$DEST"
mv "$DEST.fresh" "$DEST"

echo "Prowlarr $VERSION ($ARCH) is in $DEST"
