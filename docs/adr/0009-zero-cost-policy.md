# 0009. Zero-cost policy: open source, free services only
- Status: Accepted
- Date: 2026-10-08
- Supersedes parts of: VISION "Trustworthy distribution", ADR 0007 (cost assumptions), the signing plan in PLATFORMS/ROADMAP

## Context
The owner's decision (2026-10-08): **open source, use SignPath for Windows signing, and no paid services, tools or subscriptions.** Several plans assumed paid items: the Apple Developer Program ($99/yr), Microsoft Artifact Signing (about $10/mo), crash-reporting SaaS, a paid domain, store fees.

## Decision

| Need | Free approach |
|---|---|
| Licence | **Apache-2.0** (an OSI licence, which SignPath Foundation requires; it includes a patent grant) |
| Code hosting + CI | GitHub, **personal account, public repo** (Actions minutes are free and unlimited for public repos, including macOS runners). The remote gets created once the owner confirms it should be public. |
| Windows signing | **SignPath Foundation** (free for OSS; requires a public repo, an OSI licence and their approval). Until approved: unsigned builds, with a SmartScreen guide on the download page. |
| macOS signing | **No notarization** (that needs a paid Apple Developer account). Builds are **ad-hoc code-signed** (`codesign -s -`, required for Apple Silicon anyway). Install paths, best first: (1) a one-line install script (`curl … \| sh`; files fetched by curl aren't quarantined, so Gatekeeper doesn't block them); (2) our own Homebrew tap; (3) DMG + an illustrated "System Settings → Privacy & Security → Open Anyway" guide. Revisit if donations ever cover the $99. |
| Linux | AppImage/deb/rpm need no signing; release checksums + a free GPG key; Flathub is free |
| Updates | Tauri updater with **minisign** (free) and the feed on GitHub Releases |
| Provenance | GitHub artifact attestations (free for public repos) |
| Crash reporting | **None.** Local rotating logs + a "Copy diagnostics" button only |
| Website | GitHub Pages or Cloudflare Pages (free) on a free subdomain until the owner buys a domain (Q4, later) |
| Upload storage | Cloudflare free tier (Workers, D1, R2 10 GB) with **hard quotas in our backend so the bill stays $0** (R2 may require a card on file; check in spike S6). Fallback: Backblaze B2 free 10 GB (no card). The storage adapter stays provider-neutral. Bring-your-own bucket is supported. |
| Browser stores | Firefox AMO and Edge Add-ons are free. **The Chrome Web Store has a one-time $5 fee.** It's the only exception and needs the owner's OK in P7; until then Chrome and Brave users can load the unpacked build from the release zip (developer mode), with a guide. |
| Android (P10) | F-Droid (free); Google Play has a one-time $25 fee and needs the owner's OK |
| Dev tools | Only free/open-source tools (Rust, Node, pnpm, Playwright, etc.) |

## Consequences
- macOS users see one Gatekeeper step when they install from the DMG. We lower it with the install script and Homebrew tap, plus an honest, illustrated guide. L-72 now reads: **sign everything we can for free, and never make users type security commands**.
- The vision metric "no security workaround" becomes: **no terminal commands needed; at most one "Open Anyway" click on macOS DMG installs; Windows signed once SignPath approves.**
- The backend must enforce quotas strictly enough that free-tier limits are never exceeded (it degrades with clear 413/429/503 messages rather than incurring cost).
