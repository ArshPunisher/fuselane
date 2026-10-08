# Status

> Update this file at the end of every working session (Claude does this as part of the workflow in CLAUDE.md). Newest entry on top of the log.

## Now

- **Phase:** P1 spikes (hardware-blocked parts pending) running alongside **P2 Core + CLI** (started).
- **P2 done (tested, committed):**
  - storage: names (2.16), staging + publish (2.17, 2.19);
  - limits (2.21, 2.22);
  - engine-http: headers (2.23, 2.24), plan (2.25), scheduler (2.26), concurrency (2.27), retry (2.29), measure (2.31), and the multi-network downloader (2.28);
  - testkit hostile server (2.1).
  - Tests: 62 in engine-http, including a 15-test end-to-end attack suite plus a chaos test (100/100 seeds byte-exact).
- **Also done:**
  - netif (2.5);
  - transport: pinning on macOS, Linux and Windows + rustls TLS (2.11);
  - CLI `fuselane get` / `fuselane nets` (part of 2.44).
  - **A real HTTPS download over a pinned socket is byte-identical to curl.**
  - Workspace: **109 tests pass**.
- **Also done since:**
  - controller wired (Auto grows to 32, settles at server caps);
  - **pause/checkpoint/resume** with byte sampling and a lying-checkpoint defence;
  - SHA-256 verification;
  - `core` SQLite store + state machine;
  - CLI `ls`/`resume`/`rm`/Ctrl-C;
  - **kill -9 mid-download, then resume, is byte-exact**.
  - Workspace about 135 tests, all green.
- **Next steps, in order:**
  1. macOS friendly names/kinds via SystemConfiguration (2.6), plus Windows/Linux equivalents.
  2. Per-network DNS (2.13) and Happy Eyeballs (2.14).
  3. Edge-case coverage table.
  4. **P3 desktop app** (Tauri + Fuse Core, real engine).
- **Waiting on the owner:**
  - Approve the S9b Fuse Core contact sheets (`docs/04-plan/spikes/S9b-fuse-core.md`).
  - Q13: OK to create the **public** GitHub repo? (CI can't run until then.)
  - Q10: test hardware (a phone to tether) for S1/S2-part-2/S3.
- **Parked:** S5 torrent spike (librqbit + SOCKS5). Only scaffolding existed; restart it in P5 or when time allows.
- **Environment:** Rust via rustup (`source ~/.cargo/env`), cargo-nextest, cargo-deny, actionlint (Homebrew), Node 24, pnpm 11, Playwright WebKit + Chromium, gh logged in as ArshPunisher.

## Log

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
  - S9 Lane Weave, then the owner rejected it as too close to Plexo → **S9b Fuse Core** (original radial design, contact sheets pending approval).
- P2 engine built test-first with property tests, attack tests and chaos. Our tests found and fixed three real bugs (L-103 hidden or NBSP names, L-104 transient 403 treated as an expired link, L-105 range overflow).

### 2026-10-08 (session 2)
- Owner decisions: open source (Apache-2.0), personal GitHub, SignPath, **no paid services** → ADR 0009; domain later.
- Added LICENSE/NOTICE, the design system + motion language (`docs/07-design/`), spike S9, and the SessionStart hook (`tools/session-context.sh`) so new sessions open with this status.

### 2026-10-08
- Studied Plexo (rc.14) in depth: stack, 263 commits, 93 issues/PRs, tests, weaknesses.
- Researched competitors, user pain points and current tech (Tauri 2, socket2, librqbit, R2, WXT, signing).
- Named the product **Fuselane** (after rejecting Sangam, Pluro and others).
- Created the repo and wrote the full planning set: vision, parity checklist, features, 100 lessons, architecture + 9 subsystem docs, 8 ADRs, testing strategy, roadmap, steps, workflow, CLAUDE.md.
