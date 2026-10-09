# Store listing (Chrome Web Store, Edge Add-ons, Firefox Add-ons)

Everything the store forms ask for, in one place. Build the upload with
`pnpm --filter @fuselane/extension zip` (files land in `apps/extension/.output/`).

**Chrome Web Store:** item `nggljghjikdkigiekdciocigdnnhponl`, submitted for review 2026-10-09 (publishes automatically once approved).

## Name and summary

- **Name:** Fuselane
- **Summary (132 characters max):** Hands big downloads to the Fuselane app, which splits them across Wi-Fi, Ethernet and a tethered phone at once.
- **Category:** Productivity (Chrome) / Download management (Firefox)
- **Language:** English

## Description

Fuselane is a free, open-source download manager that uses every network your computer has at the same time: Wi-Fi, Ethernet and a phone tethered over USB. It splits each download into parts, fetches them over all of them, and fuses the parts into one verified file.

This extension connects your browser to the Fuselane desktop app:

- Big downloads (1 MB or more) go to Fuselane when the app is running. Anything it can't take stays in the browser, so you never lose a download.
- Right-click a link and choose "Download with Fuselane".
- The toolbar button shows whether the app is connected, with a switch to turn hand-off off.
- A settings page lets you choose the smallest download to hand over, only or never certain sites, and which file types.

You need the Fuselane app on the same computer (macOS, Windows or Linux): https://fuselane.app/

The extension sends nothing to the internet. It only talks to the Fuselane app on your own computer.

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

- Icon: `public/icon/128.png` (rendered from `packaging/brand/favicon.svg`).
- Screenshots (1280×800): `store/screenshot-*.png`, rendered by `store/render.mjs`.

## After the first upload

The store assigns the extension's id. Send it to the project: the desktop app then
installs the native-messaging host manifest that lets this extension (and only it)
talk to the app (STEPS 7.4).
