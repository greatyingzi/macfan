#!/usr/bin/env bash
# Put the Homebrew cask in greatyingzi/homebrew-tap at a released version.
#
# Two ways to write it, sharing one template (scripts/cask-template.rb):
#   --dir <path>   render into an existing clone of the tap (used by CI, which
#                  pushes with a deploy key)
#   (default)      render and commit through the GitHub API (used by an operator
#                  on a machine with `gh` logged in)
#
# The digest is taken from the DMG that is actually published, never from a local
# build: CI builds that artifact, so a local hash would prove nothing. The cask is
# read back afterwards and compared, because a cask with the right version and the
# wrong digest fails for users, not for us.
set -euo pipefail
cd "$(dirname "$0")/.."

TAP=greatyingzi/homebrew-tap
CASK=Casks/macfan.rb
TEMPLATE=scripts/cask-template.rb
TARGET_DIR=""

while [ $# -gt 0 ]; do
  case "$1" in
    --dir) TARGET_DIR="${2:?--dir needs a path}"; shift 2 ;;
    *) break ;;
  esac
done
V="${1:?usage: update-cask.sh [--dir <tap clone>] <version without v>}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "== downloading the published DMG for v$V"
gh release download "v$V" -R greatyingzi/macfan --pattern "macfan-$V.dmg" --dir "$WORK" --clobber
SHA="$(shasum -a 256 "$WORK/macfan-$V.dmg" | cut -d' ' -f1)"
echo "   sha256 $SHA  ($(stat -f %z "$WORK/macfan-$V.dmg") bytes)"

echo "== rendering the cask from $TEMPLATE"
python3 - "$TEMPLATE" "$V" "$SHA" > "$WORK/macfan.rb" <<'PY'
import pathlib, sys
template, version, digest = sys.argv[1], sys.argv[2], sys.argv[3]
text = pathlib.Path(template).read_text()
for token, value in (("VERSION", version), ("SHA256", digest)):
    text = text.replace("{{" + token + "}}", value)
assert "{{" not in text, "an unsubstituted placeholder survived"
sys.stdout.write(text)
PY
if grep -q '{{' "$WORK/macfan.rb"; then
  echo "  an unsubstituted placeholder survived into the cask"
  exit 1
fi

RENDERED="$(cat "$WORK/macfan.rb")"

if [ -n "$TARGET_DIR" ]; then
  echo "== writing into $TARGET_DIR"
  mkdir -p "$TARGET_DIR/$(dirname "$CASK")"
  cp "$WORK/macfan.rb" "$TARGET_DIR/$CASK"
  echo "   wrote $TARGET_DIR/$CASK"
else
  REMOTE="$(gh api "repos/$TAP/contents/$CASK" --jq '.content' 2>/dev/null | base64 -d || true)"
  if printf '%s\n' "$REMOTE" | grep -q "version \"$V\"" \
     && printf '%s\n' "$REMOTE" | grep -q "sha256 \"$SHA\""; then
    echo "   $TAP/$CASK already says $V with this digest — nothing to do"
    exit 0
  fi

  echo "== committing to $TAP"
  EXISTING_SHA="$(gh api "repos/$TAP/contents/$CASK" --jq '.sha' 2>/dev/null || true)"
  if [ -n "$EXISTING_SHA" ]; then
    gh api -X PUT "repos/$TAP/contents/$CASK" \
      -f message="macfan $V" \
      -f content="$(base64 < "$WORK/macfan.rb")" \
      -f sha="$EXISTING_SHA" >/dev/null
  else
    gh api -X PUT "repos/$TAP/contents/$CASK" \
      -f message="macfan $V" \
      -f content="$(base64 < "$WORK/macfan.rb")" >/dev/null
  fi

  echo "== reading the cask back"
  REMOTE="$(gh api "repos/$TAP/contents/$CASK" --jq '.content' | base64 -d)"
fi

WRITTEN="${TARGET_DIR:+$(cat "$TARGET_DIR/$CASK")}"
WRITTEN="${WRITTEN:-$REMOTE}"
printf '%s\n' "$WRITTEN" | grep -q "version \"$V\"" || { echo "  version did not land"; exit 1; }
printf '%s\n' "$WRITTEN" | grep -q "sha256 \"$SHA\"" || { echo "  digest did not land"; exit 1; }
echo "   ok: $CASK says $V with the digest of the published DMG"

echo "== checking brew agrees"
brew update >/dev/null 2>&1 || true
BREW_SHA="$(brew info --cask macfan 2>/dev/null | grep -i sha256 || true)"
echo "   ${BREW_SHA:-brew has not refreshed yet}"
