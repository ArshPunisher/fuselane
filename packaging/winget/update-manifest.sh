#!/usr/bin/env bash
# Writes the winget manifests for a published release, in the winget-pkgs layout
# (manifests/a/ArshPunisher/Fuselane/<version>/, schema 1.10.0), with the setup
# .exe's SHA-256 taken from the release's own SHA256SUMS (a public download: no
# GitHub token needed). Usage:
#
#   packaging/winget/update-manifest.sh v0.1.0-beta.10
#
# Running it again with the same tag writes the same files. The release date comes
# from GitHub's API; set RELEASE_DATE=YYYY-MM-DD to give it yourself. An arm64
# setup .exe is added when the release has one. Works with the macOS and Linux
# tools alike (only curl, awk and tr).
set -euo pipefail

repo="ArshPunisher/fuselane"
id="ArshPunisher.Fuselane"
schema="1.10.0"
here=$(cd "$(dirname "$0")" && pwd)

die() {
  printf 'update-manifest: %s\n' "$*" >&2
  exit 1
}

[ $# -eq 1 ] || die "give one release tag, for example: $0 v0.1.0-beta.9"
tag=$1
[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z]+(\.[0-9A-Za-z]+)*)?$ ]] ||
  die "\"$tag\" isn't a release tag. Use vX.Y.Z or vX.Y.Z-beta.N, for example v0.1.0-beta.9."
version=${tag#v}
command -v curl >/dev/null || die "curl isn't installed. Install it and run this again."

base="https://github.com/$repo/releases/download/$tag"
sums=$(curl -fsSL --retry 3 "$base/SHA256SUMS") ||
  die "couldn't download $base/SHA256SUMS. Check the tag is right and the release is published (a draft is private), then run this again."

# No early `exit` in these awk scripts: with pipefail, a writer cut off by SIGPIPE
# would fail the whole command.
sha_of() {
  printf '%s\n' "$sums" |
    awk -v f="$1" '!found && ($2 == f || $2 == "*" f) { print $1; found = 1 }' |
    tr '[:lower:]' '[:upper:]'
}

echo "Fuselane $version ($tag)"
# x64 must be there; arm64 only when the release has it (PARITY: x64 + ARM64 later).
installers=""
for arch in x64 arm64; do
  exe="Fuselane_${version}_windows-${arch}-setup.exe"
  sha=$(sha_of "$exe")
  if [ -z "$sha" ]; then
    [ "$arch" = x64 ] &&
      die "$tag's SHA256SUMS has no checksum for $exe. Check the release has the Windows installer ($base), or that the Release workflow finished."
    continue
  fi
  [[ $sha =~ ^[0-9A-F]{64}$ ]] || die "the checksum for $exe in $tag's SHA256SUMS is malformed (\"$sha\"). Check the file on the release page."
  installers+="  - Architecture: $arch
    InstallerUrl: $base/$exe
    InstallerSha256: $sha
"
  echo "  $arch  $exe  $sha"
done

date=${RELEASE_DATE:-}
if [ -z "$date" ]; then
  api="https://api.github.com/repos/$repo/releases/tags/$tag"
  if json=$(curl -fsSL --retry 3 -H 'Accept: application/vnd.github+json' "$api"); then
    date=$(printf '%s\n' "$json" | awk '
      !found && match($0, /"published_at": *"[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/) {
        s = substr($0, RSTART, RLENGTH); print substr(s, length(s) - 9); found = 1
      }')
  fi
fi
release_date=""
if [ -n "$date" ]; then
  [[ $date =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] ||
    die "RELEASE_DATE must look like 2026-10-10 (got \"$date\"). Fix it and run this again."
  release_date="ReleaseDate: $date
"
else
  # ReleaseDate is optional in winget; leave it out rather than guess.
  echo "update-manifest: no publish date from GitHub's API for $tag; leaving ReleaseDate out (set RELEASE_DATE=YYYY-MM-DD to add it)." >&2
fi

out="$here/manifests/a/ArshPunisher/Fuselane/$version"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/fuselane-winget.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
made="# Created by packaging/winget/update-manifest.sh from the $tag release's SHA256SUMS."
# Lists are indented (valid winget YAML) so the repo's Prettier check passes.

cat >"$tmp/$id.yaml" <<EOF
$made
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.$schema.schema.json

PackageIdentifier: $id
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $schema
EOF

# nullsoft: Tauri's NSIS installer (silent with /S). Scope user: installMode
# currentUser, no admin rights. Protocols and FileExtensions: what the installer
# registers (windows/hooks.nsh): magnet and .torrent offered, fuselane:// owned.
# No ProductCode or Commands: the uninstall key isn't verified, and the setup
# doesn't put a command on PATH (the CLI is a separate download).
cat >"$tmp/$id.installer.yaml" <<EOF
$made
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.$schema.schema.json

PackageIdentifier: $id
PackageVersion: $version
InstallerLocale: en-US
InstallerType: nullsoft
Scope: user
InstallModes:
  - interactive
  - silent
  - silentWithProgress
UpgradeBehavior: install
Protocols:
  - fuselane
  - magnet
FileExtensions:
  - torrent
${release_date}Installers:
${installers}ManifestType: installer
ManifestVersion: $schema
EOF

cat >"$tmp/$id.locale.en-US.yaml" <<EOF
$made
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.$schema.schema.json

PackageIdentifier: $id
PackageVersion: $version
PackageLocale: en-US
Publisher: ArshPunisher
PublisherUrl: https://github.com/ArshPunisher
PublisherSupportUrl: https://github.com/ArshPunisher/fuselane/issues
PrivacyUrl: https://github.com/ArshPunisher/fuselane/blob/main/docs/PRIVACY.md
Author: The Fuselane Authors
PackageName: Fuselane
PackageUrl: https://fuselane.app
License: Apache-2.0
LicenseUrl: https://github.com/ArshPunisher/fuselane/blob/main/LICENSE
Copyright: Copyright 2026 The Fuselane Authors
ShortDescription: Downloads over every network at once
Description: |-
  Fuselane spreads one download across every internet connection the computer has: home Wi-Fi, a phone tethered over USB, Ethernet, a second ISP. You get their combined speed, with no VPN, relay server or admin rights.
  It resumes interrupted downloads, checks finished files against the SHA-256 a site publishes, downloads torrents over every network, and sends files to other computers on your network. It collects no user data.
Moniker: fuselane
Tags:
  - bonding
  - download-manager
  - downloader
  - multi-network
  - tethering
  - torrent
ReleaseNotesUrl: https://github.com/$repo/releases/tag/$tag
ManifestType: defaultLocale
ManifestVersion: $schema
EOF

mkdir -p "$out"
for f in "$tmp"/*.yaml; do
  name=$(basename "$f")
  if cmp -s "$f" "$out/$name"; then
    echo "  $name: already up to date"
  else
    cat "$f" >"$out/$name"
    echo "  $name: written"
  fi
done
echo "  in ${out#"$here"/}"
