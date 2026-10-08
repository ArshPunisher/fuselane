# Fuselane features beyond Plexo

Parity with Plexo is tracked in [PARITY-CHECKLIST.md](PARITY-CHECKLIST.md). This file lists what Fuselane adds. Priority: **P0** = needed for 1.0, **P1** = 1.x, **P2** = later. Ph = roadmap phase. Evidence is in [`../01-research/market-research.md`](../01-research/market-research.md).

## A. Bonded uploads and share links (P0, Phase 6) — *the main thing that sets us apart*

Validated by Plexo issue #58, WeTransfer/Smash free-tier limits, and MASV's per-GB pricing. Design in [`../03-architecture/BONDED-UPLOADS.md`](../03-architecture/BONDED-UPLOADS.md).

| Feature | P |
|---|---|
| Drop files or folders, then get a share link; parts upload in parallel over every network | P0 |
| Resume per part after network loss, sleep or restart (the upload ID and finished parts are persisted) | P0 |
| Link options: expiry (1/7/30 days), download limit, password | P0 |
| Optional end-to-end encryption (key in the URL `#fragment`, never sent to the server) | P0 |
| Share page in the browser: preview metadata, download (decrypting in the browser if encrypted) | P0 |
| Receivers using Fuselane download the link bonded too | P0 |
| Notification and copy-link when the upload is done; "upload failed overnight" can't happen silently | P0 |
| Multi-file bundle: one link for many files, with an optional zip on download | P1 |
| Bring your own bucket (S3, R2, B2, MinIO), with credentials in the OS keychain | P1 |
| Dropbox concurrent upload sessions as a destination | P2 |
| Watch folders: auto-upload new exports (creator workflow) | P2 |
| Branded share pages for creators and teams | P2 |

## B. Browser capture (P0, Phase 7)

Design in [`../03-architecture/BROWSER-EXTENSION.md`](../03-architecture/BROWSER-EXTENSION.md). Plexo's PR #18 for this is still unmerged.

| Feature | P |
|---|---|
| Chrome, Edge, Brave and Opera (MV3) and Firefox extensions | P0 |
| Capture downloads automatically by size, type and domain rules; hold Alt to bypass | P0 |
| Right-click "Download with Fuselane" on links and media | P0 |
| Forward cookies (including partitioned), Referer, User-Agent and auth headers for logged-in downloads | P0 |
| Native messaging channel plus a paired localhost fallback | P0 |
| "Upload with Fuselane" from the extension (send a page's file to a share link) | P1 |
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
