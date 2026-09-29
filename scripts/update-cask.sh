#!/usr/bin/env bash
# Bump the Homebrew cask in greatyingzi/homebrew-tap to a released version.
#
# The digest is taken from the DMG that is actually published, not from the build
# on this machine: CI builds that artifact, so hashing the local one would prove
# nothing. The cask is read back afterwards and compared, because a cask that
# says the right version with the wrong digest fails for users, not for us.
#
# Usage: scripts/update-cask.sh 0.2.5
set -euo pipefail

V="${1:?usage: update-cask.sh <version without v>}"
TAP=greatyingzi/homebrew-tap
CASK=Casks/macfan.rb
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "== downloading the published DMG for v$V"
gh release download "v$V" -R greatyingzi/macfan --pattern "macfan-$V.dmg" --dir "$WORK" --clobber
SHA="$(shasum -a 256 "$WORK/macfan-$V.dmg" | cut -d' ' -f1)"
SIZE="$(stat -f %z "$WORK/macfan-$V.dmg")"
echo "   sha256 $SHA  ($SIZE bytes)"

echo "== building the cask"
cat > "$WORK/macfan.rb" <<RUBY
cask "macfan" do
  version "$V"
  sha256 "$SHA"

  url "https://github.com/greatyingzi/macfan/releases/download/v#{version}/macfan-#{version}.dmg"
  name "macfan"
  desc "Menu bar fan control and AppleSMC utility"
  homepage "https://github.com/greatyingzi/macfan"

  livecheck do
    url :url
    strategy :github_latest
  end

  app "macfan.app"

  caveats <<~EOS
    macfan is ad-hoc signed rather than notarised, so macOS quarantines it and
    the first launch needs one approval:

      xattr -dr com.apple.quarantine /Applications/macfan.app

    ...or open System Settings > Privacy & Security and allow it there.
    Homebrew 7 removed its no-quarantine flag, so that is the only route.
  EOS

  zap trash: [
    "~/Library/Application Support/macfan",
    "~/Library/LaunchAgents/com.macfan.menubar.plist",
  ]
end
RUBY

if grep -q '`' "$WORK/macfan.rb"; then
  echo "  the generated cask contains a backtick, which the heredoc would have run as a command"
  exit 1
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
echo "$REMOTE" | grep -q "version \"$V\"" || { echo "  version did not land"; exit 1; }
echo "$REMOTE" | grep -q "sha256 \"$SHA\"" || { echo "  digest did not land"; exit 1; }
echo "   ok: $TAP/$CASK says $V with the digest of the published DMG"

echo "== checking brew agrees"
brew update >/dev/null 2>&1 || true
BREW_SHA="$(brew info --cask macfan 2>/dev/null | grep -i sha256 || true)"
echo "   ${BREW_SHA:-brew has not refreshed yet}"
