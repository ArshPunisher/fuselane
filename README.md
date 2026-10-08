# Fuselane

> Fuse every connection into one fast lane.

Fuselane is a cross-platform download **and upload** manager. It spreads one transfer across every internet connection the computer has: home Wi-Fi, a phone tethered over USB, Ethernet, a second ISP. You get their combined speed, with no VPN, relay server or admin rights.

**Status: planning (Phase 0).** No product code exists yet. This repository holds the research, the decisions and the step-by-step build plan. See [`docs/04-plan/STATUS.md`](docs/04-plan/STATUS.md) for where we are right now.

## What it will do

| Pillar | What it means |
|---|---|
| **Bonded downloads** | HTTP(S) range downloads and torrents split across every network, with resume, integrity checks and per-network limits |
| **Bonded uploads** | Big files uploaded in parallel parts over every network to cloud storage, ending in a share link (WeTransfer-style, but faster) |
| **Browser capture** | Chrome, Edge, Brave and Firefox extensions (Safari later) that hand downloads to Fuselane, cookies and referrer included |
| **Everywhere** | macOS, Windows and Linux from one Rust core, with signed installers, auto-update, a CLI and daemon, and Android later |

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
