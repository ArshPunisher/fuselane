# Store listing (Chrome Web Store, Edge Add-ons, Firefox Add-ons)

Everything the store forms ask for, in one place. Build the upload with
`pnpm --filter @fuselane/extension zip` (files land in `apps/extension/.output/`).

**Chrome Web Store:** item `nggljghjikdkigiekdciocigdnnhponl`, **approved and live** (0.1.0, 2026-10-10): https://chromewebstore.google.com/detail/fuselane/nggljghjikdkigiekdciocigdnnhponl. Next upload: 0.2.0 (page list, signed-in downloads) with the images below.

## Name and summary

- **Name:** Fuselane
- **Summary (132 characters max):** Hands big downloads to the Fuselane app, which splits them across Wi-Fi, Ethernet and a tethered phone at once.
- **Category:** Productivity (Chrome) / Download management (Firefox)
- **Language:** English

## Description

Fuselane makes big downloads faster by using every network your computer has, at the same time.

Wi-Fi, Ethernet and a phone tethered over USB each carry part of the file. Fuselane fetches the parts in parallel, checks every byte, and joins them into one verified file. A download that takes 10 minutes on Wi-Fi alone can finish in a fraction of that.

This extension connects your browser to the free Fuselane app:

★ Big downloads go to Fuselane automatically
When the app is running, downloads of 1 MB or more go straight to it. Anything it can't take stays in the browser, so you never lose a download.

★ Everything on the page, one click away
Open the toolbar button to see the videos and file links on the page you're on, and send any of them to Fuselane.

★ Videos from pages
On YouTube and over 1,000 other sites, the toolbar button hands the page to Fuselane, which offers each quality and downloads the video over every network (with the free yt-dlp installed).

★ Right-click any link
Choose "Download with Fuselane" on any link.

★ You decide what goes
Pick the smallest size to hand over, which sites always or never use Fuselane, and which file types.

★ Signed-in downloads (optional)
Turn it on in settings and files that need a sign-in work too. Off by default.

What the Fuselane app adds:
• Every network at once: Wi-Fi, Ethernet and a USB-tethered phone
• Verified files: checksums found and checked by themselves
• Resume after any drop, pause, or restart
• Groups, schedules, "Ready by" times, and per-network data limits
• Torrents and direct sharing to nearby computers
• Free and open source, for macOS, Windows and Linux

Get the app: https://fuselane.app/

Privacy: the extension sends nothing to the internet. It only talks to the Fuselane app on your own computer. No account, no tracking, no ads.

## Single purpose (Chrome)

Hand downloads from the browser to the Fuselane desktop app on the same computer.

## Why each permission is needed

| Permission        | Why                                                                                                                         |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `downloads`       | To see when a download starts, pause it while asking the app, and cancel it if the app takes it over (or resume it if not). |
| `nativeMessaging` | To talk to the Fuselane app on this computer. This is the only place the extension sends anything.                          |
| `storage`         | To remember whether hand-off is switched on.                                                                                |
| `contextMenus`    | To add "Download with Fuselane" to the right-click menu on links.                                                           |
| `activeTab`       | When the person opens the popup, to list the videos and file links on the tab they're looking at (0.2.0).                   |
| `scripting`       | To run that one read-only look at the page when the popup opens. Nothing runs in the background or on other tabs.           |

No host permissions are requested at install. "Signed-in downloads" is off by default: when the person turns it on in settings, the extension asks for the optional `cookies` permission and access to sites (`<all_urls>`), so it can read the cookies for a download's own site and pass them to the Fuselane app on the same computer. Turning it off removes both. The extension never reads browsing history; it looks at a page only when the person opens the popup on it, to list its videos and file links, and keeps nothing.

- **cookies (optional):** with the person's opt-in, reads the cookies for the site a download comes from, so the Fuselane app on this computer can download files that need a sign-in. Never sent anywhere else.
- **Host access `<all_urls>` (optional):** needed by `cookies` to read a download's site cookies. Asked for only when signed-in downloads are switched on.

## Data use (Chrome privacy practices form)

- Collects no user data. Nothing is sold, shared or sent to any server.
- The download's link, file name, size and type are passed to the Fuselane app on the same computer, only to start the download there.
- Privacy policy: https://github.com/ArshPunisher/fuselane/blob/main/docs/PRIVACY.md

## Images

Rendered from the real UI by `node apps/extension/store/render.mjs` into `store/out/`. In the store dashboard, delete the old screenshots and upload these in this order. Captions go in each image's description field where the store has one (Edge, Firefox).

| File | Size | Caption |
| --- | --- | --- |
| `screenshot-1-every-network.jpg` | 1280×800 | One download, every network: Wi-Fi, Ethernet and your phone fused into one fast, verified file. |
| `screenshot-2-from-the-browser.jpg` | 1280×800 | One click gets the page's video, or any file on it; big downloads go to Fuselane by themselves. |
| `screenshot-3-right-click.jpg` | 1280×800 | Right-click any link and choose "Download with Fuselane". |
| `screenshot-4-verified.jpg` | 1280×800 | Every file is checked against its published checksum, and you see how much time each network saved. |
| `screenshot-5-whole-page.jpg` | 1280×800 | Grab every file on a downloads page, picked by type, as one group. |
| `promo-small-440x280.jpg` | 440×280 | Small promo tile |
| `promo-marquee-1400x560.jpg` | 1400×560 | Marquee promo tile |
| `logo-icon-300.png` | 300×300 | Edge Add-ons logo |

- Store icon: `public/icon/128.png` (rendered from `packaging/brand/favicon.svg`).
- Logo kit for social and press: `logo-lockup-on-dark.png`, `logo-lockup-on-light.png`, `logo-lockup-transparent.png`, `logo-icon-512.png`, `logo-mark-white.png`, `social-avatar-800.jpg`.

## After the first upload

The store assigns the extension's id. Send it to the project: the desktop app then
installs the native-messaging host manifest that lets this extension (and only it)
talk to the app (STEPS 7.4).
