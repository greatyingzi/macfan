#!/usr/bin/env bash
# Build macfan.app from the built binaries and the icon artwork.
#
#   menubar/scripts/make-app.sh [output-dir]
#
# Uses the universal binaries when they have been built
# (cargo build --profile dist --target aarch64-apple-darwin / x86_64-apple-darwin
# plus lipo), otherwise whatever `cargo build --profile release` produced.
#
# The icon comes from assets/icon-1024.png (source: assets/icon.svg, regenerate
# with scripts/make-icon.py). The bundle is ad-hoc signed: without a Developer
# ID that is as far as signing can go, and it is enough for the icon, the menu
# bar item and local use; it is not enough to pass Gatekeeper for a download.
set -euo pipefail

SELF="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SELF/../.." && pwd)"
MENUBAR="$ROOT/menubar"
OUT="${1:-$ROOT/dist}"
APP="$OUT/macfan.app"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$MENUBAR/Cargo.toml" | head -1)"

echo "version: $VERSION"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# Signing talks to amfid and can fail on a loaded machine, so retry, and when
# the existing signature is unusable ask codesign to drop it first.
sign_with_retry() { # $1 = path
  local path="$1" attempt=1 err=""
  while [ "$attempt" -le 3 ]; do
    if err="$(/usr/bin/codesign --force --sign - --timestamp=none "$path" 2>&1)"; then
      return 0
    fi
    if [ "$attempt" -eq 1 ]; then
      /usr/bin/codesign --remove-signature "$path" >/dev/null 2>&1 || true
    fi
    sleep 1
    attempt=$((attempt + 1))
  done
  echo "  signing $(basename "$path") failed after 3 attempts: $err" >&2
  return 1
}

# --- universal binaries ----------------------------------------------------
# Build both architectures and lipo them, so the app runs on Apple Silicon and
# Intel alike. If the second target is not installed we say so and ship the
# host architecture rather than failing.
UNIVERSAL="$ROOT/dist/bin"
ARCHES="aarch64-apple-darwin"
if rustup target list --installed 2>/dev/null | grep -qx x86_64-apple-darwin; then
  ARCHES="aarch64-apple-darwin x86_64-apple-darwin"
else
  echo "note: x86_64-apple-darwin target missing (rustup target add x86_64-apple-darwin)" >&2
  echo "      shipping a single-architecture app" >&2
fi

mkdir -p "$UNIVERSAL"
for target in $ARCHES; do
  echo "building $target"
  ( cd "$ROOT" && cargo build --release --target "$target" ) || exit 1
  ( cd "$MENUBAR" && cargo build --release --target "$target" ) || exit 1
done

for name in macfan rsmc macfan-menubar; do
  case "$name" in
    macfan-menubar) base="$MENUBAR/target" ;;
    *)              base="$ROOT/target" ;;
  esac
  slices=""
  for target in $ARCHES; do
    bin="$base/$target/release/$name"
    [ -x "$bin" ] || { echo "make-app: missing $bin" >&2; exit 1; }
    slices="$slices $bin"
  done
  if [ "$(echo "$ARCHES" | wc -w | tr -d ' ')" -gt 1 ]; then
    /usr/bin/lipo -create -output "$UNIVERSAL/$name" $slices
  else
    cp "${slices# }" "$UNIVERSAL/$name"
  fi
  # lipo strips the linker's ad-hoc signature; re-sign the merged binary so the
  # copies inside the bundle are already signed.
  sign_with_retry "$UNIVERSAL/$name" || true
  echo "  $name: $(/usr/bin/lipo -archs "$UNIVERSAL/$name")"
done

# --- pick the binaries -----------------------------------------------------
pick() { # $1 = name
  local name="$1"
  if [ -x "$UNIVERSAL/$name" ]; then echo "$UNIVERSAL/$name"; return; fi
  if [ -x "$MENUBAR/target/release/$name" ]; then echo "$MENUBAR/target/release/$name"; return; fi
  if [ -x "$ROOT/target/release/$name" ]; then echo "$ROOT/target/release/$name"; return; fi
  echo ""
}

GUI="$(pick macfan-menubar)"
[ -n "$GUI" ] || { echo "make-app: no macfan-menubar binary — build first" >&2; exit 1; }
cp "$GUI" "$APP/Contents/MacOS/macfan-menubar"

# The GUI looks for the CLI next to itself, so both go in MacOS/.
for cli in macfan rsmc; do
  src="$(pick "$cli")"
  if [ -n "$src" ]; then
    cp "$src" "$APP/Contents/MacOS/$cli"
    echo "bundled $cli from $src"
  else
    echo "warning: $cli not built; the app will fall back to PATH" >&2
  fi
done
chmod +x "$APP/Contents/MacOS/"*

# --- icon: 1024 master -> .iconset -> .icns --------------------------------
ICONSET="$OUT/AppIcon.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
MASTER="$MENUBAR/assets/icon-1024.png"
[ -f "$MASTER" ] || { echo "make-app: missing $MASTER" >&2; exit 1; }
for spec in "16:icon_16x16" "32:icon_16x16@2x" "32:icon_32x32" "64:icon_32x32@2x" \
            "128:icon_128x128" "256:icon_128x128@2x" "256:icon_256x256" \
            "512:icon_256x256@2x" "512:icon_512x512" "1024:icon_512x512@2x"; do
  size="${spec%%:*}"
  name="${spec##*:}"
  /usr/bin/sips -z "$size" "$size" "$MASTER" --out "$ICONSET/$name.png" >/dev/null
done
/usr/bin/iconutil --convert icns --output "$APP/Contents/Resources/AppIcon.icns" "$ICONSET"
echo "icon: $(/usr/bin/stat -f%z "$APP/Contents/Resources/AppIcon.icns") bytes, $(ls "$ICONSET" | wc -l | tr -d ' ') representations"

# --- Info.plist ------------------------------------------------------------
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>macfan</string>
    <key>CFBundleDisplayName</key>
    <string>macfan</string>
    <key>CFBundleIdentifier</key>
    <string>com.macfan.menubar</string>
    <key>CFBundleExecutable</key>
    <string>macfan-menubar</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleLocalizations</key>
    <array>
        <string>en</string>
        <string>zh-Hans</string>
        <string>zh-Hant</string>
        <string>ja</string>
    </array>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>MIT OR Apache-2.0</string>
</dict>
</plist>
PLIST

/usr/bin/plutil -lint "$APP/Contents/Info.plist" >/dev/null || { echo "Info.plist is not valid" >&2; exit 1; }

# --- ad-hoc signature ------------------------------------------------------
# The Mach-O files inside have to be signed first: lipo'ing strips the linker's
# ad-hoc signature, and signing a bundle that holds unsigned binaries fails with
# "code object is not signed at all".
if /usr/bin/pgrep -x macfan-menubar >/dev/null 2>&1; then
  echo "note: a macfan instance is running; macOS will not re-sign its binary." >&2
  echo "      quit it (menu bar -> Quit) and re-run to get a signed bundle." >&2
fi

# Signing can fail transiently on a loaded machine (codesign talks to amfid),
# so retry, and judge the result by an independent verify rather than by the
# exit code of the signing call alone.

SIGNED=1
for bin in "$APP/Contents/MacOS"/*; do
  sign_with_retry "$bin" || SIGNED=0
done
sign_with_retry "$APP" || SIGNED=0

if [ "$SIGNED" = "1" ] && /usr/bin/codesign --verify --strict "$APP" 2>/dev/null; then
  echo "signed (ad-hoc) and verified: $(/usr/bin/codesign -dv --verbose=2 "$APP" 2>&1 | awk -F= '/^Format=/{print $2}')"
else
  echo "warning: the bundle is not properly signed (it still runs locally)" >&2
  /usr/bin/codesign --verify --strict "$APP" 2>&1 | head -3 >&2
fi

echo "built $APP"
/usr/bin/find "$APP" -type f | sed "s|$OUT/||" | sort | sed 's/^/  /'
