#!/usr/bin/env bash
# Install macfan + rsmc into ~/.local/bin (default) and make sure PATH has it.
#
#   scripts/install.sh [source-dir] [dest-dir]
#
# source-dir defaults to ./target/release, or the directory holding these
# scripts when run from an extracted release tarball.
set -euo pipefail

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC="${1:-}"
if [ -z "$SRC" ]; then
  if [ -x "$SELF_DIR/macfan" ]; then
    SRC="$SELF_DIR"                      # extracted release tarball
  else
    SRC="$(cd "$SELF_DIR/.." && pwd)/target/release"
  fi
fi
DEST="${2:-$HOME/.local/bin}"

for b in macfan rsmc; do
  [ -x "$SRC/$b" ] || { echo "install: $SRC/$b not found (build first: cargo build --release)" >&2; exit 1; }
done

mkdir -p "$DEST"
for b in macfan rsmc; do
  cp "$SRC/$b" "$DEST/$b"
  chmod +x "$DEST/$b"
  echo "installed $DEST/$b"
done

case ":${PATH}:" in
  *":$DEST:"*) ;;
  *)
    case "$(basename "${SHELL:-/bin/zsh}")" in
      zsh) RC="$HOME/.zshrc" ;;
      bash) RC="$HOME/.bash_profile" ;;
      *) RC="$HOME/.profile" ;;
    esac
    if ! grep -qF "$DEST" "$RC" 2>/dev/null; then
      printf '\nexport PATH="%s:$PATH"\n' "$DEST" >> "$RC"
      echo "added $DEST to PATH in $RC"
    fi
    echo "open a new terminal, or: export PATH=\"$DEST:\$PATH\""
    ;;
esac

echo
echo "try: macfan status   /   macfan temps   /   rsmc -f"
