# winget

Manifests for installing Fuselane with `winget install ArshPunisher.Fuselane` (STEPS 9.3). Nothing is submitted yet: the recommendation is to submit from 1.0.

| Path | What it is |
|---|---|
| `update-manifest.sh` | Writes the three manifest files for a published release |
| `manifests/a/ArshPunisher/Fuselane/0.1.0-beta.9/` | The files it wrote for `v0.1.0-beta.9`, kept as a tested example. The folder layout is winget-pkgs' own, so it can be copied over as is |

The three files (schema 1.10.0):

- `ArshPunisher.Fuselane.yaml`: the version file.
- `ArshPunisher.Fuselane.installer.yaml`: the release's `Fuselane_<version>_windows-x64-setup.exe` with its SHA-256 from the release's `SHA256SUMS` (upper case, as winget writes it). Tauri's NSIS installer, so `InstallerType: nullsoft`; it installs for the current user without admin rights, so `Scope: user`. `Protocols` (`magnet`, `fuselane`) and `FileExtensions` (`torrent`) are what the installer registers (`apps/desktop/src-tauri/windows/hooks.nsh`). An arm64 installer is added by itself once a release has `Fuselane_<version>_windows-arm64-setup.exe`.
- `ArshPunisher.Fuselane.locale.en-US.yaml`: name, publisher, licence, descriptions, tags and links.

Left out on purpose: `ProductCode` (Tauri's installer most likely names its uninstall entry `Fuselane`, but that isn't verified; see below) and `Commands` (the setup doesn't put a command on PATH; the CLI is a separate download).

## Make the files for a new release

```sh
packaging/winget/update-manifest.sh v1.0.0
```

It reads the release's `SHA256SUMS` (public, no token) and the publish date (GitHub's API; set `RELEASE_DATE=YYYY-MM-DD` to give it yourself, or the optional `ReleaseDate` is left out). It writes `manifests/a/ArshPunisher/Fuselane/<version>/`; running it twice changes nothing. The version is the tag without its `v`. Delete older version folders here when they're no longer useful; winget-pkgs keeps its own copy of each.

## Check them

`winget validate` exists only on Windows, so these files were checked on macOS against winget's own JSON schemas (`schemas/JSON/manifests/v1.10.0` in microsoft/winget-cli), and the installer's SHA-256 against the downloaded file. The real check, on a Windows 10 or 11 machine:

```powershell
winget validate --manifest manifests\a\ArshPunisher\Fuselane\1.0.0
winget settings --enable LocalManifestFiles          # once, in an admin terminal
winget install --manifest manifests\a\ArshPunisher\Fuselane\1.0.0
winget list Fuselane                                 # shows the installed version
winget uninstall Fuselane
```

While it's installed, look in `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\` for Fuselane's key. If its name is `Fuselane`, add `ProductCode: Fuselane` to the installer file (in `update-manifest.sh`) so winget matches the installed app exactly.

winget-pkgs also has `Tools\SandboxTest.ps1 <folder>`, which installs the manifest in Windows Sandbox.

## Submit to winget-pkgs (the owner, after 1.0)

1. Fork <https://github.com/microsoft/winget-pkgs> and make a branch.
2. Copy `manifests/a/ArshPunisher/Fuselane/<version>/` to the same path in the fork.
3. Run the checks above on Windows.
4. Commit as `New package: ArshPunisher.Fuselane version <version>` (later ones: `New version: ...`), push, and open the pull request. Its bots validate, install and scan the package; a moderator merges it.

Or with Microsoft's `wingetcreate`: `wingetcreate new <installer URL>` for the first version, then `wingetcreate update ArshPunisher.Fuselane --version <version> --urls <installer URL> --submit` for each release (it needs a GitHub token with `public_repo`, kept out of this repo). The same can run from a release workflow later.

## Betas

The winget client has no beta channel: every version under `ArshPunisher.Fuselane` is offered to everyone who runs `winget upgrade`. winget compares versions part by part between the dots, and a part with text after its number sorts before the plain number, so `0.1.0-beta.9` comes before `0.1.0`, and `beta.10` after `beta.9` (the numbers compare as numbers). Betas would still reach every winget user as upgrades.

**Recommendation: don't submit betas.** Submit `1.0.0` first (STEPS 9.3 is in P9). If betas should be on winget one day, give them their own id, `ArshPunisher.Fuselane.Beta` (change `id` and the folder in the script), so people on the stable package never get one.

## Signing and updates

- **The installer isn't code-signed yet.** Windows releases are to be signed through the SignPath Foundation; until then SmartScreen may warn (README, "Code signing policy"). winget-pkgs accepts unsigned installers, but its scans flag new unsigned installers more often, so signing first makes review smoother.
- **The app updates itself** from Fuselane's signed feed (a passive NSIS install). winget then sees the newer version under Apps & features, and `UpgradeBehavior: install` installs over the top when winget is asked to upgrade.
