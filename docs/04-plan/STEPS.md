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
- [x] 0.7 Create the GitHub repo and push; protect `main` (no force-push, no deletion)
- [x] 0.8 `rust-toolchain.toml`, a Cargo workspace with empty crates (`netif`, `transport`, `storage`, `limits`, `engine-http`, `core`, `api`, `testkit`) and `apps/cli` printing its version — test: `cargo build` + `cargo nextest run` on 3 OSes
- [x] 0.9 pnpm workspace with `packages/ui` and `packages/api-types` placeholders — test: `pnpm -r build`
- [ ] 0.10 (written and actionlint-clean; tick when green on GitHub) CI: static job (fmt, clippy, cargo-deny, eslint, tsc) + test matrix (macOS, Windows, Linux) — gate: green on an empty workspace
- [x] 0.11 Commit-message lint (conventional commits) and a PR template that lists the `L-xx` rules a PR relies on
- [x] 0.12 Renovate config (grouped weekly updates, lockfile maintenance)
- [ ] 0.13 Free accounts only (ADR 0009): apply to SignPath Foundation once the repo is public; no hosted services (ADR 0011); domain later (owner)

## P1 Risk spikes (throwaway code on `spike/*` branches; results in `docs/04-plan/spikes/`)

- [ ] 1.1 S3 Linux pinning + `tools/netlab` scripts (two shaped links) running in GitHub Actions — L-56, L-93
- [ ] 1.2 S1 Windows `IP_UNICAST_IF` pinning on real hardware (x64; ARM64 if available)
- [ ] 1.3 S2 macOS `IP_BOUND_IF` pinning with iPhone + Android tether, VPN behaviour (**part 1 done** on one uplink: [result](spikes/S2-macos-pinning.md); part 2 needs a tether)
- [ ] 1.4 S4 per-network DNS on each OS
- [ ] 1.5 S7 Tauri window + Channel IPC throughput + tray + single instance on 3 OSes (**macOS done**: [result](spikes/S7-tauri-shell.md); Windows + Linux pending)
- [x] 1.6 S5 librqbit + SOCKS5 balancer (built for real in P5)
- [x] ~~1.7 S6 R2 multipart from 2 pinned networks~~ dropped with cloud uploads (ADR 0011)
- [ ] 1.7b S7 Fuse Send reachability: two machines on home Wi-Fi, a phone hotspot and CGNAT find and connect over DHT (UPnP, uTP, TCP); record how often each pair works
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
- [x] 2.15 Link probes: reachability and captive portal (Fuselane's own site, every minute and on network changes; sign-in networks left out of downloads). Public IP and latency not needed yet — L-62

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
- [x] 2.42 Single-owner lock: one owner per download (OS file lock per job), so the app and the CLI never run the same download

**2H api + CLI**
- [x] 2.43 Local API (one JSON per line over a Unix socket / named pipe), owner-only socket + peer uid check, offers validated (shared vectors) — L-97
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
- [x] 3.8 Networks menu, rename/recolour, health probe and speed test, guided setup (Windows Wi-Fi policy, Android-on-Mac) (rename/recolour; reach checks every minute; network check B10.1; a welcome on the first launch with platform tips)
- [x] 3.9 Limits dialog (global, per network, slow mode, data allowances)
- [ ] 3.10 Tray/menu-bar mode, start at login, notifications, dock/taskbar progress, window-state persistence
- [ ] 3.11 Keyboard shortcuts, drag-drop anywhere, single instance + link hand-over
- [ ] 3.12 Settings screen + diagnostics ("Copy diagnostics", log level)
- [ ] 3.13 a11y pass (axe in CI), reduced motion, 24px targets, screen-reader labels — L-83
- [ ] 3.14 tauri-driver e2e smoke (Linux, Windows) + Playwright mockIPC UI tests (Playwright on the demo backend done: 32 tests, WebKit + Chromium)
- [ ] 3.15 **Gate review:** parity checklist (HTTP parts) ticked

## P4 Signed beta 0.1

- [x] 4.1 Release workflow: tag → native per-OS builds → package content check — L-73, L-74
- [x] 4.2 macOS ad-hoc signing + install script (`install.sh`) + Homebrew tap + illustrated Open Anyway guide — L-72, ADR 0009 (done: ad-hoc signing, tested install script, tap repo + cask generator, illustrated guide on the site's download page with its install-script alternative; site tests cover it)
- [ ] 4.3 Windows signing of binaries and installer via SignPath (unsigned fallback + SmartScreen guide until approved) — L-72
- [ ] 4.4 Linux AppImage/deb/rpm + checksums; Flatpak manifest (submission can wait until 1.0) (done: AppImage x64, deb/rpm x64 + arm64, SHA256SUMS; Flatpak manifest `packaging/flatpak` (app id `app.fuselane.Fuselane`, GNOME 51) built from the release .deb, filled from SHA256SUMS by `update-manifest.sh`, checked against flatpak-builder-lint's schema and xmllint on macOS; the app knows it's in a Flatpak (`flatpak.rs`: no in-app updates, single-instance name under the app id so no `--own-name`, start at login and keep awake through the Background and Inhibit portals, no browser registration, Sleep/Shut down refused) with unit and UI tests; to do: a test build and run with flatpak-builder on Linux, including the portals)
- [x] 4.5 Tauri updater: minisign key (backed up offline), our own `latest.json` feed, real semver tests — L-76, L-77, L-78
- [x] 4.6 Packaged smoke (`--self-test`) on signed artifacts per OS — L-75
- [x] 4.7 Updater e2e: install N-1, update to N, data intact (2026-10-08: real beta.1 → beta.2 on macOS; a 31% download resumed in beta.2, byte-exact)
- [x] 4.8 Landing page (`apps/site`): OS/arch detection, signed downloads, checksums (use the design skills)
- [x] 4.9 Local diagnostics bundle + privacy policy (no crash-reporting service, ADR 0009)
- [ ] 4.10 Real-hardware matrix pass → tag `v0.1.0-beta.1` (step-by-step checks: [HARDWARE-CHECKLIST.md](../05-quality/HARDWARE-CHECKLIST.md); results: [HARDWARE-RESULTS.md](../05-quality/HARDWARE-RESULTS.md))

## Beta.8 round: approved Figma designs (2026-10-09)

The owner approved the Figma designs (local file "Fuselane") and asked for all of it in one release. Each item is tested at the cheapest level, checked in the browser demo and, where it touches the OS or network, in the real app.

- [x] B8.1 Speeds add up: torrent speed from received bytes per network (not the verified delta); the big number is the sum of the lanes under it; sidebar total labelled "All downloads together"
- [x] B8.2 Remove asks and waits: a modal dialog (Cancel / Keep files / Move files to Bin) for downloads and torrents; Esc or outside click cancels; never closes on a timer
- [x] B8.3 Link box without a scrollbar: one-line field; a pasted magnet or link becomes a card (name, size, files, Change); long links cut in the middle
- [x] B8.4 Updates show size and progress: size before starting, a progress bar with MB, speed and time left, installing and failed states with a reason and Try again
- [x] B8.5 Start at a set time (per download), shown in the list as "Starts at 02:00"
- [x] B8.6 If the name is taken: Ask / Keep both / Replace (setting) and the inline choice in New download
- [x] B8.7 When a download finishes: Nothing / Open it / Unpack it (zip, tar, tar.gz; inside the download folder only, path-safe)
- [x] B8.8 Type filter in the list (Video, Music, Archives, Apps, Documents)
- [x] B8.9 Mirrors: extra links for the same file; each is checked for the same size (and validator) before it is used; lanes spread over the sources (8.3)
- [x] B8.10 Torrent files: priorities High / Normal / Low / Skip (Low waits until the rest is done) and Play while downloading (the start of the file first)
- [x] B8.11 Nearby: device discovery on the local network, device keys, the four-word check, trusted devices, "who can see me" (everyone for 10 minutes, then trusted only), send and receive ([NEARBY.md](../03-architecture/NEARBY.md))
- [x] B8.12 Nearby for phones: a page served by the app over the LAN (QR code) to send and receive in a phone browser; LocalSend v2 interop
- [x] B8.13 Screenshots for the owner, then release 0.1.0-beta.8

## Beta.9 round: download features (2026-10-10)

The owner asked for all ten ideas from the feature list. Each one is tested at the cheapest level and checked in the browser demo; the Send redesign, receiver Cancel and wide layouts from the same round are done.

- [x] B9.1 Do this one now: one download gets every network; the others wait and carry on after it
- [x] B9.2 Groups: links added together stay together (one row, one progress, pause or resume all, one notice when all are done)
- [x] B9.3 Find files on a page: paste a web page, pick its files by type, add them as a group
- [x] B9.4 Ready by: a deadline per download; earliest deadline first, runs outside the schedule when it would otherwise miss, says On track or At risk
- [x] B9.5 The phone only when it's worth it: per network Always / Only for long downloads / Never, with a minutes threshold
- [x] B9.6 What each network saved: after a download, "Without iPhone USB it would have taken 6 min"
- [x] B9.7 Checksums found by themselves: `<file>.sha256` or `SHA256SUMS` next to the link is used, and the download shows Verified
- [x] B9.8 Already downloaded: the same file (name and size) already finished and still on disk is pointed out before downloading again
- [x] B9.9 Hand off over Nearby: a paused download (its partial file and progress) goes to another Fuselane and continues there
- [x] B9.10 Battery aware: on low battery (not charging), keep going, leave out the phone, or pause

## Beta.10 round: free tools built in (2026-10-10)

The owner asked for more built-in features that cost nothing to run (no paid tiers, no hosted service) and save installing separate apps.

- [x] B10.1 Network check: per-network speed, latency, jitter, bufferbloat grade and DNS time, every network together, an outage log from the minute-by-minute reach checks, and a dated report for the internet provider
- [x] B10.2 Clipboard between your computers: copy on one Fuselane computer, paste on another (Nearby, LAN only, encrypted)
- [x] B10.3 Shared folders between your own computers over Nearby (no cloud)
- [x] B10.6 Hours per network: use a network only between set times every day (a night plan); running downloads move over when a window opens or closes
- [x] B10.5 Data used: bytes per network per day for two months, this month's total per network and a 30-day chart
- [x] B10.4 Video and audio from pages: hand pages to a yt-dlp the person installs (detected, never bundled), downloads run over every network
- [x] B10.7 Text on the phone page: a phone without the app sends text or a link to this computer's clipboard, and copies text this computer offers
- [x] B10.8 Feeds: follow an RSS or Atom feed (podcasts, release files, nightly builds); new files download by themselves, with words to include or skip; torrent items wait to be opened, or start by themselves when the feed allows it
- [x] B10.9 Watch folder: .torrent files, Metalinks and .txt lists of links put in a chosen folder start by themselves, then are renamed .added or .failed (works with media tools' "torrent blackhole")

## P5 Torrents (0.2)

- [x] 5.1 `engine-torrent` crate with librqbit; uTP/UPnP/LSD/web seeds off (librqbit 9 has no web seeds)
- [x] 5.2 SOCKS5 balancer (loopback, auth) + least-busy network choice — L-67 (L-71 waits for incoming peers, off by default)
- [x] 5.3 Torrent inputs: magnet, file, drag-drop, paste, OS "Open with" (Alternate handler only: LSHandlerRank Alternate on macOS, OpenWithProgids + Default apps on Windows, .desktop MIME on Linux). .torrent web links still go through the dialog as plain downloads.
- [x] 5.4 Torrent path safety + hostile fixtures — L-68, L-121
- [x] 5.5 File selection before and during; edge-piece cleanup — L-69
- [x] 5.6 Per-network credit after verification; peers UI
- [x] 5.7 Optional seeding with ratio/time limits; metered-network guard (off by default; tethers and cellular skipped while only seeding)
- [x] 5.8 Local swarm tests (isolated temp dirs) + a public swarm benchmark — L-70 (Debian 13.7 netinst in the real app: 16 MB/s over Ethernet + Wi-Fi, SHA-256 matched)
- [ ] 5.9 Upstream PR: librqbit connector hook + Windows binding

## P6 Fuse Send (0.3): direct sharing, no cloud ([FUSE-SEND.md](../03-architecture/FUSE-SEND.md), [ADR 0011](../adr/0011-fuse-send-p2p.md))

- [x] 6.1 `send` crate: link format `v1.<info-hash ‖ key ‖ flags>` (parse/print, version check) with shared vectors; negative tests for truncated, tampered and future links
- [x] 6.2 Encryption: seekable XChaCha20 at byte offsets + sealed header (name, size, BLAKE3); vectors; tamper and wrong-key tests
- [x] 6.3 Encrypting `StorageFactory` for librqbit: encrypt on read (sender), decrypt on write (receiver), no temp copy
- [x] 6.4 Sender: build the torrent over ciphertext and seed it through the engine (`share::prepare`, `share::seed`); a file changed after sharing is caught by the check
  - [x] 6.4b Stop sharing; optional stop after one full copy is sent; re-seed after a restart from the saved header and torrent (a moved or changed file is shown and forgotten). Per-peer "arrived" is still open
- [x] 6.5 Receiver: pasted links, lookup by info-hash, shape check, hidden side files that survive a restart, BLAKE3 check, real name without overwriting (`share::receive`, `Receiving::finish`)
  - [ ] 6.5b Open `fuselane://send/…` from the OS (**done**: macOS plist, Windows installer, Linux desktop entry; real-machine check pending); sender offline after 60 s (**done**); public UDP trackers as a DHT fallback; can't reach each other; not enough disk
- [ ] 6.6 Incoming peers per network (L-71) and UPnP per network, so senders are reachable (Fuse Send's own engine now listens on all addresses with UPnP and local discovery; per-network is open)
- [x] 6.7 Static link page on GitHub Pages (`/s#…`): hands the link to the app, install help, never sends the fragment anywhere (noindex, no-referrer; tested)
- [x] 6.8 Send UI in the desktop app: choose a file, copy link, "keep Fuselane open", sent and receivers, stop sharing, stop after one full copy; receive with progress, check, Show. QR and drag-and-drop are open
- [ ] 6.9 netlab e2e: sender and receiver each with 2 networks; summed throughput recorded; one side behind simulated NAT

## P7 Browser extension (0.4)

- [x] 7.1 WXT project: Chrome MV3 + Firefox MV3 builds; popup (status, on/off). Options page later
- [x] 7.2 Capture rules engine (size, type, MIME, domain) — unit tests (`packages/capture`). Alt bypass needs a content script and host permission: later
- [x] 7.3 Auth forwarding: cookies (incl. partitioned), referrer, UA, opt-in from settings; header allow-list in the engine; never logged or stored on disk
- [x] 7.4 Native-messaging host: the app binary (and the CLI) runs as the host; the app writes manifests and HKCU keys for installed browsers on every launch; `fuselane browsers`; uninstall cleanup on Windows; Linux CI e2e with a real Chromium
- [ ] 7.5 Localhost WebSocket fallback with pairing code + token + Origin/Host checks — T3, T4
- [x] 7.6 Message schema v1 validated on both sides (shared vectors); the browser resumes on decline or timeout
- [x] 7.7 Context menu "Download with Fuselane"
- [x] 7.8 Playwright extension e2e; `web-ext lint` (native host and popup in a real Chromium; web-ext lint in CI: 0 errors, 0 warnings)
- [ ] 7.9 Store listings on the free stores (Edge Add-ons, AMO) with a privacy policy; Chrome Web Store ($5 one-time) only with the owner's OK

## P8 Power features (0.5 → 0.9)

- [x] 8.1 Scheduler (time windows, per-network schedules, "before midnight", power actions) (daily schedule window; start at a time B8.5; hours per network B10.6 with a "Before midnight" preset (22:00 to 00:00) for daily data packs, backed by throttle detection when a pack runs out; when-done power actions)
- [x] 8.2 Throttle detection and auto-move (pure detector: busy, still delivering, under 32 KB/s or a tenth of its best, while another network to the same server is 4× faster overall and per stream; benched networks hand their blocks back and get a one-stream check after 1, 2, 4, 5 minutes; the download says so; ENGINE-DOWNLOAD.md §13)
- [x] 8.3 Mirrors / multi-source + Metalink (mirrors done in B8.9; Metalink 3 and 4 links add their files with mirrors and SHA-256, as a group; aria2.addMetalink too)
- [x] 8.4 Proxy per network / per download (HTTP, SOCKS5) (per network: HTTP CONNECT and SOCKS5 with an optional login, pinned to the network, end-to-end TLS, names resolved by the proxy, checked on save, plain errors, password never sent back; torrents stay direct; per download left out, a download uses its networks' proxies; NETWORKING.md §3a)
- [x] 8.5 Categories and auto-folders, search, sort (sort by type into folders; search and filters)
- [x] 8.6 Per-job speed limit (live, kept with the job)
- [x] 8.10 IDM parity, small things: Download later, export/import the list, catch copied links (opt-in), logins in links (HTTP Basic), redirects followed (up to 5, credentials never cross sites)
- [x] 8.7 Remote control: local web UI + aria2-compatible JSON-RPC (with auth) (aria2 JSON-RPC over HTTP and WebSocket with a secret, ADR 0013, Settings → Other apps, checked with AriaNg 1.3.15; a remote page for phones at / with a QR code)
- [x] 8.8 i18n framework + Hindi (our own small layer, the English sentence is the key; Settings → Look and feel → Language: System / English / हिन्दी; numbers and dates through Intl `hi-IN`; `scripts/i18n-check.ts` fails on a string without Hindi; core messages translated in the app from `locales/hi-backend.ts` (exact sentences and `{name}` templates), the rest stays English; docs/07-design/I18N.md)
- [ ] 8.9 Bring-your-own S3 bucket for uploads (keychain credentials)

## P9 1.0 launch

- [ ] 9.1 Weekly 24 h soak clean for 2 weeks; performance pass (done: soak tooling, `tools/soak.sh` + TESTING.md §7, 3-minute runs clean; the probe-retry bug its `resets` mode found is fixed; to do: the weekly 24 h soaks and the performance pass)
- [x] 9.2 User docs site; FAQ honest about limits (the site's guide page, ending in Limits, honestly; site tests cover it)
- [ ] 9.3 winget, Homebrew cask, Flathub (done: Homebrew cask generator; winget manifests `packaging/winget` (generator + v0.1.0-beta.9 example, checked against winget's 1.10.0 schemas); Flatpak manifest (4.4); to do after 1.0: the winget-pkgs and Flathub pull requests)
- [ ] 9.4 Launch plan (Product Hunt, Reddit, HN, YouTube/Instagram demos, Indian tech communities) (plan written: [LAUNCH.md](LAUNCH.md); carried out at 1.0)
- [ ] 9.5 Tag `v1.0.0`

## P10 Android: dropped

Not planned ([ADR 0010](../adr/0010-no-android-app.md)). Android phones remain supported as USB-tethered networks for the desktop app.
