#!/bin/sh
# Writes Casks/fuselane.rb for a *published* release, with the dmg's real SHA-256
# taken from the release's SHA256SUMS, and prints it. Usage:
#   update-cask.sh v0.1.0-beta.1 > Casks/fuselane.rb   (in the homebrew-fuselane repo)
set -eu
tag=${1:?usage: update-cask.sh vX.Y.Z[-pre]}
version=${tag#v}
repo="ArshPunisher/fuselane"
dmg="Fuselane_${version}_macos-universal.dmg"
sums=$(curl -fsSL "https://github.com/$repo/releases/download/$tag/SHA256SUMS") ||
  { echo "update-cask: no SHA256SUMS for $tag (is the release published?)" >&2; exit 1; }
sha=$(printf '%s\n' "$sums" | awk -v f="$dmg" '$2 == f || $2 == "*"f {print $1}')
[ -n "$sha" ] || { echo "update-cask: $dmg is not in SHA256SUMS" >&2; exit 1; }
cat <<CASK
cask "fuselane" do
  version "$version"
  sha256 "$sha"

  url "https://github.com/$repo/releases/download/v#{version}/Fuselane_#{version}_macos-universal.dmg"
  name "Fuselane"
  desc "Download one file over every network you have at once"
  homepage "https://github.com/$repo"

  depends_on macos: ">= :ventura"

  app "Fuselane.app"

  # Open source and ad-hoc signed, not notarized (no paid Apple account, by policy).
  postflight do
    system_command "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "#{appdir}/Fuselane.app"]
  end

  zap trash: [
    "~/Library/Application Support/app.fuselane",
  ]
end
CASK
