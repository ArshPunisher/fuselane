# Store listing (Chrome Web Store, Edge Add-ons, Firefox Add-ons)

Everything the store forms ask for, in one place. Build the upload with
`pnpm --filter @fuselane/extension zip` (files land in `apps/extension/.output/`).

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

You need the Fuselane app on the same computer (macOS, Windows or Linux): https://arshpunisher.github.io/fuselane/

The extension sends nothing to the internet. It only talks to the Fuselane app on your own computer.

## Single purpose (Chrome)

Hand downloads from the browser to the Fuselane desktop app on the same computer.

## Why each permission is needed

| Permission | Why |
|---|---|
| `downloads` | To see when a download starts, pause it while asking the app, and cancel it if the app takes it over (or resume it if not). |
| `nativeMessaging` | To talk to the Fuselane app on this computer. This is the only place the extension sends anything. |
| `storage` | To remember whether hand-off is switched on. |
| `contextMenus` | To add "Download with Fuselane" to the right-click menu on links. |

No host permissions are requested. The extension doesn't read pages, cookies or browsing history.

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
