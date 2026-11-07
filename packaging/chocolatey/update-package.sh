#!/usr/bin/env bash
# Writes the Chocolatey package (packaging/chocolatey/fuselane) for a published
# release: the nuspec and the install/uninstall scripts, with the setup .exe's
# SHA-256 from the release's SHA256SUMS (a public download: no GitHub token
# needed). The package downloads the installer from the GitHub release; nothing
# is embedded. Usage:
#
#   packaging/chocolatey/update-package.sh v1.0.0
#
# Running it again with the same tag writes the same files. Packing and pushing:
# README.md in this folder.
set -euo pipefail

repo="ArshPunisher/fuselane"
here=$(cd "$(dirname "$0")" && pwd)
out="$here/fuselane"

die() {
  printf 'update-package: %s\n' "$*" >&2
  exit 1
}

[ $# -eq 1 ] || die "give one release tag, for example: $0 v1.0.0"
tag=$1
[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  die "\"$tag\" isn't a stable release tag (vX.Y.Z). The Chocolatey package follows stable releases only."
version=${tag#v}
command -v curl >/dev/null || die "curl isn't installed. Install it and run this again."

base="https://github.com/$repo/releases/download/$tag"
sums=$(curl -fsSL --retry 3 "$base/SHA256SUMS") ||
  die "couldn't download $base/SHA256SUMS. Check the tag is right and the release is published, then run this again."

exe="Fuselane_${version}_windows-x64-setup.exe"
sha=$(printf '%s\n' "$sums" | awk -v f="$exe" '!found && ($2 == f || $2 == "*" f) { print $1; found = 1 }')
[[ $sha =~ ^[0-9a-f]{64}$ ]] || die "$tag's SHA256SUMS has no valid checksum for $exe ($base)."

mkdir -p "$out/tools"

cat >"$out/fuselane.nuspec" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<!-- Written by packaging/chocolatey/update-package.sh from the $tag release. Don't edit by hand. -->
<package xmlns="http://schemas.microsoft.com/packaging/2015/06/nuspec.xsd">
  <metadata>
    <id>fuselane</id>
    <version>$version</version>
    <packageSourceUrl>https://github.com/$repo/tree/main/packaging/chocolatey</packageSourceUrl>
    <owners>ClearCraft Labs</owners>
    <title>Fuselane</title>
    <authors>ClearCraft Labs</authors>
    <projectUrl>https://fuselane.app</projectUrl>
    <iconUrl>https://raw.githubusercontent.com/$repo/$tag/apps/desktop/src-tauri/icons/128x128.png</iconUrl>
    <copyright>Copyright 2026 ClearCraft Labs</copyright>
    <licenseUrl>https://github.com/$repo/blob/main/LICENSE</licenseUrl>
    <requireLicenseAcceptance>false</requireLicenseAcceptance>
    <projectSourceUrl>https://github.com/$repo</projectSourceUrl>
    <docsUrl>https://fuselane.app/faq/</docsUrl>
    <bugTrackerUrl>https://github.com/$repo/issues</bugTrackerUrl>
    <tags>fuselane download-manager downloader torrent bonding multi-network tethering foss</tags>
    <summary>Downloads over every network at once</summary>
    <description><![CDATA[Fuselane spreads one download across every internet connection the computer has: home Wi-Fi, a phone tethered over USB, Ethernet, a second ISP. You get their combined speed, with no VPN, relay server or admin rights.

It resumes interrupted downloads, checks finished files against the SHA-256 a site publishes, downloads torrents over every network, and sends files to other computers on your network. It collects no user data.

Fuselane updates itself (signed updates); \`choco upgrade fuselane\` works too.
]]></description>
    <releaseNotes>https://github.com/$repo/releases/tag/$tag</releaseNotes>
  </metadata>
  <files>
    <file src="tools\\**" target="tools" />
  </files>
</package>
EOF

cat >"$out/tools/chocolateyinstall.ps1" <<EOF
# Written by packaging/chocolatey/update-package.sh from the $tag release. Don't edit by hand.
\$ErrorActionPreference = 'Stop'

# Tauri's NSIS setup, downloaded from the GitHub release and checked against the
# release's SHA256SUMS. /S installs silently.
\$packageArgs = @{
  packageName    = \$env:ChocolateyPackageName
  fileType       = 'exe'
  url64bit       = '$base/$exe'
  checksum64     = '$sha'
  checksumType64 = 'sha256'
  silentArgs     = '/S'
  validExitCodes = @(0)
  softwareName   = 'Fuselane*'
}

Install-ChocolateyPackage @packageArgs
EOF

cat >"$out/tools/chocolateyuninstall.ps1" <<EOF
# Written by packaging/chocolatey/update-package.sh from the $tag release. Don't edit by hand.
\$ErrorActionPreference = 'Stop'

# Runs the uninstaller Fuselane's setup registered, silently.
[array]\$keys = Get-UninstallRegistryKey -SoftwareName 'Fuselane*'
if (\$keys.Count -eq 0) {
  Write-Warning 'Fuselane is not in the list of installed programs; nothing to uninstall.'
  return
}
foreach (\$key in \$keys) {
  \$file = (\$key.UninstallString -replace '^"([^"]+)".*\$', '\$1')
  Uninstall-ChocolateyPackage -PackageName \$env:ChocolateyPackageName -FileType 'exe' \`
    -SilentArgs '/S' -File \$file -ValidExitCodes @(0)
}
EOF

echo "fuselane $version ($tag)"
echo "  x64  $exe  $sha"
echo "  written to $out"
