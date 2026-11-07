#!/usr/bin/env bash
# Writes the AUR package fuselane-bin (PKGBUILD and .SRCINFO) for a published
# release: the release's own .deb for x86_64 and aarch64, with SHA-256s from the
# release's SHA256SUMS (a public download: no GitHub token needed). Usage:
#
#   packaging/aur/update-pkgbuild.sh v1.0.0
#
# Running it again with the same tag writes the same files. Publishing to the AUR
# is a git push to ssh://aur@aur.archlinux.org/fuselane-bin.git (README.md).
set -euo pipefail

repo="ArshPunisher/fuselane"
here=$(cd "$(dirname "$0")" && pwd)
out="$here/fuselane-bin"

die() {
  printf 'update-pkgbuild: %s\n' "$*" >&2
  exit 1
}

[ $# -eq 1 ] || die "give one release tag, for example: $0 v1.0.0"
tag=$1
[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  die "\"$tag\" isn't a stable release tag (vX.Y.Z). The AUR package follows stable releases only."
version=${tag#v}
command -v curl >/dev/null || die "curl isn't installed. Install it and run this again."

base="https://github.com/$repo/releases/download/$tag"
sums=$(curl -fsSL --retry 3 "$base/SHA256SUMS") ||
  die "couldn't download $base/SHA256SUMS. Check the tag is right and the release is published, then run this again."

sha_of() {
  local sha
  sha=$(printf '%s\n' "$sums" | awk -v f="$1" '!found && ($2 == f || $2 == "*" f) { print $1; found = 1 }')
  [[ $sha =~ ^[0-9a-f]{64}$ ]] || die "$tag's SHA256SUMS has no valid checksum for $1 ($base)."
  printf '%s' "$sha"
}

deb_x64="Fuselane_${version}_linux-x64.deb"
deb_arm64="Fuselane_${version}_linux-arm64.deb"
sha_x64=$(sha_of "$deb_x64")
sha_arm64=$(sha_of "$deb_arm64")

mkdir -p "$out"

# The PKGBUILD reads its own variables at build time, so most of this heredoc
# is quoted ('EOF') and only the version and checksums are filled in here.
{
  cat <<EOF
# Maintainer: ClearCraft Labs <https://github.com/$repo>
# Written by packaging/aur/update-pkgbuild.sh from the $tag release. Don't edit by hand.

pkgname=fuselane-bin
pkgver=$version
EOF
  cat <<'EOF'
pkgrel=1
pkgdesc='Download manager that uses every network at once (Wi-Fi, Ethernet, a tethered phone)'
arch=('x86_64' 'aarch64')
url='https://fuselane.app'
license=('Apache-2.0')
depends=('gtk3' 'webkit2gtk-4.1' 'libayatana-appindicator' 'hicolor-icon-theme')
provides=('fuselane')
conflicts=('fuselane')
options=('!strip' '!debug')
EOF
  cat <<EOF
source_x86_64=("fuselane-\$pkgver-x86_64.deb::https://github.com/$repo/releases/download/v\$pkgver/Fuselane_\${pkgver}_linux-x64.deb")
source_aarch64=("fuselane-\$pkgver-aarch64.deb::https://github.com/$repo/releases/download/v\$pkgver/Fuselane_\${pkgver}_linux-arm64.deb")
sha256sums_x86_64=('$sha_x64')
sha256sums_aarch64=('$sha_arm64')
EOF
  cat <<'EOF'

package() {
  # makepkg unpacks the .deb itself; the files are in data.tar.*.
  bsdtar -xf data.tar.* -C "$pkgdir"
  # Tauri puts the 256 px icon in "256x256@2"; move it where icon themes look.
  install -Dm644 "$pkgdir/usr/share/icons/hicolor/256x256@2/apps/fuselane-desktop.png" \
    "$pkgdir/usr/share/icons/hicolor/256x256/apps/fuselane-desktop.png"
  rm -r "$pkgdir/usr/share/icons/hicolor/256x256@2"
  chmod 755 "$pkgdir/usr/bin/fuselane-desktop"
}
EOF
} >"$out/PKGBUILD"

# .SRCINFO is what the AUR's web page and helpers read; it must match the
# PKGBUILD (CI compares it with `makepkg --printsrcinfo` on Arch).
cat >"$out/.SRCINFO" <<EOF
pkgbase = fuselane-bin
	pkgdesc = Download manager that uses every network at once (Wi-Fi, Ethernet, a tethered phone)
	pkgver = $version
	pkgrel = 1
	url = https://fuselane.app
	arch = x86_64
	arch = aarch64
	license = Apache-2.0
	depends = gtk3
	depends = webkit2gtk-4.1
	depends = libayatana-appindicator
	depends = hicolor-icon-theme
	provides = fuselane
	conflicts = fuselane
	options = !strip
	options = !debug
	source_x86_64 = fuselane-$version-x86_64.deb::https://github.com/$repo/releases/download/v$version/$deb_x64
	sha256sums_x86_64 = $sha_x64
	source_aarch64 = fuselane-$version-aarch64.deb::https://github.com/$repo/releases/download/v$version/$deb_arm64
	sha256sums_aarch64 = $sha_arm64

pkgname = fuselane-bin
EOF

echo "fuselane-bin $version ($tag)"
echo "  x86_64   $deb_x64  $sha_x64"
echo "  aarch64  $deb_arm64  $sha_arm64"
echo "  written to $out"
