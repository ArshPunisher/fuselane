# Fuselane

> Fuse every connection into one fast lane.

Fuselane is a cross-platform download **and upload** manager. It spreads one transfer across every internet connection the computer has: home Wi-Fi, a phone tethered over USB, Ethernet, a second ISP. You get their combined speed, with no VPN, relay server or admin rights.

**Status: public beta (0.1).** Bonded HTTP(S) downloads work on macOS, Windows and Linux, in a desktop app and a CLI. Torrents, bonded uploads and the browser extension come next. See [`docs/04-plan/STATUS.md`](docs/04-plan/STATUS.md).

## Install the beta

| System | How |
|---|---|
| macOS 13.3+ | `curl -fsSL https://raw.githubusercontent.com/ArshPunisher/fuselane/main/packaging/macos/install.sh \| sh`, or `brew install --cask arshpunisher/fuselane/fuselane`, or the `.dmg` |
| Windows 10/11 x64 | `Fuselane_<version>_windows-x64-setup.exe` |
| Linux x64 / arm64 | `.AppImage` (x64), `.deb` or `.rpm` |
| CLI | `fuselane-cli_<version>_<system>` |

All files are on [Releases](https://github.com/ArshPunisher/fuselane/releases) with `SHA256SUMS`. The app updates itself from a signed feed. It sends nothing else: see [PRIVACY.md](docs/PRIVACY.md).

## Code signing policy

Windows releases are to be signed through the [SignPath Foundation](https://signpath.org) (free code signing for open source; application in progress). Until then, Windows installers are unsigned and SmartScreen may warn.

- Every release is built by GitHub Actions from this public repository; nothing is built or signed on a personal machine.
- Committers and reviewers: [@ArshPunisher](https://github.com/ArshPunisher). Approver of each signed release: [@ArshPunisher](https://github.com/ArshPunisher).
- macOS builds are ad-hoc signed; app updates are signed with the project's own update key and verified by the app before installing.
- Privacy: the app collects no user data ([PRIVACY.md](docs/PRIVACY.md)).

## What it does

| Pillar | What it means |
|---|---|
| **Bonded downloads** | HTTP(S) range downloads and torrents split across every network, with resume, integrity checks and per-network limits |
| **Bonded uploads** | Big files uploaded in parallel parts over every network to cloud storage, ending in a share link (WeTransfer-style, but faster) |
| **Browser capture** | Chrome, Edge, Brave and Firefox extensions (Safari later) that hand downloads to Fuselane, cookies and referrer included |
| **Everywhere** | macOS, Windows and Linux from one Rust core, with installers, signed auto-updates and a CLI (no Android app: [ADR 0010](docs/adr/0010-no-android-app.md)) |

## Documentation map

| Folder | What's inside |
|---|---|
| [`docs/00-overview`](docs/00-overview) | Vision, users, positioning, principles, non-goals |
| [`docs/01-research`](docs/01-research) | Plexo analysis and forensic study, market research, tech research, naming |
| [`docs/02-product`](docs/02-product) | Plexo parity checklist, new features, lessons from Plexo |
| [`docs/03-architecture`](docs/03-architecture) | Architecture, tech stack, engine, networking, torrent, uploads, extension, security, errors, platforms |
| [`docs/04-plan`](docs/04-plan) | Roadmap, detailed steps, live status, open questions |
| [`docs/05-quality`](docs/05-quality) | Testing strategy, test levels, hardware matrix |
| [`docs/06-process`](docs/06-process) | How we work: workflow, commits, branches, releases |
| [`docs/07-design`](docs/07-design) | Design system ("Lanes → Fuse") and motion language |
| [`docs/adr`](docs/adr) | Architecture Decision Records |

Start with [`docs/00-overview/VISION.md`](docs/00-overview/VISION.md), then [`docs/04-plan/ROADMAP.md`](docs/04-plan/ROADMAP.md).

## Inspiration and originality

Fuselane was inspired by [Plexo](https://github.com/anmolkapil/plexo) (MIT), which proved that multi-network downloading without a VPN is wanted. We studied its behaviour, bugs and history to learn from them. **We do not copy its code.** Fuselane is a clean-room implementation in a different language (Rust) with a different architecture. See [ADR 0005](docs/adr/0005-clean-room-policy.md).

## Licence

[Apache-2.0](LICENSE). Fuselane is free and open source, and it uses only free services ([ADR 0009](docs/adr/0009-zero-cost-policy.md)).
