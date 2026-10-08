# Fuselane: vision

## One line

**IDM plus WeTransfer, on every network you have.** A free, open-source, cross-platform download and upload manager that stripes one file across Wi-Fi, a tethered phone and Ethernet, in both directions, without a paid VPN.

## The problem

1. **Connections sit idle.** A laptop on home Wi-Fi with a 5G phone in the pocket uses one connection. The OS sends everything through a single default route.
2. **Uploads are the real bottleneck.** Home lines are asymmetric. Creators lose evenings to 20 GB uploads that fail overnight without anyone noticing. WeTransfer's free plan is now 3 GB or 10 transfers a month, and MASV charges $0.25/GB for "accelerated" uploads.
3. **Download managers stopped evolving.**
   - IDM is Windows-only and paid.
   - Motrix and aria2 are unmaintained.
   - FDM and JDownloader had their sites compromised.
   - Plexo proved bonding is wanted (about 1.5k stars in 3 weeks), but it's unsigned, download-only and has no browser capture.
4. **Mobile data caps are daily** (India: 1.5–2 GB/day packs that drop to 64 kbps, reset at midnight, often with unlimited 5G that can be tethered). People need a tool that uses exactly what's left today, before it expires.

## Who it's for

| Persona | Need | What Fuselane gives them |
|---|---|---|
| **Heavy downloader** (games, ISOs, courses, movies) | Faster, resumable, reliable downloads | Bonded downloads, queue, browser capture, resume across restarts |
| **Creator or videographer** | Send 5–100 GB to a client quickly | Bonded uploads, a share link, resume per part, notification when done |
| **Mobile-data user** (Jio/Airtel tether + broadband) | Use the daily quota fully without overspending | Per-network daily caps, "before midnight" scheduling, throttle detection |
| **Developer or AI hobbyist** | Large model weights and datasets, scripted | CLI and daemon, checksum verification, mirrors |
| **IDM switcher on macOS or Linux** | IDM's features off Windows | Browser capture, categories, scheduler, a familiar list UI |

## Pillars

1. **Bond everything.** Downloads (HTTP range, torrents) and uploads (S3-compatible multipart) over every usable network. Each network's sockets are pinned to it properly on every OS.
2. **Never corrupt, never lose.** Check every server answer, check against the disk before resuming, persist crash-safe, and never mark a short file complete.
3. **Capture from the browser.** Extensions hand downloads over with their cookies and headers, so authenticated links work.
4. **Trustworthy distribution at zero cost** ([ADR 0009](../adr/0009-zero-cost-policy.md)). Windows signed through SignPath (free for open source); macOS ad-hoc signed with a one-line install script and a Homebrew tap; signed auto-updates (minisign); reproducible CI builds with provenance. Users never have to type security commands.
5. **Light and fast.** A native Rust core (a few MB, low RAM), with a web UI only for the window.
6. **Honest UX.** Say when bonding *can't* help (same router, same subnet, an IP-locked link) and show each network's real contribution.

## Principles

- **The core is a library.** One Rust core drives the desktop app, the CLI, the daemon and (later) Android. The UI never holds engine logic.
- **Decide with pure functions.** Scheduling, concurrency, limits and retry policy are pure functions of a snapshot, so they can be property-tested.
- **Every failure has an owner.** Server, network, disk or sleep: each is handled differently, and each tells the user what to do.
- **Polite to servers.** Adaptive connection counts, honour Retry-After, back off on refusals, and remember per-host limits.
- **Respect the user's machine.** No admin rights. Never take over default handlers. Keep Gatekeeper/SmartScreen quarantine marks on downloaded files.
- **Ship small, test everything.** Small commits, CI on all three OSes from day one, and a fast test suite on every PR.

## Non-goals (for 1.0)

- System-wide or per-app bonding through a relay or VPN. It's a different product with server costs and latency. It may come later as an optional "boost everything" mode.
- Live-stream upload bonding. It's latency-sensitive and needs a relay.
- DRM content or site-specific video ripping. HLS/DASH grabbing of non-DRM streams comes after 1.0.
- iOS. iOS allows bonding only while the app is in the foreground, so it isn't worth it before Android.

## How we'll know it works

| Metric | Target |
|---|---|
| Combined speed with 2 independent links | ≥ 85% of the sum of each link alone (measured in the network lab and on real hardware) |
| Corrupt or short files marked complete | **0**, checked by SHA-256 in every e2e test |
| First-launch friction | No terminal commands; at most one "Open Anyway" click for macOS DMG installs; none with the install script or Homebrew |
| Resume after crash, sleep, network loss or link expiry | 100% of chaos-suite scenarios |
| Idle RAM of the desktop app | < 120 MB including the webview |
| Installer size | < 20 MB |
| CI | All three OSes green on every PR |

## Positioning against alternatives

| | Fuselane | Plexo | IDM | Speedify | WeTransfer |
|---|---|---|---|---|---|
| Bonded downloads | ✅ | ✅ | ❌ | ✅ (VPN) | – |
| Bonded uploads + share links | ✅ | ❌ | ❌ | ✅ (VPN, no links) | ❌ (one link) |
| Browser capture | ✅ | ❌ | ✅ | – | – |
| macOS / Windows / Linux | ✅ | ✅ | Windows only | ✅ | Web |
| Signed + auto-update | ✅ (free signing, ADR 0009) | ❌ | ✅ | ✅ | – |
| Price | Free and open source (Apache-2.0) | Free | ₹2,390 lifetime | $90/yr | Free 3 GB/mo |
