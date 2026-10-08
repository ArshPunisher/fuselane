# Status

> Update this file at the end of every working session (Claude does this as part of the workflow in CLAUDE.md). Newest entry on top of the log.

## Now

- **Phase:** **P3 desktop app** in progress on top of a working P2 core + CLI. P1 hardware spikes still wait on a phone to tether.
- **P3 done (tested, committed):**
  - Engine snapshots for the UI (per-network bytes, rates, streams; 180 ring ticks with fill, owner, in-flight).
  - `core::runner`: one job runner shared by the CLI and the desktop app (link checks, network choice, Happy Eyeballs connects, store bookkeeping, plain-language errors). `core::home`: one shared download list.
  - `apps/desktop/src-tauri`: Tauri 2 shell + `Service` (3-slot queue, pause/resume/remove, crash recovery, live events). 12 service tests against the hostile server, including break tests (duplicate links, 0-byte files, range-ignoring servers, a 30-job flood).
  - `apps/desktop`: React UI with the live Fuse Core, Stream, list, New download dialog, Networks and Settings; compact/regular/wide layouts; light/dark/system. In a plain browser it runs a labelled demo engine (`src/lib/demo.ts`, URL params documented at the top).
  - **32 Playwright UI tests** (WebKit + Chromium) are part of `tools/check.sh` and CI.
  - Real-world bugs found and fixed this session: IPv6 source binding hung downloads (L-109); 429 on the probe ended downloads (L-110); CLI and app used different databases (L-111); "0 B" for fast completed downloads (L-112); dialog focus (L-113).
  - Verified in the real window: a 300 MB download over **Ethernet (en0) + Wi-Fi (en1)** at ~50 MB/s, byte-exact; finished screen shows Ethernet 73%, Wi-Fi 27%.
  - Workspace: **156 Rust tests + 44 UI tests, all green**.
- **Run it:**
  - UI in a browser (demo data): `pnpm --filter @fuselane/desktop dev`, then open http://localhost:5190.
  - Real app: `pnpm --filter @fuselane/desktop build`, then `cargo run -p fuselane-desktop --features tauri/custom-protocol` (or `pnpm --filter @fuselane/desktop tauri dev`).
- **P4 beta:** **`v0.1.0-beta.3` published 2026-10-08** (data allowances, new logo and icon, updated message). Site has SEO, share card, favicons, sitemap; IndexNow pinged on every release; Google Search Console waits for the owner's verification tag. Earlier: **`v0.1.0-beta.2` published 2026-10-08** (real update from beta.1 verified with the owner's one click). Download page live at https://arshpunisher.github.io/fuselane/. Earlier: **`v0.1.0-beta.1` published 2026-10-08** (pre-release, 16 files). Verified after publishing: macOS app checksum + self-test, live update feed (4 platforms), the Homebrew cask, and the live install script. Release workflow builds macOS universal (ad-hoc signed), Windows x64, Linux x64/arm64 natively, checks contents, self-tests the packaged app, and drafts a pre-release with SHA256SUMS. Publishing a release deploys the signed update feed to https://arshpunisher.github.io/fuselane/updates/latest.json. Updater key: `~/.tauri/fuselane.key` (password in Keychain "Fuselane updater key password"; both are GitHub secrets). Homebrew tap: `ArshPunisher/homebrew-fuselane` (cask from `packaging/homebrew/update-cask.sh <tag>`).
- **Next steps, in order:**
  3. 4.3 SignPath for Windows: **applied 2026-10-08** (owner, signpath.org form). Waiting for their email; then add the organization ID, project slug and API token as secrets and wire the signing step into release.yml.
  5. **P5 torrents in progress:** engine (SOCKS5 proxy per network, path safety, file selection, edge-piece cleanup, credit for verified bytes, resume after restart) and the desktop app (magnet, Open .torrent, paste, drop, Choose files, torrent detail with per-network peers and credit). Verified in the real window: Debian 13.7 netinst over Ethernet + Wi-Fi at 16 MB/s, SHA-256 matched, credit 62%/38%. **Torrents also follow** speed limits, slow mode and data allowances (uploads count too), and the sidebar shows torrent speed per network. Sharing after download is opt-in with ratio and time limits. **Known gaps:** DHT and UDP tracker packets go out on the OS's default route and aren't counted toward allowances (small, ADR 0006). Opening magnet links and .torrent files from the OS works (verified on macOS with open -a; Windows and Linux packaging untested on real machines yet). CI fixed 2026-10-08: the proxy forwarded the BitTorrent handshake in two writes, which broke every peer connection on Linux (L-123); Windows runners read their Hyper-V card as virtual. Next: 5.9 upstream librqbit PR, then P3 leftovers.
- **Repo:** public at https://github.com/ArshPunisher/fuselane (pushed 2026-10-08). First CI run **green on macOS, Windows and Linux**. `main` blocks force-push and deletion. Push only `main` and only when the owner says "push"; spikes stay local.
- **Waiting on the owner, and what's next:** see [PENDING.md](PENDING.md).
- **Environment:** Rust via rustup (`source ~/.cargo/env`), cargo-nextest, cargo-deny, actionlint (Homebrew), Node 24, pnpm 11, Playwright WebKit + Chromium, gh logged in as ArshPunisher. macOS has no `timeout` command. Screen recording is granted (capture a window with `screencapture -l<id>`; find the id with a CGWindowList script); clicking is not (no Accessibility), so drive the real window with `FUSELANE_DEV_ADD=<url>` in debug builds. macOS notifications only work from a bundle: `pnpm --filter @fuselane/desktop tauri build --debug --bundles app`, then run `target/debug/bundle/macos/Fuselane.app/Contents/MacOS/fuselane-desktop`.

## Log

### 2026-10-09
- Extension settings page (size, sites, file types, MIME types) with inline validation; Chrome Web Store draft filled, waiting on the publisher email check.
- Fixed: `Store::open` moved a busy (not damaged) database aside, orphaning every job; this was the Linux kill -9 test flake.
- Owner decision: no plans and no hosted services. Cloud uploads dropped and removed; P6 is now Fuse Send, direct peer-to-peer sharing (ADR 0011).

### 2026-10-08 (session 3, evening)
- P5 torrents end to end: engine (SOCKS5 relay per network, path safety, file choice, credit, resume, sharing) and the desktop app (magnet, .torrent, paste, drop, OS Open with as an alternate handler, limits and allowances, sign-in page checks). Real Debian 13.7 download over Ethernet + Wi-Fi verified by SHA-256.
- CI had been red on Linux and Windows for about a dozen pushes (L-124). Causes fixed: the relay split the BitTorrent handshake (L-123), Windows runners' Hyper-V card read as virtual, a list-only add opening peer connections, librqbit's retry backoff after an allowance block, and two backends starting under React StrictMode. All three platforms green again; the release dry run builds all four targets.
- Also: one owner per download (app vs CLI), `--json` for `ls` and `nets`, axe-core accessibility checks in light and dark, torrents following network changes.

### 2026-10-08 (session 3, later)
- Owner granted screen recording and plugged in Ethernet. Native screenshots found L-114 (black ring) and L-115 (Retina pane overflow); both fixed with tests. Real bonded Ethernet + Wi-Fi download verified in the app.

### 2026-10-08 (session 3)
- P3 started: snapshots, shared runner, Tauri service, React UI with the Fuse Core, 32 UI tests in the gate.
- Real downloads exposed L-109 (IPv6 source binding hang, so Happy Eyeballs 2.14 was built), L-110 (429 on the probe) and L-111 (two databases).

### 2026-10-08 (session 2, latest)
- Controller wiring, core store, resume and checksums.
- Bugs found by our own tests: checkpoints lost in-flight bytes (L-107); a validator loop that hung on resume (L-108); a test-server throttle that didn't throttle; a piped check that let a broken build be committed, so `tools/check.sh` now gates every commit.

### 2026-10-08 (session 2, later)
- Built netif, transport (pinning + TLS) and the CLI.
- A real download from proof.ovh.net exposed an If-Range bug: we sent a normalized ETag, so servers returned 200. Fixed, the test server now enforces RFC 9110, and the lesson is L-106.

### 2026-10-08 (session 2, continued)
- Spikes:
  - S2 macOS pinning part 1 (found that iCloud Private Relay relays plain HTTP);
  - S7 Tauri shell on macOS (5.76 MiB app, about 75–80 MB of RAM);
  - S9 Lane Weave, then the owner rejected it as too close to Plexo → **S9b Fuse Core** (original radial design, approved by the owner on seeing the real window).
- P2 engine built test-first with property tests, attack tests and chaos. Our tests found and fixed three real bugs (L-103 hidden or NBSP names, L-104 transient 403 treated as an expired link, L-105 range overflow).

### 2026-10-08 (session 2)
- Owner decisions: open source (Apache-2.0), personal GitHub, SignPath, **no paid services** → ADR 0009; domain later.
- Added LICENSE/NOTICE, the design system + motion language (`docs/07-design/`), spike S9, and the SessionStart hook (`tools/session-context.sh`) so new sessions open with this status.

### 2026-10-08
- Studied Plexo (rc.14) in depth: stack, 263 commits, 93 issues/PRs, tests, weaknesses.
- Researched competitors, user pain points and current tech (Tauri 2, socket2, librqbit, R2, WXT, signing).
- Named the product **Fuselane** (after rejecting Sangam, Pluro and others).
- Created the repo and wrote the full planning set: vision, parity checklist, features, 100 lessons, architecture + 9 subsystem docs, 8 ADRs, testing strategy, roadmap, steps, workflow, CLAUDE.md.
