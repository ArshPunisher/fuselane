# Testing strategy

Principle: **every risk has a test at the cheapest level that can catch it**, and the fast levels run on every PR on all three OSes (L-89). Retries are off; a known bug is written as an expected-failure test (L-94).

## 1. Test levels

| Level | What | Tools | Where it runs | Speed budget |
|---|---|---|---|---|
| **L0 Static** | fmt, clippy (`-D warnings`), tsc, eslint, prettier, cargo-deny (licences, advisories, banned deps and layering), commit lint | rustfmt, clippy, cargo-deny, eslint, tsc | Every PR, Linux | < 3 min |
| **L1 Unit + property** | Pure logic: planner, scheduler, concurrency controller, retry policy, limits, data periods, name sanitizing, Content-Disposition, ETag normalizing, version compare, part sizing, crypto offsets, error catalogue, state-machine transitions | cargo-nextest, proptest, Vitest | Every PR, **macOS + Windows + Linux** | < 5 min |
| **L2 Integration (in-process)** | Engine against the **testkit fault server**: ranges, redirects, stalls at exact bytes, 206 lies, ETag rotation, 429 with Retry-After (seconds and HTTP date), gzip, IP-locked responses, expired links; storage on temp dirs; SQLite migrations; fake S3 for uploads; local torrent swarm | nextest, axum testkit, turmoil, fake `Pinner` | Every PR, all 3 OSes | < 10 min |
| **L3 Network lab** | **Real multi-interface** on Linux: client and server namespaces, 2–3 veth links shaped with `tc netem` (rate, delay, loss), per-link DNS; throughput sum, failover (link down mid-transfer), address change, IP-locked link, captive-portal mimic | `tools/netlab`, sudo on GitHub runners | Every PR touching `netif/transport/engine-*`; full run nightly | < 15 min |
| **L4 App e2e** | Real app: start a download, pause and resume, quit and relaunch, limits, selection actions, error screens; UI with mocked IPC for fast UI tests | tauri-driver (Linux, Windows), Playwright + `mockIPC`, axe | PR: smoke subset; nightly: full | smoke < 10 min |
| **L5 Packaged smoke** | Build the installer on its native runner, install it, launch it headless (`--self-test`): version, DB opens, `Pinner::self_test`, package contents (no foreign native modules, L-73) | CI release-like job | Every PR to `main` (unsigned), every release (signed) | < 20 min |
| **L6 Chaos and soak** | Model-based random sequences (pause, resume, network down/up, disk full, quit, crash, ETag change, stall), shrinking, seed replay; 24 h soak with leak checks (memory, handles, sockets) | proptest state machines, netlab | Nightly (50 seeds), weekly soak | – |
| **L7 Performance** | Benchmarks: throughput with 1/2/3 links in netlab, CPU per Gbit, UI push cost at 10k blocks, scheduler cost | criterion, netlab | Nightly; a regression > 10% fails | – |
| **L8 Real hardware** | Wi-Fi + phone USB tether (iPhone and Android) + Ethernet on real macOS, Windows and Linux machines; 5G tether; HDD vs SSD | Manual checklist, scripted CLI runs | End of each phase, before each release | – |

## 2. Harness rules (from Plexo's lessons)

- Every completed file is checked with **SHA-256** against the source (L-90).
- **Invariant checker** runs on every engine event: bytes covered at most once, block states legal, no stream holds two blocks, totals add up (L-90).
- Fail the test on **warnings, leaked file handles or leaked sockets**, measured on every OS including Windows (L-44, L-91).
- **Isolated temp directories** per test, including any library's defaults (L-70).
- Test knobs exist only behind the `testkit` cargo feature (L-100).
- Skipped tests are **reported** in the CI summary; a required job that skips everything fails (L-93).
- Tests guard risks, not constants or styling (L-92).

## 3. CI pipeline (GitHub Actions)

```text
PR / push to main
 ├─ static (linux)                              L0
 ├─ test-{macos,windows,linux}                  L1 + L2      (required)
 ├─ netlab (linux, sudo)                        L3 subset    (required when core paths change)
 ├─ e2e-smoke-{linux,windows}                   L4 smoke     (required)
 ├─ ui (linux)                                  Vitest + Playwright mockIPC + axe
 └─ package-{macos,windows,linux} (unsigned)    L5           (required on main)
nightly
 ├─ full L3, full L4, L6 chaos (50 seeds), L7 benchmarks
release tag
 └─ build-sign-notarize per OS → L5 on signed artifacts → publish + updater feed + attestations
```

Branch protection: all required jobs green, no force-push to `main` (L-79).

## 4. Gates per phase

| Phase | Must be green before moving on |
|---|---|
| 1 Spikes | Each spike has a written result in `docs/04-plan/spikes/` with numbers; L8 checks on all 3 OSes |
| 2 Core + CLI | L0–L3 green on all OSes; edge-case catalogue (§6) ≥ 90% covered; chaos 50 seeds clean |
| 3 Desktop | L4 smoke + axe clean; UI reviewed with the design-taste and web-design-guidelines skills; 375px…min-window checks |
| 4 Release | L5 on **signed** builds; updater e2e (old → new); first-launch with no security workaround on all 3 OSes |
| 5 Torrents | Local swarm tests; hostile `.torrent` fixtures; public swarm benchmark |
| 6 Uploads | Fake-S3 L2; real R2 staging e2e with 2 links in netlab; crypto vectors shared Rust ↔ web; backend error-status tests (413/415/410/429) |
| 7 Extension | Playwright extension e2e; native-messaging install test per OS; fallback-to-browser test |
| 8 Power features | Feature-specific L2 + L4 |

## 5. Real hardware matrix (L8)

| Setup | macOS | Windows | Linux |
|---|---|---|---|
| Wi-Fi + iPhone USB | ☐ | ☐ | ☐ |
| Wi-Fi + Android USB (macOS needs a driver) | ☐ | ☐ | ☐ |
| Wi-Fi + Ethernet (different ISPs) | ☐ | ☐ | ☐ |
| Wi-Fi + Ethernet (same router), warning shown | ☐ | ☐ | ☐ |
| 5G tether (Jio/Airtel), daily cap reached | ☐ | ☐ | ☐ |
| VPN active | ☐ | ☐ | ☐ |
| HDD destination | ☐ | ☐ | ☐ |
| exFAT USB drive destination | ☐ | ☐ | ☐ |
| Sleep/wake mid-download | ☐ | ☐ | ☐ |
| Unplug tether mid-download | ☐ | ☐ | ☐ |

Record results (speeds, issues) in `docs/05-quality/HARDWARE-RESULTS.md` with date, version and machine.

## 6. Edge-case catalogue

All 120 edge cases in [`../01-research/plexo-forensics.md`](../01-research/plexo-forensics.md) Part 3–5 §B.3 become test IDs `EC-001 … EC-120`, plus Plexo's untested gaps (Part 6 §5) as `EC-2xx`:

- EC-201 429 + Retry-After as an HTTP date
- EC-202 HTTPS with a certificate error, SNI
- EC-203 HTTP/2 server
- EC-204 401 and cookie-gated URL (via forwarded cookies)
- EC-205 gzip `Content-Encoding` body on a ranged request
- EC-206 `multipart/byteranges` reply
- EC-207 files > 4 GiB end to end (sparse server-side generator)
- EC-208 real DNS, per-network answers
- EC-209 ENOSPC partway through
- EC-210 EIO / drive unplugged during a write
- EC-211 slow fsync
- EC-212 hostile torrent peer sending bad pieces
- EC-213 tracker failures and slow magnet metadata
- EC-214 torrent with disk full
- EC-215 a compromised UI sends malformed IPC (fuzz)
- EC-216 `file:` and intranet URLs in the probe
- EC-217 corrupt SQLite DB at startup
- EC-218 migration from every released schema
- EC-219 IP-locked signed URL across 2 networks
- EC-220 system proxy set while pinning

Coverage is tracked in `docs/05-quality/EDGE-CASES.md`, one row per ID mapping to the test name (created in Phase 2).
