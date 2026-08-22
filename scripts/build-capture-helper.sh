#!/bin/sh
set -eu

fail() {
  printf 'capture helper build failed: %s\n' "$1" >&2
  exit 1
}

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd -P)
PACKAGE="$ROOT/src-tauri/native/capture-helper"
BINARIES="$ROOT/src-tauri/binaries"
SWIFT_TRIPLE=arm64-apple-macosx14.0
TAURI_TARGET=aarch64-apple-darwin
NAME=lifesub-capture-helper
OUTPUT="$BINARIES/$NAME-$TAURI_TARGET"
IDENTITY=${LIFESUB_CODESIGN_IDENTITY:--}

[ -f "$PACKAGE/Package.swift" ] || fail "missing Swift package"
mkdir -p "$BINARIES"

swift build --package-path "$PACKAGE" -c release --triple "$SWIFT_TRIPLE"
BIN_PATH=$(swift build \
  --package-path "$PACKAGE" \
  -c release \
  --triple "$SWIFT_TRIPLE" \
  --show-bin-path)
SOURCE="$BIN_PATH/$NAME"
[ -f "$SOURCE" ] && [ ! -L "$SOURCE" ] || fail "missing regular release helper"

ARCHS=$(lipo -archs "$SOURCE" 2>/dev/null) || fail "cannot inspect helper architecture"
[ "$ARCHS" = "arm64" ] || fail "helper must be arm64 only, got: $ARCHS"

TMP=$(mktemp "$BINARIES/.$NAME.XXXXXX")
trap 'rm -f "$TMP"' EXIT HUP INT TERM
cp "$SOURCE" "$TMP"
chmod 0755 "$TMP"
printf 'capture helper signing identity: %s\n' "$IDENTITY"
if [ "$IDENTITY" = "-" ]; then
  codesign --force --sign - --identifier "$NAME" --options runtime --timestamp=none "$TMP"
else
  codesign --force --sign "$IDENTITY" --identifier "$NAME" --options runtime --timestamp "$TMP"
fi
codesign --verify --strict --verbose=2 "$TMP"
mv -f "$TMP" "$OUTPUT"
trap - EXIT HUP INT TERM
codesign --verify --strict --verbose=2 "$OUTPUT"

HASH=$(shasum -a 256 "$OUTPUT" | awk '{print $1}')
printf 'capture helper: %s\n' "$OUTPUT"
printf 'capture helper sha256: %s\n' "$HASH"
