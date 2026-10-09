# Fuselane features beyond Plexo

Parity with Plexo is tracked in [PARITY-CHECKLIST.md](PARITY-CHECKLIST.md). This file lists what Fuselane adds. Priority: **P0** = needed for 1.0, **P1** = 1.x, **P2** = later. Ph = roadmap phase. Evidence is in [`../01-research/market-research.md`](../01-research/market-research.md).

## A. Fuse Send: direct sharing over every network (P0, Phase 6) — *the main thing that sets us apart*

Validated by Plexo issue #58 and WeTransfer/Smash free-tier limits. No cloud and no plans: files go straight from the sender to the receiver ([`../03-architecture/FUSE-SEND.md`](../03-architecture/FUSE-SEND.md), [ADR 0011](../adr/0011-fuse-send-p2p.md)).

| Feature | P |
|---|---|
| Drop files or folders, get a link; the transfer uses every network on both sides | P0 |
| Always end-to-end encrypted (key only in the link's `#fragment`) | P0 |
| Resume after network loss, sleep or restart on either side | P0 |
| Clear status: "keep Fuselane open", progress per network, "arrived" | P0 |
| Every failure says why and what to try (sender offline, can't reach each other, disk full) | P0 |
| Stop sharing any time; optional "stop after the first full download" | P0 |
| QR code for the link | P1 |
| Same-network sending without the DHT (mDNS discovery) | P1 |
| Bring your own cloud (the user's own Dropbox or S3, at their cost) for offline handover | P2 |
| Receiving in a browser without the app (needs WebRTC) | P3, research |

## B. Browser capture (P0, Phase 7)

Design in [`../03-architecture/BROWSER-EXTENSION.md`](../03-architecture/BROWSER-EXTENSION.md). Plexo's PR #18 for this is still unmerged.

| Feature | P |
|---|---|
| Chrome, Edge, Brave and Opera (MV3) and Firefox extensions | P0 |
| Capture downloads automatically by size, type and domain rules; hold Alt to bypass | P0 |
| Right-click "Download with Fuselane" on links and media | P0 |
| Forward cookies (including partitioned), Referer, User-Agent and auth headers for logged-in downloads | P0 |
| Native messaging channel plus a paired localhost fallback | P0 |
| "Send with Fuselane" from the extension (share a page's file with a Fuse Send link) | P2 |
| Refresh an expired link automatically by re-asking the page | P1 |
| Safari extension (Xcode wrapper) | P2 |

## C. Trust and distribution (P0, Phase 4)

| Feature | P |
|---|---|
| macOS: ad-hoc signing, one-line install script, own Homebrew tap, illustrated "Open Anyway" guide (no paid notarization, ADR 0009) | P0 |
| Windows Authenticode signing through SignPath Foundation (free for open source) | P0 |
| Signed auto-update with real semver (pre-release to release ordering tested) | P0 |
| Packaged app smoke-tested in CI on all three OSes | P0 |
| SHA-256 checksums and build provenance attestations on every release | P0 |
| winget, Homebrew cask, Flathub | P1 |

## D. Engine features (Phases 2 and 8)

| Feature | P | Ph |
|---|---|---|
| Correct per-interface pinning on every OS: `IP_UNICAST_IF` (Windows), `IP_BOUND_IF` (macOS), `SO_BINDTODEVICE` (Linux) | P0 | 2 |
| Per-interface DNS, so each network resolves through itself | P0 | 2 |
| Probe and drop networks that get 403 or a redirect for IP-locked links | P0 | 2 |
| Checksum verification: paste one, read a `.sha256` file, or use the Digest header; re-fetch only bad blocks | P0 | 2/8 |
| Preallocation where supported, crash-safe fsync policy | P0 | 2 |
| HTTP/2 off by default for segmented downloads (HTTP/1.1 with several connections); HTTP/2 for single streams | P1 | 2 |
| Proxy per network or per download (HTTP and SOCKS5) | P1 | 8 |
| Mirrors and multi-source downloads (several URLs for one file), Metalink | P1 | 8 |
| HLS/DASH grabbing of non-DRM streams, with segments fetched across networks | P2 | 9+ |
| Sequential "stream while downloading" mode for media | P2 | 9+ |
| MPTCP opt-in on Linux | P2 | 9+ |

## E. Daily-use features (Phase 8 unless noted)

| Feature | P |
|---|---|
| Scheduler: start/stop windows, per-network schedules (for example the phone only after 11 pm), "use what's left before midnight" | P0 |
| Power actions when done: sleep, shut down, quit | P1 |
| Categories and auto-folders by type or site, with rules | P1 |
| Search and sort in the list | P0 (Ph 3) |
| Tray / menu-bar mode and start at login | P0 (Ph 3) |
| Throttle detection (a link drops to about 64 kbps), then move to other networks and warn | P1 |
| Remote control: local web UI plus an aria2-compatible JSON-RPC (so AriaNg and similar tools work) | P1 |
| CLI: `fuselane get <url>`, `fuselane send <file>`, `fuselane ls`, scripting-friendly JSON output | P0 (Ph 2) |
| Headless daemon `fuselaned` for servers, NAS and home labs | P1 |
| Diagnostics: rotating logs, a "Copy diagnostics" button, no crash-reporting service (ADR 0009) | P0 (Ph 3) |
| Internationalization (English first; Hindi next) | P1 |
| Do this one now: one download gets every network, the rest wait (B9.1) | P0 |
| Groups of downloads added together, with one progress and one notice (B9.2) | P0 |
| Find files on a web page and add them by type (B9.3) | P1 |
| Ready by: a deadline per download, earliest first, On track / At risk (B9.4) | P1 |
| The phone only for long downloads, with a threshold (B9.5) | P0 |
| What each network saved, shown after a download (B9.6) | P1 |
| Checksums found next to the link (`.sha256`, `SHA256SUMS`) and a Verified badge (B9.7) | P1 |
| Already downloaded: same name and size already on disk (B9.8) | P1 |
| Hand a paused download to another Fuselane over Nearby (B9.9) | P1 |
| Battery aware: on low battery leave out the phone or pause (B9.10) | P2 |

## F. Platforms beyond desktop

| Feature | P | Ph |
|---|---|---|
| Android app bonding Wi-Fi and cellular (Rust core via uniffi, Kotlin UI) | P1 | 10 |
| Android as a "network donor" or remote control for the desktop | P2 | 10+ |
| iOS (foreground-only bonding) | P2 | – |
| Optional relay "boost everything" mode (system or per-app, bring your own VPS) | P2 | – |

## Explicitly out of scope

- Cracking or bypassing DRM, captchas, or paywalls.
- Ads, bundled offers, or selling data. These are what damaged trust in JDownloader and others.
