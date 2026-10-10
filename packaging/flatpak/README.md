# Flatpak

A Flatpak of the Fuselane desktop app, made from the release's own `.deb` packages (STEPS 4.4). Flathub submission waits until 1.0 (STEPS 9.3).

| File | What it is |
|---|---|
| `app.fuselane.Fuselane.yml` | The manifest: GNOME 51 runtime, sandbox permissions (each one explained), the x86_64 and aarch64 `.deb` with their SHA-256 |
| `app.fuselane.Fuselane.metainfo.xml` | AppStream metadata: what Flathub and software centres show (description, screenshots, releases) |
| `app.fuselane.Fuselane.desktop` | The launcher entry: icon, categories, `.torrent` files, magnet and `fuselane://` links |
| `update-manifest.sh` | Points the manifest at a release and adds that release to the metainfo |
| `.gitignore` | Keeps local build output and the cloned shared modules out of git |

## How it is built

- **From the release `.deb`, not from source.** These are the binaries the Release workflow built, checked and self-tested. The build unpacks the `.deb` and installs the binary, the icons (renamed to the app id), the desktop entry and the metainfo.
- **GNOME 51 runtime.** Tauri 2 needs WebKitGTK 4.1 (`webkit2gtk-4.1`), which the GNOME runtime ships. GNOME 48 reached end of life on 2026-03-24, so 51, the current release, is used. Move up each March and September.
- **libayatana-appindicator** is built from Flathub's shared module. The tray needs it, the GNOME runtime doesn't have it, and without it the app stops at start-up ("Failed to load ayatana-appindicator3").

## Which app id

The Tauri identifier is `app.fuselane`, but **Flatpak refuses app ids with fewer than three parts** ("Names must contain at least 2 periods"), and Flathub's linter does too (`appid-less-than-3-components`). So the Flatpak id has to differ from the Tauri identifier either way. The two real choices:

| Id | Verified on Flathub by | Ties it to |
|---|---|---|
| **`app.fuselane.Fuselane`** (used here) | A token file at `https://fuselane.app/.well-known/org.flathub.VerifiedApps.txt` | Keeping the fuselane.app domain |
| `io.github.arshpunisher.Fuselane` | Signing in to Flathub with the GitHub account | The GitHub username, free forever |

**Recommendation: `app.fuselane.Fuselane`.** The owner controls fuselane.app (the site runs there on Cloudflare Pages), the domain matches the Tauri identifier, the browser extension's host name (`app.fuselane.host`) and the macOS data folder, and the token file is a free static file in `apps/site-next/public/.well-known/`. Flathub needs control of that domain for the verified badge.

Pick `io.github.arshpunisher.Fuselane` instead if the domain might not be renewed for good (ADR 0009: no paid services). **Decide before the first Flathub submission:** a Flathub id is permanent (renaming means an end-of-life rebase that every user has to follow). Changing it now is only a rename of the three files plus the `id`, `Icon`, `<id>`, `<launchable>` and icon names.

Window matching doesn't depend on the id: Tauri leaves the GTK application id unset, so the window's class (X11) and app id (Wayland) are `fuselane-desktop`, which `StartupWMClass` covers.

## Build and test on Linux

Once: Flatpak, the Flathub remote and the builder app.

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.flatpak.Builder
```

Build and install (it fetches the GNOME 51 runtime and SDK from Flathub the first time):

```sh
cd packaging/flatpak
git clone --depth 1 https://github.com/flathub/shared-modules.git   # once; ignored by git
flatpak run org.flatpak.Builder --user --install --install-deps-from=flathub \
  --force-clean build-dir app.fuselane.Fuselane.yml
```

With a distro `flatpak-builder` it's the same: `flatpak-builder --user --install --install-deps-from=flathub --force-clean build-dir app.fuselane.Fuselane.yml`.

Check the files and the result:

```sh
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest app.fuselane.Fuselane.yml
flatpak run --command=appstreamcli org.flatpak.Builder validate --no-net app.fuselane.Fuselane.metainfo.xml
desktop-file-validate app.fuselane.Fuselane.desktop                 # from desktop-file-utils
flatpak run app.fuselane.Fuselane --self-test                       # the release workflow's packaged self-test
flatpak run app.fuselane.Fuselane
```

The manifest lint reports `finish-args-own-name-app.fuselane.SingleInstance` until the manifest moves to beta.11 or later ("Before Flathub", item 1). To lint the built repository too, add `--repo=repo` to the build and run `flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo`.

By hand: the window opens on Wayland and on X11 (`flatpak run --nosocket=wayland app.fuselane.Fuselane`); the tray shows Fuselane's icon; a download into `~/Downloads` finishes with a notification; Networks lists every interface and a download uses more than one (this is spike S3: bonding inside the sandbox); a `.torrent` double-clicked and a magnet link clicked in a browser open in the window that is already running; Show in folder opens the folder.

## Update for a release

```sh
packaging/flatpak/update-manifest.sh v0.1.0-beta.10
git diff packaging/flatpak
```

It reads the release's `SHA256SUMS` (public, no token), rewrites both `.deb` URLs and SHA-256s, and adds the release to the metainfo's `<releases>` with its publish date (from GitHub's API; set `RELEASE_DATE=YYYY-MM-DD` if the API's hourly limit is used up). Running it twice changes nothing. Then build again to check. On Flathub the same change goes as a pull request to the app's own repository there.

## How the app behaves in the sandbox

From 0.1.0-beta.11 the app knows when it runs as a Flatpak (`FLATPAK_ID`, or `/.flatpak-info`; `apps/desktop/src-tauri/src/flatpak.rs`) and changes only these things there. Outside a Flatpak, and always on macOS and Windows, nothing changes.

| What | In the Flatpak |
|---|---|
| **Updates** | Never checked, downloaded or installed in the app: Flathub (or whichever remote it came from) updates it. Settings → Updates says "Updates come through your software centre (Flatpak).", the update banner never appears, and the update commands answer `update-flatpak` with that message instead of trying. |
| **One running copy** | The single-instance D-Bus name is `app.fuselane.Fuselane.SingleInstance` (the plugin's `dbus_id`, set to the Flatpak id). An app may own names under its own id, so no `--own-name` is needed. |
| **Start at login** | Through the Background portal (`org.freedesktop.portal.Background.RequestBackground`, `autostart` on or off, command line `fuselane-desktop --minimized`). The portal writes the autostart entry on the host, where the desktop reads it; the app can't see that entry, so it remembers what the portal last granted. If the user or the desktop refuses, Settings says so. |
| **Keep the computer awake** | Through the Inhibit portal (`org.freedesktop.portal.Inhibit`, flag 4: suspend), held while something downloads and closed when nothing does; the portal also lifts it if the app dies. |
| **Sleep or Shut down when done** | Not offered (Settings shows only Nothing and Quit, with a one-line note), and refused by the backend: they need `systemctl`, which the sandbox lacks, and no portal does this. |
| **Browser extension** | The app doesn't write native-messaging manifests (the browsers' folders are on the host, out of reach, and a browser couldn't start the sandboxed helper anyway). Settings → Other apps and the welcome tour say: "The browser extension can't talk to the Flatpak version yet; use the .deb or AppImage for it." |

All of this is done with zbus calls (zbus is already in the build for the single-instance plugin), so no crates were added.

## Still limited in the sandbox

- **The browser extension**, as above. It needs a host-side helper that talks to the sandbox (a later step).
- **The CLI.** The Flatpak keeps its own download list in `~/.var/app/app.fuselane.Fuselane/data/fuselane`, separate from a `.deb` or AppImage install, and its local API lives in the sandbox, so the host's `fuselane` command doesn't see it.
- **Sleep or Shut down when done**, as above.
- **Video and audio from pages.** This uses `yt-dlp` and `ffmpeg` installed on the computer, which the sandbox can't see.
- **Folders outside Downloads.** Only `~/Downloads` is open to the app. A folder picked in Fuselane is granted through the file-chooser portal and shows up as `/run/user/<uid>/doc/...`. To use a folder's real path, grant it: `flatpak override --user --filesystem=~/Videos app.fuselane.Fuselane` (or use Flatseal).
- **Bonding** should work (with `--share=network` the app sees every interface, and `SO_BINDTODEVICE` needs no privileges on kernel 5.7+) but is not verified yet (spike S3).
- **Not yet run in a real Flatpak.** The changes above have unit tests (the detection, the gating, the portal request paths and answers) and UI tests (the demo backend's `?flatpak=1`), and the Linux-only code was compiled on macOS, but the portals and the single-instance name have not been exercised on Linux yet. Check them in the first build from beta.11: toggle Start at login (then look for `~/.config/autostart/app.fuselane.Fuselane.desktop` on the host), start a download with Keep awake on (`gnome-session-inhibit --list` or `systemd-inhibit --list` shows it), and open a magnet link while the app runs (it goes to the running window).

## Before Flathub

1. **The manifest still pins the beta.9 `.deb`**, which predates these app changes, so it keeps `--own-name=app.fuselane.SingleInstance`. When `update-manifest.sh` moves it to beta.11 or later, delete that line (and its comment); `flatpak-builder-lint` then stops reporting `finish-args-own-name-app.fuselane.SingleInstance`.
2. Screenshots of the app on Linux, at URLs pinned to a release tag rather than `main` (the current two are from macOS: "Show in Finder").
3. Keep the desktop entries in step: this one and the `.deb`'s own (from `tauri.linux.conf.json`) both offer `.torrent` files, magnet links and `fuselane://`.

## Submit to Flathub (after 1.0)

Not before 1.0 (STEPS 9.3). Flathub's stable repository is for stable releases; betas belong on Flathub's beta repository, if at all.

1. Fork <https://github.com/flathub/flathub> with all branches (untick "Copy the master branch only").
2. Clone the `new-pr` branch and start a branch from it:
   ```sh
   git clone --branch=new-pr git@github.com:<you>/flathub.git && cd flathub
   git checkout -b add-fuselane
   ```
3. Add `app.fuselane.Fuselane.yml`, `app.fuselane.Fuselane.desktop` and `app.fuselane.Fuselane.metainfo.xml`, and the shared modules as a submodule: `git submodule add https://github.com/flathub/shared-modules.git`.
4. Commit, push, and open a pull request against **`new-pr`** (not `master`), titled "Add app.fuselane.Fuselane". A bot builds it; comment `bot, build` to build again after changes.
5. What reviewers check: `flatpak-builder-lint` passes for the manifest and the built repository (Flathub reviews any `--own-name`), the metainfo passes `appstreamcli validate`, a supported runtime, builds on x86_64 and aarch64, every permission justified, Linux screenshots, and the app id's domain. Flathub prefers apps built from source: reviewers may accept the release `.deb`, but be ready to build from source instead (vendoring crates and pnpm packages offline with flatpak-builder-tools' cargo and node generators).
6. After the merge, Flathub creates `github.com/flathub/app.fuselane.Fuselane` and invites the owner. Updates are pull requests there (this script's output); its external data checker can open them by itself (`x-checker-data` on the `.deb` sources, reading GitHub's latest release, which skips pre-releases).
7. Verify the app: sign in on flathub.org, open the app's developer settings, and put the token it gives at `https://fuselane.app/.well-known/org.flathub.VerifiedApps.txt` (a file in `apps/site-next/public/.well-known/`, then deploy the site).
