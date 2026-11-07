# Written by packaging/chocolatey/update-package.sh from the v1.0.0 release. Don't edit by hand.
$ErrorActionPreference = 'Stop'

# Runs the uninstaller Fuselane's setup registered, silently.
[array]$keys = Get-UninstallRegistryKey -SoftwareName 'Fuselane*'
if ($keys.Count -eq 0) {
  Write-Warning 'Fuselane is not in the list of installed programs; nothing to uninstall.'
  return
}
foreach ($key in $keys) {
  $file = ($key.UninstallString -replace '^"([^"]+)".*$', '$1')
  Uninstall-ChocolateyPackage -PackageName $env:ChocolateyPackageName -FileType 'exe' `
    -SilentArgs '/S' -File $file -ValidExitCodes @(0)
}
