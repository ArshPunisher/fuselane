# Written by packaging/chocolatey/update-package.sh from the v1.0.0 release. Don't edit by hand.
$ErrorActionPreference = 'Stop'

# Tauri's NSIS setup, downloaded from the GitHub release and checked against the
# release's SHA256SUMS. /S installs silently.
$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  fileType       = 'exe'
  url64bit       = 'https://github.com/ArshPunisher/fuselane/releases/download/v1.0.0/Fuselane_1.0.0_windows-x64-setup.exe'
  checksum64     = 'd280840f6e788e764bfc871d576a22153582a8c23d6f4a961c478960f3686f0c'
  checksumType64 = 'sha256'
  silentArgs     = '/S'
  validExitCodes = @(0)
  softwareName   = 'Fuselane*'
}

Install-ChocolateyPackage @packageArgs
