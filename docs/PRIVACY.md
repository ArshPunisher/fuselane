# Privacy

Fuselane is a download manager that runs on your computer. It does not have accounts, analytics, telemetry or crash reporting (ADR 0009).

## What stays on your computer

- **Your download list**: links, folders, progress and errors, in a local SQLite file in the app-data folder (`~/Library/Application Support/app.fuselane` on macOS, `%APPDATA%\Fuselane` on Windows, `~/.local/share/fuselane` on Linux). Removing a download removes its entry.
- **Settings**, such as speed limits and the theme, in the same place.
- **Your clipboard (only if you turn it on)**: with **Settings → Catch copied download links**, Fuselane reads the clipboard about once a second to notice a copied link to a file or a magnet, and offers to download it. It reads it only on your computer, keeps only the last text to notice a change, never saves or sends it, and ignores anything that isn't a single download link. It is off by default.

## What goes over the network

- **Your downloads**: Fuselane connects to the servers in the links you give it, over each network you allow, and nowhere else.
- **Sign-in page checks**: about once a minute, and when a network appears, Fuselane asks its own site (`http://arshpunisher.github.io/fuselane/probe`, on GitHub Pages) through each network to see whether a hotel or café sign-in page is in the way. GitHub sees your network's public IP address and the app name, as with update checks; nothing else is sent.
- **Server lookups (only if you turn it on)**: **Settings → Look up servers through each network** asks Cloudflare (1.1.1.1) and Google (8.8.8.8) DNS for each server's address through each network, so every network gets a nearby server. Those resolvers then see the server names (for example `downloads.example.org`), never the files or full links. It is off by default.
- **Fuse Send**: sending a file makes this computer reachable by the receiver: Fuselane listens for connections, asks your router to forward its port (UPnP), and uses the BitTorrent DHT and local network discovery so the receiver can find it. Peers in the DHT can see that your address has the share's random identifier, never its name or contents, which are encrypted with a key that exists only in the link. The share page (`/s`) reads the link in your browser and sends nothing.
- **Update checks** (from the first signed release): a request for a small signed file listing the newest version. It carries no identifier.

Nothing else is sent. There is no background reporting of usage, errors or files.

## Browser extension

The Fuselane browser extension only talks to the Fuselane app on your own computer (native messaging). When a download starts, it passes the app the link, the file name, its size and type, and the page it came from, so the app can download it. When you open its toolbar popup, it looks once at the tab you're on to list that page's videos, file links and feeds, and keeps nothing; it never reads other tabs or your browsing history. It sends nothing to the internet.

**Signed-in downloads (off unless you turn them on).** If you switch on "Send the site's sign-in with downloads" in the extension's settings, your browser first asks you to allow it. From then on, when a download is handed to Fuselane, the extension reads that site's cookies and your browser's User-Agent and passes them to the Fuselane app on your computer, so the app can fetch files that need you to be signed in. The app keeps them in memory only while that download runs, never writes them to disk or logs, and sends them only to the site the file comes from. Switching it off removes the permission.

## Diagnostics

**Settings → Copy diagnostics** builds a text report for bug reports and shows it to you first. It includes the app version, operating system, network device names and kinds, address counts (never the addresses), speed limits, and recent downloads' states and error codes (never links or file names). It only leaves your computer if you paste it somewhere.

## Questions

Open an issue at https://github.com/ArshPunisher/fuselane/issues.
