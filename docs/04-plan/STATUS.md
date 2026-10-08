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
- **Next steps, in order:**
  1. Get CI green on all three OSes (first run on Windows and Linux), then protect `main` with required checks.
  2. 3.4 dialog: native folder picker (tauri-plugin-dialog), drop a link, probe preview (name and size before starting).
  3. 3.6 complete/error screens with catalogue actions; 3.7 Reveal in Finder/Explorer; 3.10 tray progress, notifications.
  4. tauri-specta (or a schema test) so `types.ts` can't drift from `service.rs`.
  5. P2 leftovers: per-network DNS (2.13), Windows/Linux friendly names (2.7, 2.8), free-space check (2.20), sleep/wake (2.39), `--json`.
- **Repo:** public at https://github.com/ArshPunisher/fuselane (pushed 2026-10-08). Push only `main`; spikes stay local.
- **Waiting on the owner:**
  - Enable Renovate on the repo (install the free Renovate GitHub app); SignPath application once there is a release.
  - Q10: a phone to tether, for S1/S2-part-2/S3.
- **Parked:** S5 torrent spike (librqbit + SOCKS5).
- **Environment:** Rust via rustup (`source ~/.cargo/env`), cargo-nextest, cargo-deny, actionlint (Homebrew), Node 24, pnpm 11, Playwright WebKit + Chromium, gh logged in as ArshPunisher. macOS has no `timeout` command. Screen recording is granted (capture a window with `screencapture -l<id>`; find the id with a CGWindowList script); clicking is not (no Accessibility), so drive the real window with `FUSELANE_DEV_ADD=<url>` in debug builds.

## Log

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
