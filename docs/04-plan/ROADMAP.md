# Roadmap

Order: **risky things first, a shippable thing early, the differentiators next, breadth last.** Each phase has an exit gate (from [`../05-quality/TESTING.md`](../05-quality/TESTING.md) §4). The detailed steps for each phase are in [STEPS.md](STEPS.md), and where we are now is in [STATUS.md](STATUS.md).

```text
P0 Foundations ─► P1 Risk spikes ─► P2 Core + CLI ─► P3 Desktop app ─► P4 Signed beta (0.1)
                                                                            │
     P10 Android ◄─ P9 1.0 launch ◄─ P8 Power features ◄─ P7 Extension ◄─ P6 Bonded uploads ◄─ P5 Torrents
```

| Phase | Goal | Delivers | Exit gate |
|---|---|---|---|
| **P0 Foundations** | Everything needed to start building | Repo, docs, toolchains, empty workspace that builds on 3 OSes, CI skeleton (L0/L1 on all 3 OSes), free accounts (public GitHub repo, SignPath application, Cloudflare free tier) | CI green on macOS, Windows and Linux with an empty workspace |
| **P1 Risk spikes** | Remove the unknowns that could change the architecture | 8 throwaway spikes (below), each with a written result | Every spike answered; ADRs 0002/0006/0007 accepted or replaced |
| **P2 Core + CLI** | A correct, bonded HTTP download engine you can use from the terminal | `netif`, `transport`, `storage`, `limits`, `engine-http`, `core`, `api`, `testkit`, `netlab`; `fuselane get/ls/pause/resume` | L0–L3 green; edge cases ≥ 90%; chaos clean; ≥ 85% of the summed link speed in netlab |
| **P3 Desktop app** | Plexo parity for HTTP downloads, in a great UI | Tauri app: list, new download, detail, complete and error screens, networks, limits, tray, notifications, settings | Parity checklist §1, 3–9 ticked for HTTP; L4 smoke and axe clean |
| **P4 Beta 0.1** | Ship to real users with no terminal commands | Free signing (SignPath, ad-hoc), install script, Homebrew tap, auto-update, landing page, release workflow | L5 on release builds; updater e2e; real-hardware matrix |
| **P5 Torrents (0.2)** | Torrents with per-network peers | `engine-torrent`, magnet/file input, file picker, peers view, optional seeding | Torrent gates |
| **P6 Bonded uploads (0.3)** | Upload any file fast and get a share link | `engine-upload`, `crypto`, backend Worker, share page, upload UI | Upload gates; abuse controls live |
| **P7 Browser extension (0.4)** | Downloads captured from the browser | WXT extension, native-messaging host + installer registration, pairing fallback | Extension gates; store submissions |
| **P8 Power features (0.5–0.9)** | Daily-use parity with IDM, beyond Plexo | Scheduler, checksums, mirrors, proxy, categories, search, throttle detection, remote API, i18n | Per-feature gates |
| **P9 1.0 launch** | Stable, documented, distributed | Soak, performance pass, docs, winget/Homebrew/Flathub, store listings, launch | All gates; 2 weeks with no P0/P1 bugs |
| **P10 Android** | Bonding on phones | uniffi core, Kotlin UI, foreground service | Android gates |

Extension (P7) and uploads (P6) can swap, or run in parallel once P4 has shipped: both depend only on the core API.

## Phase 1 spikes

Each spike is a throwaway branch `spike/sN-*` with a written result in `docs/04-plan/spikes/SN-*.md`: numbers, what worked, what didn't, and the decision.

| # | Question | Done when |
|---|---|---|
| **S1** | Windows `IP_UNICAST_IF` pins TCP (IPv4/IPv6) on Wi-Fi + USB tether, x64 and ARM64 | Two pinned downloads at once, each link's traffic seen in Task Manager / `netstat`, sum ≥ 85% |
| **S2** | macOS `IP_BOUND_IF` with iPhone USB and Android (TetherKit); with a VPN present | Same measurements; VPN behaviour documented |
| **S3** | Linux `SO_BINDTODEVICE` on Ubuntu 22.04/24.04 and Fedora, and inside Flatpak | Same, plus netlab script working in GitHub Actions |
| **S4** | Per-network DNS on each OS returns that network's answers | Netlab per-link DNS test passes; macOS/Windows APIs proven |
| **S5** | librqbit through the SOCKS5 balancer pins every peer; performance on a public swarm | Peer connections per network logged; throughput within 10% of no-proxy |
| **S6** | R2 multipart from 2 pinned networks in parallel; presigned URLs from different IPs; per-part checksums | 2 GB upload, throughput sums, object SHA-256 matches |
| **S7** | Tauri 2 window on 3 OSes (including Linux NVIDIA/Wayland), `Channel` IPC at 10 Hz with 10k blocks, tray, single instance | Smooth UI, memory measured, workarounds documented |
| **S8** | Native messaging round trip (Chrome + Firefox) to a Rust host on 3 OSes; localhost pairing fallback | Message received with cookies; host manifest install script per OS |
