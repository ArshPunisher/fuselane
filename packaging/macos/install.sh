#!/bin/sh
# Installs Fuselane on macOS so nobody has to type security commands (L-72).
#   curl -fsSL https://raw.githubusercontent.com/ArshPunisher/fuselane/main/packaging/macos/install.sh | sh
# Options (environment):
#   FUSELANE_VERSION=v0.1.0-beta.1   install that release instead of the newest
#   FUSELANE_DEST=/path/Applications install somewhere else
#   FUSELANE_RELEASE_BASE=...        download from another base URL (tests)
# The download is checked against the release's SHA256SUMS before anything is installed.
set -eu

repo="ArshPunisher/fuselane"
say() { printf '%s\n' "fuselane: $*"; }
die() { printf '%s\n' "fuselane: $*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || die "this installer is for macOS. See https://github.com/$repo/releases for other systems."
major=$(sw_vers -productVersion | cut -d. -f1)
[ "$major" -ge 13 ] || die "Fuselane needs macOS 13.3 or newer."
command -v curl >/dev/null || die "curl is missing."

if [ -n "${FUSELANE_RELEASE_BASE:-}" ]; then
  base=$FUSELANE_RELEASE_BASE
  tag=${FUSELANE_VERSION:-test}
else
  if [ -n "${FUSELANE_VERSION:-}" ]; then
    tag=$FUSELANE_VERSION
  else
    # /releases/latest skips pre-releases (L-77), so read the list and take the newest published one.
    tag=$(curl -fsSL "https://api.github.com/repos/$repo/releases?per_page=20" |
      sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
    [ -n "$tag" ] || die "couldn't find a release. Check your connection, or see https://github.com/$repo/releases"
  fi
  base="https://github.com/$repo/releases/download/$tag"
fi
version=${tag#v}
asset="Fuselane_${version}_macos-universal.app.tar.gz"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
say "downloading Fuselane $version"
curl -fsSL "$base/$asset" -o "$tmp/$asset" || die "couldn't download $asset."
curl -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" || die "couldn't download SHA256SUMS."

want=$(awk -v f="$asset" '$2 == f || $2 == "*"f {print $1}' "$tmp/SHA256SUMS")
[ -n "$want" ] || die "SHA256SUMS has no entry for $asset."
got=$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')
[ "$got" = "$want" ] || die "the download is corrupted or was changed (SHA-256 mismatch). Nothing was installed."

tar -xzf "$tmp/$asset" -C "$tmp"
[ -d "$tmp/Fuselane.app" ] || die "the download doesn't contain Fuselane.app."

dest=${FUSELANE_DEST:-/Applications}
if [ ! -w "$dest" ]; then
  dest="$HOME/Applications"
  mkdir -p "$dest"
fi
if pgrep -x fuselane-desktop >/dev/null 2>&1; then
  die "Fuselane is running. Quit it (menu bar icon → Quit), then run this again."
fi
rm -rf "$dest/Fuselane.app"
mv "$tmp/Fuselane.app" "$dest/Fuselane.app"
# Open-source builds are ad-hoc signed, not notarized (ADR 0009). Clearing the
# quarantine flag here is what the "Open Anyway" button would do.
xattr -dr com.apple.quarantine "$dest/Fuselane.app" 2>/dev/null || true

"$dest/Fuselane.app/Contents/MacOS/fuselane-desktop" --version >/dev/null || die "the installed app didn't start."
say "installed Fuselane $version in $dest. Open it from Launchpad or Spotlight."
