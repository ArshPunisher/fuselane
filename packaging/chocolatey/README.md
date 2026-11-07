# Chocolatey

The `fuselane` package for `choco install fuselane`. It downloads the release's own Windows setup from GitHub and checks it against the release's `SHA256SUMS`; nothing is embedded, so no VERIFICATION.txt is needed.

| Path | What it is |
|---|---|
| `update-package.sh <tag>` | Writes `fuselane/` for a published stable release (`vX.Y.Z` only) |
| `fuselane/fuselane.nuspec` | Package metadata |
| `fuselane/tools/chocolateyinstall.ps1` | Downloads the setup `.exe` and runs it with `/S` |
| `fuselane/tools/chocolateyuninstall.ps1` | Runs the uninstaller the setup registered, with `/S` |

Tested on a real Windows runner by `.github/workflows/packages.yml` (pack, install, the installed app's `--self-test`, uninstall) whenever these files change on `main`.

The setup installs for the current user (Tauri's `installMode: currentUser`), so under Chocolatey it lands in the profile of the account that ran `choco`. Moderators may ask about this; it's how the app updates itself without admin rights.

## For a new release

```sh
packaging/chocolatey/update-package.sh v1.0.1
git diff packaging/chocolatey        # commit, then check the Packages workflow is green
```

## Publish (the owner)

1. Make a free account at <https://community.chocolatey.org/account/Register> and copy your API key from <https://community.chocolatey.org/account>.
2. On Windows with Chocolatey (`winget install Chocolatey.Chocolatey`):
   ```powershell
   cd packaging\chocolatey\fuselane
   choco pack
   choco apikey --key <your API key> --source https://push.chocolatey.org/
   choco push fuselane.1.0.0.nupkg --source https://push.chocolatey.org/
   ```
3. Chocolatey's validator, verifier and a human moderator review the first version (can take days to weeks). Answer their comments on the package page.
