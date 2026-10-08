# Plexo parity checklist

Every user-facing feature Plexo rc.14 has. Fuselane must match each one (**=**) or do better (**+**) by the phase shown. Tick an item only when it is implemented **and** covered by a test. Phase numbers refer to [`../04-plan/ROADMAP.md`](../04-plan/ROADMAP.md).

Source of truth for Plexo's behaviour: [`../01-research/plexo-forensics.md`](../01-research/plexo-forensics.md) Part 1. This is *behaviour* parity only; nothing is copied from Plexo's code ([ADR 0005](../adr/0005-clean-room-policy.md)).

Legend: `[ ]` todo · `[x]` done and tested · **=** match · **+** better · Ph = phase

---

## 1. Direct HTTP(S) downloads

| ✓ | Feature (Plexo behaviour) | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Probe with `Range: bytes=0-0`; range support only if the server answers 206 | + | Same, plus `If-Range` on every segment and `Accept-Encoding: identity` | 2 |
| [ ] | Size from Content-Range; a `416 bytes */0` is a valid empty file | = | | 2 |
| [ ] | Follow up to 5 redirects within one time budget | = | | 2 |
| [ ] | Torrent detected by MIME type or `.torrent` extension | = | | 5 |
| [ ] | Content-Disposition per RFC 6266/5987 (`filename*` wins; ISO-8859-1) | = | | 2 |
| [ ] | Name from the URL path, else "download" | = | | 2 |
| [ ] | Editable file name before start | = | | 3 |
| [ ] | Block size 1–8 MiB, about 2 blocks per stream | + | Same starting point, tuned from network-lab benchmarks | 2 |
| [ ] | Streams per network: Auto (8 → 16 → 32) or a fixed 4/8/16/32 | + | Same, plus a remembered per-host ceiling | 2 |
| [ ] | Interleave stream starts across networks | = | | 2 |
| [ ] | Shared block queue; faster networks take more | = | | 2 |
| [ ] | Race slow tail blocks (max 2 hedges per block, another network first) | + | Same, plus splitting a large in-flight block (true work stealing) | 2 |
| [ ] | Back off when a server refuses streams; regain one a minute | = | | 2 |
| [ ] | Disk-aware Auto (halve streams while the disk is behind) | + | Same, plus write coalescing and an "HDD detected" hint | 2 |
| [ ] | Pause and resume, with validators checked (ETag/Last-Modified, byte sampling) | = | | 2 |
| [ ] | Survives quit, crash and sleep; restored downloads come back paused | = | | 2 |
| [ ] | Keeps the computer awake while downloading | + | Same, plus a setting to turn it off | 3 |
| [ ] | Fix an expired link (same size required) | + | Same, plus automatic refresh through the browser extension | 3 / 7 |
| [ ] | Backs off on 408/429/5xx for up to 5 min and honours Retry-After | = | | 2 |
| [ ] | Staging file `<name>.<ext>` next to the destination, renamed on completion | = | Our suffix is `.fuselane` | 2 |
| [ ] | Free-space check before start | + | Also checks during the download, plus preallocation | 2 |
| [ ] | Sparse staging file on NTFS | = | | 2 |
| [ ] | Name collisions get `name (N).ext`; 255-byte limit; Windows reserved names | = | | 2 |
| [ ] | Single-stream fallback for unknown size or no range support | = | | 2 |

## 2. Torrents

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Magnet, http(s) `.torrent` and local `.torrent` (open dialog, drag-drop, "Open with") | = | | 5 |
| [ ] | Magnet metadata with a 3 min timeout; a newer lookup cancels the older | = | | 5 |
| [ ] | `.torrent` up to 10 MB | = | | 5 |
| [ ] | Reject v2-only torrents | + | librqbit may support v2 later; at minimum, a clear message | 5 |
| [ ] | Torrent paths made safe (traversal, reserved names, case collisions) | = | | 5 |
| [ ] | Choose files before start, and change the choice while running | = | | 5 |
| [ ] | Peers spread across networks (each new peer goes to the network with the fewest) | + | Weighted by each network's measured throughput | 5 |
| [ ] | Uploads to peers on every network | = | | 5 |
| [ ] | Per-peer rows: client name, speeds, bytes, share | = | | 5 |
| [ ] | Bytes credited to networks once a piece is verified | = | | 5 |
| [ ] | — (Plexo has no seeding after completion) | + | Optional seeding with a ratio/time limit, off by default | 5 |
| [ ] | — (Plexo has no uTP, UPnP, PEX or LSD) | + | Per-interface where possible; UPnP per gateway | 5+ |

## 3. Queue

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Downloads at once: default 2, range 1–8 | = | | 2 |
| [ ] | FIFO queue; a resumed download goes to the front | = | | 2 |
| [ ] | Queued time counts as paused time | = | | 2 |
| [ ] | — (Plexo can't reorder the queue) | + | Drag to reorder, "move to top", priorities | 3 |

## 4. Downloads list and history

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Filter: All / In progress / Finished / Needs attention, with counts | = | | 3 |
| [ ] | Groups: Downloading, Queued, Paused, Needs attention, Finished | = | | 3 |
| [ ] | Row: type badge, name, network dots, speed, per-network segmented progress, detail line | = | | 3 |
| [ ] | Row actions: Pause, Resume, Fix link, Retry, Download again | = | | 3 |
| [ ] | Multi-select toolbar: Pause / Resume / Retry / Remove / Cancel… / Move to Trash… | = | | 3 |
| [ ] | Confirmations for destructive actions | = | | 3 |
| [ ] | History survives restart (Plexo caps it at 500) | + | SQLite, no small cap, searchable | 3 |
| [ ] | "Missing" marker when a finished file was moved or deleted | = | | 3 |
| [ ] | Reveal in Finder/folder | = | | 3 |
| [ ] | Trash only the files the download created | = | | 3 |
| [ ] | Empty states: no downloads / no networks / filtered | = | | 3 |
| [ ] | Clipboard link autofill when New download opens | = | | 3 |
| [ ] | — | + | Search, sort, categories (see FEATURES.md) | 8 |

## 5. Download detail views

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Hero: total speed, AVG/PEAK, slow-mode or limit suffix, "slow drive" warning | = | | 3 |
| [ ] | "N× faster than Wi-Fi alone" chip | = | | 3 |
| [ ] | Throughput chart for the last 60 s, stacked by network, with hover readout | = | | 3 |
| [ ] | Block grid coloured by the network that fetched each block, with hover detail | = | | 3 |
| [ ] | Network table: status, progress, share, speed, bytes; expandable stream/peer rows | = | | 3 |
| [ ] | BACKUP badge on hedge streams | = | | 3 |
| [ ] | Complete screen: stats, chart, per-network contribution | = | | 3 |
| [ ] | Error screen: plain-language message, Fix link / Retry / Download again, copy link | = | | 3 |
| [ ] | Window title shows progress | + | Also dock/taskbar progress and badge | 3 |

## 6. Limits

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Total speed limit | = | | 2 |
| [ ] | Per-network speed limit | = | | 2 |
| [ ] | Slow mode (default 2 MiB/s, one click) | = | | 3 |
| [ ] | Data allowance per network per day/week/month (local time, weeks start Monday) | + | Configurable reset day/time, upload bytes counted separately | 2 |
| [ ] | Reset usage / Remove limits (with confirmation) | = | | 3 |
| [ ] | Usage kept across restarts | = | | 2 |
| [ ] | — (no per-download limit in Plexo) | + | Per-download speed limit | 8 |
| [ ] | — | + | Throttle detection (for example 64 kbps after a cap) and automatic move to other networks | 8 |

## 7. Networks

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Friendly names (Wi-Fi, Ethernet, iPhone USB) on every OS | + | Native APIs, not CLI parsing or PowerShell | 2 |
| [ ] | Kind: wifi / usb / ethernet / bridge / other | + | Plus cellular/WWAN, VPN, virtual (filtered out by default) | 2 |
| [ ] | Rename and recolour a network (8 swatches) | = | | 3 |
| [ ] | Networks appearing or vanishing mid-download are handled | + | OS change notifications, not polling every second | 2 |
| [ ] | Same-subnet warning | + | Also detects a shared public IP / shared upstream through per-network probes | 2 |
| [ ] | Dialog when per-interface binding isn't supported (Linux < 5.7) | = | | 3 |
| [ ] | — | + | Per-network health check: internet reachable, captive portal, latency, speed test | 3 |
| [ ] | — | + | Guided setup: Windows "keep Wi-Fi with Ethernet", Android tethering on macOS | 3 |

## 8. OS integration

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Single instance; a second launch hands its link to the first | = | | 3 |
| [ ] | Magnet and `.torrent` offered as an Alternate handler, never the default | = | | 5 |
| [ ] | Notifications for complete and failed | = | | 3 |
| [ ] | Move to Trash / Recycle Bin | = | | 3 |
| [ ] | Wake from sleep refreshes connections | = | | 2 |
| [ ] | Quit pauses and persists everything, with a hard deadline | = | | 2 |
| [ ] | Drag-drop of links and `.torrent` files; ⌘V/Ctrl+V outside fields opens New download | = | | 3 |
| [ ] | ⌘N / Ctrl+N | + | Full shortcut set (pause, select all, delete, search) | 3 |
| [ ] | — | + | Tray / menu-bar mode, start at login, window-state persistence | 3 |
| [ ] | — | + | Quarantine (macOS) / Mark of the Web (Windows) on downloaded files | 2 |

## 9. Updates, theming, accessibility, formatting

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | Update notice | + | Signed auto-update (Tauri updater), real semver, our own feed | 4 |
| [ ] | Light/dark theme following the OS on first run | + | Also a System option | 3 |
| [ ] | Reduced-motion support, 24px hit targets, ARIA roles | + | Plus an automated axe audit in CI | 3 |
| [ ] | Speed unit toggle (MB/s or Mbps) | = | | 3 |
| [ ] | Data usage rounds *down* | = | | 3 |
| [ ] | Smoothed ETA (falls 30%/s, rises 10%/s) | = | | 2 |
| [ ] | Peak = best 5 s average, never below AVG | = | | 2 |

## 10. Distribution

| ✓ | Feature | = / + | Fuselane | Ph |
|---|---|---|---|---|
| [ ] | macOS x64 + arm64 | + | Universal, ad-hoc signed, install script + Homebrew tap (ADR 0009) | 4 |
| [ ] | Windows x64 + ARM64 installer that lets you choose the folder | + | **Signed via SignPath**, plus winget | 4 |
| [ ] | Linux AppImage, deb, rpm (x64 + arm64) | + | Plus Flatpak, plus a signed apt/rpm repo later | 4 |
| [ ] | Landing site with OS/arch detection | + | Plus a signed download page and checksums | 4 |
