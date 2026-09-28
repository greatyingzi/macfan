#!/usr/bin/env bash
# Package macfan.app into a drag-to-install DMG.
#
#   menubar/scripts/make-dmg.sh [app-path] [output-dmg]
#
# The image holds the app plus an /Applications symlink, which is the layout
# people expect: open the DMG, drag the app across.
set -euo pipefail

SELF="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SELF/../.." && pwd)"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/menubar/Cargo.toml" | head -1)"

APP="${1:-$ROOT/dist/macfan.app}"
OUT="${2:-$ROOT/dist/macfan-$VERSION.dmg}"

[ -d "$APP" ] || { echo "make-dmg: $APP not found (run scripts/make-app.sh first)" >&2; exit 1; }

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

rm -f "$OUT"
/usr/bin/hdiutil create \
  -volname "macfan" \
  -srcfolder "$STAGE" \
  -fs HFS+ \
  -format UDZO \
  -ov \
  "$OUT" >/dev/null

/usr/bin/hdiutil verify "$OUT" >/dev/null
echo "built $OUT ($(/usr/bin/stat -f%z "$OUT") bytes)"

# Prove it mounts and the app is inside, rather than trusting the exit code.
MOUNT="$(/usr/bin/mktemp -d)"
if /usr/bin/hdiutil attach "$OUT" -nobrowse -readonly -mountpoint "$MOUNT" >/dev/null; then
  echo "mounted contents:"
  /bin/ls -1 "$MOUNT" | sed 's/^/  /'
  [ -x "$MOUNT/macfan.app/Contents/MacOS/macfan-menubar" ] \
    && echo "  app executable present" \
    || { echo "  app executable MISSING" >&2; exit 1; }
  /usr/bin/hdiutil detach "$MOUNT" >/dev/null
fi
rmdir "$MOUNT" 2>/dev/null || true
