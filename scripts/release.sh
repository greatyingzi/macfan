#!/usr/bin/env bash
# Cut a release, and do not leave the Homebrew cask behind.
#
# The release workflow can bump the tap itself, but only with a TAP_TOKEN secret
# that is not configured, so it used to skip that step behind a warning: the cask
# sat at 0.2.0 while the releases went to 0.2.5. This script is the operator's
# path — it runs the checks, tags, waits for the published assets, updates the
# cask, and then proves the cask agrees with what was actually released.
#
# Usage: scripts/release.sh 0.2.6 "one-line summary"
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin"

V="${1:?usage: release.sh <version> <summary>}"
SUMMARY="${2:?usage: release.sh <version> <summary>}"
TAP=greatyingzi/homebrew-tap

step() { printf "\n== %s\n" "$*"; }

step "checks"
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
(cd menubar && cargo clippy --all-targets -- -D warnings)
cargo test --quiet
(cd menubar && cargo test --quiet)
echo "   fmt, clippy and tests are clean"

step "version"
python3 - "$V" <<'PY'
import pathlib, sys
version = sys.argv[1]
for name in ['Cargo.toml', 'menubar/Cargo.toml']:
    path = pathlib.Path(name)
    text = path.read_text()
    current = [l for l in text.splitlines() if l.startswith('version = "')][0]
    new = f'version = "{version}"'
    if current != new:
        path.write_text(text.replace(current, new, 1))
        print(f"   {name}: {current} -> {new}")
PY

step "offline install check"
pkill -x macfan-menubar 2>/dev/null || true
sleep 1
bash menubar/scripts/make-app.sh | tail -1
bash menubar/scripts/make-dmg.sh | tail -1

step "commit, tag, push"
git add -A
if ! git diff --cached --quiet; then
  git commit -q -m "$SUMMARY"
fi
git push -q origin main
git tag -a "v$V" -m "macfan $V — $SUMMARY" 2>/dev/null || true
git push -q origin "v$V"
echo "   pushed v$V"

step "wait for the published assets"
RUN="$(gh run list --workflow Release --limit 1 --json databaseId --jq '.[0].databaseId')"
if gh run watch "$RUN" --exit-status >/dev/null 2>&1; then
  echo "   release workflow succeeded"
else
  echo "   release workflow FAILED — the cask is not touched"; exit 1
fi
gh release view "v$V" --json assets --jq '.assets[].name' | sed 's/^/   asset: /'
gh release view "v$V" --json assets --jq '.assets[].name' | grep -q "macfan-$V.dmg" || {
  echo "   the DMG is missing from the release"; exit 1; }
echo "   the DMG the cask will point at is published"

step "cask"
bash scripts/update-cask.sh "$V"

step "verify the cask against the release"
REMOTE="$(gh api "repos/$TAP/contents/Casks/macfan.rb" --jq '.content' | base64 -d)"
echo "$REMOTE" | grep -q "version \"$V\"" || { echo "   cask version wrong"; exit 1; }
gh release download "v$V" --pattern "macfan-$V.dmg" --dir /tmp/cask-verify --clobber >/dev/null
WANT="$(shasum -a 256 "/tmp/cask-verify/macfan-$V.dmg" | cut -d' ' -f1)"
echo "$REMOTE" | grep -q "$WANT" || { echo "   cask digest does not match the published DMG"; exit 1; }
rm -rf /tmp/cask-verify
echo "   cask says $V with the digest of the published DMG"

step "done"
echo "   brew install --cask greatyingzi/tap/macfan   # then approve once (ad-hoc signed)"
