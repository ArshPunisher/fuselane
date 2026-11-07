# AUR (Arch Linux)

The `fuselane-bin` package for `yay -S fuselane-bin` (or any AUR helper). It repackages the release's own `.deb` for x86_64 and aarch64, checked against the release's `SHA256SUMS`.

| Path | What it is |
|---|---|
| `update-pkgbuild.sh <tag>` | Writes `fuselane-bin/` for a published stable release (`vX.Y.Z` only) |
| `fuselane-bin/PKGBUILD` | The build recipe |
| `fuselane-bin/.SRCINFO` | What the AUR and its helpers read; must match the PKGBUILD |

Tested in an Arch Linux container by `.github/workflows/packages.yml` (`.SRCINFO` equals `makepkg --printsrcinfo`, namcap, build, install, the installed app's `--self-test`) whenever these files change on `main`.

## For a new release

```sh
packaging/aur/update-pkgbuild.sh v1.0.1
git diff packaging/aur               # commit, then check the Packages workflow is green
```

## Publish (the owner)

1. Make an account at <https://aur.archlinux.org/register> and add an SSH public key to it (My Account → SSH Public Key; for example `~/.ssh/id_ed25519.pub`).
2. Push the two files to the AUR's own git repository (the first push creates the package):
   ```sh
   git clone ssh://aur@aur.archlinux.org/fuselane-bin.git /tmp/fuselane-bin
   cp packaging/aur/fuselane-bin/PKGBUILD packaging/aur/fuselane-bin/.SRCINFO /tmp/fuselane-bin/
   cd /tmp/fuselane-bin
   git add PKGBUILD .SRCINFO
   git commit -m "fuselane-bin 1.0.0"
   git push
   ```
3. It's live at once on <https://aur.archlinux.org/packages/fuselane-bin>. Later releases: same steps with the new files.
