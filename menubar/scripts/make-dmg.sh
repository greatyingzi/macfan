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

# First launch of an unsigned app is not obvious, so say it inside the image.
cat > "$STAGE/安装说明 - How to install.txt" <<'NOTE'
macfan — 安装 / install
========================

1. 把 macfan.app 拖到右边的 Applications 文件夹。
   Drag macfan.app onto the Applications folder next to it.

2. 第一次打开需要放行一次（app 没有开发者签名，macOS 会拦）：
   The first launch needs one manual approval (the app has no Developer ID):

   方式一（推荐，终端一行）：
       xattr -dr com.apple.quarantine /Applications/macfan.app
       open /Applications/macfan.app

   方式二（图形界面）：在 Finder 里右键点 macfan.app → 打开 → 在弹框里再点“打开”。
   如果 macOS 只说“无法打开”，去 系统设置 → 隐私与安全性 → 找到 macfan 的提示 → 点“仍要打开”。

3. 之后就能正常双击启动，也会出现在启动台里。
   After that it opens normally and shows up in Launchpad.

为什么需要这一步：本 app 用 ad-hoc 签名（自签），没有 Apple 的开发者证书，
所以 Gatekeeper 会拦。把它拖进 Applications 后按上面放行一次即可，功能不受影响。
NOTE

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
  ls "$MOUNT" | grep -q "安装说明" && echo "  install note present" \
    || { echo "  install note MISSING" >&2; exit 1; }
  /usr/bin/hdiutil detach "$MOUNT" >/dev/null
fi
rmdir "$MOUNT" 2>/dev/null || true
