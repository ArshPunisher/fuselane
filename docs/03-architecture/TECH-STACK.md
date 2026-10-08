# Tech stack

Versions are current as of 2026-10-08 (from [`../01-research/tech-research.md`](../01-research/tech-research.md)). They are pinned in `Cargo.lock` / `pnpm-lock.yaml`; upgrades go through Renovate PRs and must pass CI on all three OSes.

## Chosen

| Area | Choice | Version | Why |
|---|---|---|---|
| Language (core) | **Rust** (stable, pinned in `rust-toolchain.toml`) | – | Native socket control on every OS, small binaries, no GC pauses on the data path, one core for desktop, CLI and Android ([ADR 0003](../adr/0003-rust-core.md)) |
| Async runtime | tokio | 1.53 | Standard; required by hyper, librqbit, quinn |
| Desktop shell | **Tauri 2** | 2.12.1 | ~3–10 MB installers and ~30–80 MB RAM vs Electron's 100 MB+ and 150–300 MB; a Rust backend in the same process ([ADR 0002](../adr/0002-tauri-over-electron.md)) |
| Tauri plugins | updater 2.13, single-instance 2.5, deep-link 2.6, notification 2.5, autostart 2.7, dialog 2.8, opener 2.7, process 2.4, log 2.10 | – | Official, maintained |
| UI | React 19.3 + Vite 8.3 + Tailwind CSS 4.3 + TypeScript | – | Shared with the extension popup and the share page |
| UI components | Base UI / shadcn-style primitives in `packages/ui` | – | Accessible, unstyled; styled by us |
| UI state and data | Zustand + TanStack Query + TanStack Virtual | – | Small stores; virtualized lists for WebKitGTK performance |
| Typed IPC | specta + tauri-specta (Rust → TypeScript) | – | One source of truth; type drift is a compile error |
| HTTP | hyper 1.x + hyper-util with **our own pinned connector**; one client per network | hyper 1.12 | reqwest's `interface()` doesn't exist on Windows, so we own socket creation |
| TLS | rustls + rustls-platform-verifier | 0.23 | OS trust store (corporate CAs work); no OpenSSL |
| Sockets | socket2 (`all`) + windows-sys | 0.6.5 / 0.61 | `bind_device` (Linux), `bind_device_by_index_v4/v6` (macOS), `IP_UNICAST_IF` by hand (Windows) |
| Interfaces | netdev + per-OS change watchers (rtnetlink / SCDynamicStore or nw_path_monitor / NotifyIpInterfaceChange) | netdev 0.46 | Native data, no CLI parsing (L-61) |
| DNS | hickory-resolver with a pinned `RuntimeProvider`; `DNSServiceGetAddrInfo` on macOS | 0.26 | Each network resolves through itself (L-65) |
| Disk | std positional I/O + fs4 (allocate, locks) + `FSCTL_SET_SPARSE` on Windows | fs4 1.1 | Offset writes, preallocation, sparse files |
| Database | SQLite via rusqlite (bundled) | 0.40 | Crash-safe, migrations, queryable history ([ADR 0008](../adr/0008-sqlite-persistence.md)) |
| Torrent | **librqbit** + our per-network peer dialer (in-process SOCKS5 balancer first, an upstream connector hook later). Plan B: libtorrent-rasterbar | 9.0.1 | Pure Rust; libtorrent means maintaining C++ bindings ([ADR 0006](../adr/0006-torrent-engine.md)) |
| Upload client | presigned UploadPart URLs from our backend; rusty-s3 for bring-your-own-bucket | 0.10 | The client never holds our credentials |
| Crypto | aes-gcm (RustCrypto) or aws-lc-rs AEAD; HKDF-SHA256 | aes-gcm 0.11 | AES-256-GCM is native in WebCrypto, so browsers can decrypt |
| Backend | **Cloudflare Workers + R2 + D1**, Hono router, Cron Triggers | wrangler 4.148, hono 4.13 | **$0 egress**, global, cheap ([ADR 0007](../adr/0007-upload-backend.md)) |
| Extension | **WXT** (MV3 for Chromium browsers and Firefox) | 0.21 | One codebase, Vite-based |
| CLI | clap 4 | – | Standard |
| Logging | tracing + tracing-appender | – | Structured, rotating |
| Errors | thiserror (libraries), anyhow only in apps | – | Typed error kinds (see ERRORS.md) |
| Android (later) | uniffi 0.32 + cargo-ndk + Kotlin/Compose | – | Native ConnectivityManager integration |

## Testing and quality tools

| Tool | Use |
|---|---|
| cargo-nextest | Rust test runner (per-test processes, JUnit output) |
| proptest | Property tests: planner, scheduler, limits, part sizing, crypto offsets |
| turmoil | Deterministic simulated network for retry and scheduler logic |
| Linux netns + veth + tc netem | Real multi-interface tests with different speeds, latency and loss |
| toxiproxy | Latency/reset faults on macOS and Windows CI |
| axum (in testkit) | Fault-injecting range server and a fake S3 |
| Vitest + Testing Library | UI unit and component tests |
| Playwright | UI against a mocked IPC; extension e2e in Chromium |
| tauri-driver (WebDriver) | Real-app e2e on Linux and Windows (no macOS support, so macOS gets a packaged smoke test plus manual runs) |
| axe-core | Automated accessibility checks |
| cargo-deny, cargo-audit, clippy, rustfmt, eslint, prettier, tsc | Static gates |
| criterion | Engine micro-benchmarks |

## Considered and rejected

| Option | Why not |
|---|---|
| **Electron** (what Plexo uses) | 80–150 MB installers, 150–300 MB idle RAM, and native socket options need FFI hacks (koffi) anyway |
| Wails (Go) | v3 still in beta; a smaller plugin ecosystem; the Go core is fine, but Rust has better crates for TLS, torrent and FFI to Android |
| Flutter desktop | Weaker desktop-native feel (tray, menus); the UI can't be shared with the extension or the share page |
| Slint / native UI ×3 | 3× the UI cost, or a niche toolkit; reconsider native macOS later |
| reqwest only | `interface()` is cfg'd out on Windows; we need our own connector anyway |
| HTTP/3 for v1 | Little benefit for downloads; reqwest's support is unstable. Revisit after 1.0. |
| WebTorrent / Node | Not in a Rust core |
| AWS S3 as the backend | Egress costs about $0.09/GB, which kills a sharing product |
| JSON state files | Plexo's corruption and reset problems; no migrations or queries |
