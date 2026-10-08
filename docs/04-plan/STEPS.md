# Step-by-step plan

Every step is small enough for **one focused commit, or a short PR of a few commits**, and names its test. Tick a step only when its test exists and passes on all three OSes (unless the step says otherwise). `L-xx` = the rule from [LESSONS-FROM-PLEXO](../02-product/LESSONS-FROM-PLEXO.md) the step must follow. `EC-xxx` = the edge-case test ID.

---

## P0 Foundations

- [x] 0.1 Create the repo, README, .gitignore, .editorconfig
- [x] 0.2 Research reports into `docs/01-research`
- [x] 0.3 Vision, parity checklist, features, lessons
- [x] 0.4 Architecture, tech stack, subsystem designs, ADRs
- [x] 0.5 Testing strategy, roadmap, steps, workflow, CLAUDE.md
- [x] 0.6 Decide the open questions blocking P0/P1 (licence, GitHub org/remote, budget for signing) → [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md)
- [ ] 0.7 Create the GitHub repo and push; protect `main` (status checks required, no force-push)
- [x] 0.8 `rust-toolchain.toml`, a Cargo workspace with empty crates (`netif`, `transport`, `storage`, `limits`, `engine-http`, `core`, `api`, `testkit`) and `apps/cli` printing its version — test: `cargo build` + `cargo nextest run` on 3 OSes
- [x] 0.9 pnpm workspace with `packages/ui` and `packages/api-types` placeholders — test: `pnpm -r build`
- [ ] 0.10 (written and actionlint-clean; tick when green on GitHub) CI: static job (fmt, clippy, cargo-deny, eslint, tsc) + test matrix (macOS, Windows, Linux) — gate: green on an empty workspace
- [x] 0.11 Commit-message lint (conventional commits) and a PR template that lists the `L-xx` rules a PR relies on
- [x] 0.12 Renovate config (grouped weekly updates, lockfile maintenance)
- [ ] 0.13 Free accounts only (ADR 0009): apply to SignPath Foundation once the repo is public; Cloudflare free tier (P6); domain later (owner)

## P1 Risk spikes (throwaway code on `spike/*` branches; results in `docs/04-plan/spikes/`)

- [ ] 1.1 S3 Linux pinning + `tools/netlab` scripts (two shaped links) running in GitHub Actions — L-56, L-93
- [ ] 1.2 S1 Windows `IP_UNICAST_IF` pinning on real hardware (x64; ARM64 if available)
- [ ] 1.3 S2 macOS `IP_BOUND_IF` pinning with iPhone + Android tether, VPN behaviour (**part 1 done** on one uplink: [result](spikes/S2-macos-pinning.md); part 2 needs a tether)
- [ ] 1.4 S4 per-network DNS on each OS
- [ ] 1.5 S7 Tauri window + Channel IPC throughput + tray + single instance on 3 OSes (**macOS done**: [result](spikes/S7-tauri-shell.md); Windows + Linux pending)
- [ ] 1.6 S5 librqbit + SOCKS5 balancer
- [ ] 1.7 S6 R2 multipart from 2 pinned networks
- [ ] 1.8 S8 native messaging round trip on 3 OSes
- [ ] 1.8b S9 Lane Weave prototype (lab route, synthetic feed, contact sheet) — MOTION.md §6 (v1 Weave [superseded](spikes/S9-lane-weave.md); **v2 Fuse Core built**: [result](spikes/S9b-fuse-core.md); tick after the owner approves)
- [ ] 1.9 Update ADRs 0002/0006/0007 (Accepted or superseded) and adjust the design docs to the spike results

## P2 Core + CLI (production code, test-first)

**2A testkit first** (so every later step has a harness)
- [x] 2.1 `testkit::RangeServer` (axum): serves generated content (seeded PRNG, any size incl. > 4 GiB without storing it) with a fault script: stall at byte N, cap ranges, wrong start, overrun, short body, ignore Range, gzip, ETag rotate, 429/503 with Retry-After (seconds/date), 401/403/410, redirect chains, per-client-IP rules — test: its own unit tests
- [ ] 2.2 `testkit::FakePinner` + fake interfaces (aliases 127.0.0.2/3) so engine tests run on every OS
- [ ] 2.3 Invariant checker + SHA-256 helper + handle/socket leak checker usable from any test — L-90, L-91, L-44
- [ ] 2.4 `tools/netlab` promoted from the spike: `netlab up --links "50mbit/20ms,10mbit/80ms/1%"`, per-link DNS, `netlab down-link N`

**2B netif**
- [x] 2.5 Interface model + filter (link-local, no gateway, virtual adapters) — fixtures per OS — L-60
- [ ] 2.6 macOS friendly names + kind via SystemConfiguration — L-61
- [x] 2.7 Windows friendly names + IfType via GetAdaptersAddresses
- [x] 2.8 Linux sysfs + NetworkManager names and kind
- [ ] 2.9 Stable network id from MAC/GUID
- [ ] 2.10 Change watcher per OS (rtnetlink / SCDynamicStore / NotifyIpInterfaceChange) with debounce + polling fallback — netlab: address change detected < 2 s — L-63

**2C transport**
- [x] 2.11 `Pinner` trait + per-OS implementations from the spikes + `self_test()` — L-56, L-57, L-58 (Windows path awaits a native CI run)
- [ ] 2.12 Pinned connector for hyper-util + rustls, keep-alive pool per (network, origin), stale-socket retry — L-66
- [x] 2.13 Per-network DNS resolver per OS with a fallback — EC-208 — L-65 (opt-in: public resolvers pinned per network; OS per-interface resolvers later)
- [x] 2.14 Happy Eyeballs (250 ms, last-good first) and layered deadlines — EC-0xx (dead AAAA) — L-09, L-11
- [ ] 2.15 Link probes: reachability, captive portal, public IP, latency — L-62

**2D storage**
- [x] 2.16 Name sanitizer for all OSes + 255-byte truncation + collision suffixes — table tests — L-39, L-40
- [x] 2.17 Staging file: exclusive create, preallocate/sparse per OS, FAT32 4 GiB check — L-37, L-41
- [ ] 2.18 Writer pool with coalescing + backpressure signal — slow-disk test — L-13, L-27
- [x] 2.19 Publish: complete check, intent record, rename with Windows retries, dir fsync, quarantine/MOTW — L-43, L-45, L-54, L-99
- [x] 2.20 Free-space check before and during; ENOSPC → immediate pause — EC-209 — L-46

**2E limits**
- [x] 2.21 Token buckets (global, per network, per job) — property tests — L-30
- [x] 2.22 Data usage periods (day/week/month, local time, configurable reset), download/upload split — L-35

**2F engine-http**
- [x] 2.23 Probe: `bytes=0-0`, redirects within budget, 416 empty, Content-Disposition parser — EC-001…014 — L-01, L-04, L-08, L-10
- [x] 2.24 Version model + ETag normalization + sample-confirm — L-05, L-06
- [x] 2.25 Planner (pure) — property tests — L-22, L-23
- [x] 2.26 Scheduler (pure): primary, avoid-network, hedge, split — property tests (coverage, no deadlock, caps) — L-24, L-25, L-28
- [x] 2.27 Concurrency controller (pure): grow, refusal ceiling, recovery, disk cap, per-host memory — simulations — L-18, L-26, L-27
- [x] 2.28 Stream worker: request with Range/If-Range/identity, response checks, write, meters — EC-037…044 — L-02, L-03, L-07
- [x] 2.29 Retry policy (pure) + its wiring: busy/strike/link-expired/blocked/unreachable — EC-051…060, EC-201, EC-219 — L-14…L-20
- [ ] 2.30 Slow-stream refresh (< 10% of the network's median for 10 s, max 2 per block) — L-12
- [x] 2.31 Meters, AVG/PEAK/ETA — property tests — L-30…L-33
- [x] 2.32 Checksum verification at publish (pasted hash, `.sha256`, Digest header)

**2G core**
- [x] 2.33 SQLite schema v1 + migrations framework + corrupt-DB handling — EC-217 — L-49, L-50, L-51
- [x] 2.34 Job state machine (pure transition table) — exhaustive tests — L-29
- [ ] 2.35 Queue (1–8 at once, FIFO, resume to front, reorder) 
- [ ] 2.36 Network reconcile per job (on/off/offline/unreachable/failed/limit/blocked) — netlab link down/up — L-16, L-64
- [x] 2.37 Persistence: checkpoint every 15 s + on state changes, fsync before durable — L-55
- [x] 2.38 Resume: validate, reconcile, re-probe — crash-at-any-byte chaos — L-42, L-53
- [ ] 2.39 Sleep/wake and quit (pause all, 3 s deadline, frozen writes) — L-21, L-52
- [ ] 2.40 Event bus with deltas + sequence numbers — benchmark at 10k blocks — L-34
- [ ] 2.41 Error catalogue `describe()` + snapshot test — L-85
- [ ] 2.42 Single-owner lock on the data dir

**2H api + CLI**
- [ ] 2.43 Local JSON-RPC server (UDS / named pipe) + per-user access + schema validation — EC-215 — L-97, L-98
- [ ] 2.44 `fuselane get <url> [--networks] [--streams] [--sha256]`, `ls`, `pause`, `resume`, `rm`, `nets`, `--json` (**get, nets, ls, resume, rm, --sha256 done** and tested; pause = Ctrl-C; `--json` and the daemon remain)
- [ ] 2.45 `fuselaned` headless daemon mode
- [ ] 2.46 Chaos suite (L6) with 50 seeds nightly; benchmarks (L7) nightly
- [ ] 2.47 **Gate review:** edge-case coverage table ≥ 90%, netlab ≥ 85% summed speed, real hardware on 3 OSes (CLI)

## P3 Desktop app (load the design-taste-frontend skill first; review with web-design-guidelines; check with playwright-cli)

- [x] 3.1 Tauri 2 app shell embedding the core; capabilities and CSP locked down (types hand-kept in `apps/desktop/src/lib/types.ts`; tauri-specta generation still to do)
- [ ] 3.2 Design system in `packages/ui` from [DESIGN-SYSTEM.md](../07-design/DESIGN-SYSTEM.md): tokens, light/dark/system, Geist fonts, Phosphor icons, components with every state
- [ ] 3.2b Contact-sheet script (Playwright: keyframes × themes × widths → one PNG) used on every UI PR — MOTION.md §6
- [x] 3.2c Responsive shell: compact (bottom tabs, sheets), regular (rail), wide (three panes); min window 360 × 560
- [ ] 3.3 Downloads list: groups, filters, rows, per-network progress, virtualized — L-81, L-84 (done: groups, rows, per-network bar, content-visibility; to do: filters, true virtualization)
- [ ] 3.4 New download dialog: paste/clipboard/drop, probe states, file name, destination, network chips, streams, warnings — L-62 (done: paste-anywhere, clipboard prefill, destination, inline errors; to do: drop, probe states, folder picker, chips, streams)
- [x] 3.5 Detail screen: hero speed (NumberFlow), ×-faster chip, Fuse Core, Stream graph, network table with streams and orbs
- [x] 3.6 Complete and error screens with the error catalogue actions; Fix link
- [ ] 3.7 Selection toolbar with confirmations; Trash; Reveal
- [ ] 3.8 Networks menu, rename/recolour (done: rename/recolour; to do: health probe, speed test, guided setup), health probe and speed test, guided setup (Windows Wi-Fi policy, Android-on-Mac)
- [x] 3.9 Limits dialog (global, per network, slow mode, data allowances)
- [ ] 3.10 Tray/menu-bar mode, start at login, notifications, dock/taskbar progress, window-state persistence
- [ ] 3.11 Keyboard shortcuts, drag-drop anywhere, single instance + link hand-over
- [ ] 3.12 Settings screen + diagnostics ("Copy diagnostics", log level)
- [ ] 3.13 a11y pass (axe in CI), reduced motion, 24px targets, screen-reader labels — L-83
- [ ] 3.14 tauri-driver e2e smoke (Linux, Windows) + Playwright mockIPC UI tests (Playwright on the demo backend done: 32 tests, WebKit + Chromium)
- [ ] 3.15 **Gate review:** parity checklist (HTTP parts) ticked

## P4 Signed beta 0.1

- [x] 4.1 Release workflow: tag → native per-OS builds → package content check — L-73, L-74
- [ ] 4.2 macOS ad-hoc signing + install script (`install.sh`) + Homebrew tap + illustrated Open Anyway guide — L-72, ADR 0009 (done: ad-hoc signing, tested install script, tap repo + cask generator; to do: illustrated guide)
- [ ] 4.3 Windows signing of binaries and installer via SignPath (unsigned fallback + SmartScreen guide until approved) — L-72
- [ ] 4.4 Linux AppImage/deb/rpm + checksums; Flatpak manifest (submission can wait until 1.0) (done: AppImage x64, deb/rpm x64 + arm64, SHA256SUMS; to do: Flatpak manifest)
- [x] 4.5 Tauri updater: minisign key (backed up offline), our own `latest.json` feed, real semver tests — L-76, L-77, L-78
- [x] 4.6 Packaged smoke (`--self-test`) on signed artifacts per OS — L-75
- [x] 4.7 Updater e2e: install N-1, update to N, data intact (2026-10-08: real beta.1 → beta.2 on macOS; a 31% download resumed in beta.2, byte-exact)
- [x] 4.8 Landing page (`apps/site`): OS/arch detection, signed downloads, checksums (use the design skills)
- [x] 4.9 Local diagnostics bundle + privacy policy (no crash-reporting service, ADR 0009)
- [ ] 4.10 Real-hardware matrix pass → tag `v0.1.0-beta.1`

## P5 Torrents (0.2)

- [x] 5.1 `engine-torrent` crate with librqbit; uTP/UPnP/LSD/web seeds off (librqbit 9 has no web seeds)
- [x] 5.2 SOCKS5 balancer (loopback, auth) + least-busy network choice — L-67 (L-71 waits for incoming peers, off by default)
- [ ] 5.3 Torrent inputs: magnet, URL, file, drag-drop, OS "Open with" (Alternate handler only) (magnet, Open .torrent, paste and drop done; OS handler and .torrent URLs left)
- [x] 5.4 Torrent path safety + hostile fixtures — L-68, L-121
- [x] 5.5 File selection before and during; edge-piece cleanup — L-69
- [x] 5.6 Per-network credit after verification; peers UI
- [x] 5.7 Optional seeding with ratio/time limits; metered-network guard (off by default; tethers and cellular skipped while only seeding)
- [x] 5.8 Local swarm tests (isolated temp dirs) + a public swarm benchmark — L-70 (Debian 13.7 netinst in the real app: 16 MB/s over Ethernet + Wi-Fi, SHA-256 matched)
- [ ] 5.9 Upstream PR: librqbit connector hook + Windows binding

## P6 Bonded uploads (0.3)

- [ ] 6.1 Backend: Worker + Hono + D1 schema + R2 bucket; create/presign/list/complete routes with the error statuses (413/415/409/410/429/503) — tests with Miniflare
- [ ] 6.2 Device-key auth + quotas + rate limits
- [ ] 6.3 `engine-upload`: part planner (R2 equal parts, ≤ 10k parts) — property tests
- [ ] 6.4 Uploader over pinned networks with the shared scheduler + upload concurrency controller; per-part checksum
- [ ] 6.5 Resume: persisted parts + ListParts reconciliation — chaos tests
- [ ] 6.6 `crypto` crate (chunked AES-256-GCM STREAM) + `packages/crypto-web` + shared test vectors
- [ ] 6.7 Share page (Worker-served): metadata, download via presigned GET, in-browser decrypt (Service Worker streaming)
- [ ] 6.8 Link options: expiry, download limit, password; hourly expiry cron + lifecycle rules
- [ ] 6.9 Abuse: report button, takedown runbook, ToS
- [ ] 6.10 Upload UI in the desktop app (drop zone, Launch moment, progress per network, copy link, history of shares)
- [ ] 6.11 Receiver flow: Fuselane opens share links and downloads them bonded
- [ ] 6.12 netlab e2e: 2 links → real R2 staging bucket; summed throughput recorded

## P7 Browser extension (0.4)

- [ ] 7.1 WXT project: Chrome MV3 + Firefox MV3 builds; popup + options with `packages/ui`
- [ ] 7.2 Capture rules engine (size, type, domain, Alt bypass) — unit tests
- [ ] 7.3 Auth forwarding: cookies (incl. partitioned), headers, referrer, UA — never logged
- [ ] 7.4 Native-messaging host mode in the CLI + manifest/registry install per OS from the app installer
- [ ] 7.5 Localhost WebSocket fallback with pairing code + token + Origin/Host checks — T3, T4
- [ ] 7.6 Message schema v1 validated on both sides; fall back to the browser on decline or timeout
- [ ] 7.7 Context menu "Download with Fuselane"
- [ ] 7.8 Playwright extension e2e; `web-ext lint`
- [ ] 7.9 Store listings on the free stores (Edge Add-ons, AMO) with a privacy policy; Chrome Web Store ($5 one-time) only with the owner's OK

## P8 Power features (0.5 → 0.9)

- [ ] 8.1 Scheduler (time windows, per-network schedules, "before midnight", power actions)
- [ ] 8.2 Throttle detection and auto-move
- [ ] 8.3 Mirrors / multi-source + Metalink
- [ ] 8.4 Proxy per network / per download (HTTP, SOCKS5)
- [ ] 8.5 Categories and auto-folders, search, sort
- [ ] 8.6 Per-job speed limit
- [ ] 8.7 Remote control: local web UI + aria2-compatible JSON-RPC (with auth)
- [ ] 8.8 i18n framework + Hindi
- [ ] 8.9 Bring-your-own S3 bucket for uploads (keychain credentials)

## P9 1.0 launch

- [ ] 9.1 Weekly 24 h soak clean for 2 weeks; performance pass
- [ ] 9.2 User docs site; FAQ honest about limits
- [ ] 9.3 winget, Homebrew cask, Flathub
- [ ] 9.4 Launch plan (Product Hunt, Reddit, HN, YouTube/Instagram demos, Indian tech communities)
- [ ] 9.5 Tag `v1.0.0`

## P10 Android: dropped

Not planned ([ADR 0010](../adr/0010-no-android-app.md)). Android phones remain supported as USB-tethered networks for the desktop app.
