# Status

> Update this file at the end of every working session (Claude does this as part of the workflow in CLAUDE.md). Newest entry on top of the log.

## Now

- **Phase:** **P4 beta**: **`v0.1.0-beta.10` published 2026-10-10** (verified: checksums, universal binary, self-test incl. Nearby, update feed for 4 platforms, Homebrew cask; site redeployed to fuselane.app; Search Console verified with the sitemap submitted; extension 0.2.0 submitted to the Chrome Web Store). Remaining before 1.0: the real-hardware checklist (needs a phone and other computers), two weeks of 24 h soaks, Windows signing, store listings.
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
- **P4 beta:** **`v0.1.0-beta.9` published 2026-10-10** (B9.1–B9.10 download features, Send radar, receiver cancel, crash reports and Report a problem; verified: checksums, universal binary, self-test incl. Nearby, live feed for 4 platforms, probe unchanged, Homebrew cask). Site live at https://fuselane.app (Cloudflare Pages; IndexNow accepted). Earlier: **`v0.1.0-beta.8` published 2026-10-09** (the four reported fixes, mirrors, start at a time, name taken, open/unpack when done, type filter, torrent priorities and Play, Nearby with LocalSend and a phone page; approved by the owner from screenshots; verified: checksums, universal binary, DMG layout, self-test incl. Nearby, live feed for 4 platforms, the live package verified after a bonded fetch, Homebrew cask). Earlier: **`v0.1.0-beta.7` published 2026-10-09** (torrent Peers tab with which network carries each peer, pieces strip, tabs, motion foundations; approved by the owner from screenshots first). Earlier: **`v0.1.0-beta.6` published 2026-10-09** (a designed macOS disk image: background, large icons, first-open steps; the release workflow sets `TAURI_BUNDLER_DMG_IGNORE_CI`, without which Tauri drops the Finder layout on CI, and checks the image has it). Earlier: **`v0.1.0-beta.5` published 2026-10-09** (Fuse Send, redirects fixed, per-download limits, Download later, automatic retries, export/import, copied links, Move to Trash; verified: checksums, universal binary, self-test, `fuselane://` in the macOS plist and the Linux desktop entry, live feed for 4 platforms, site and `/s` page live, Homebrew cask). Earlier: **`v0.1.0-beta.4` published 2026-10-09** (torrents, browser extension host, sign-in pages; verified: checksums, universal binary, self-test, host mode, live feed for 4 platforms, Homebrew cask). **Release checklist includes the Homebrew cask** (`packaging/homebrew/update-cask.sh <tag>` in `ArshPunisher/homebrew-tap`): beta.2 and beta.3 missed it, so the tap jumped from beta.1 to beta.4. Earlier: **`v0.1.0-beta.3` published 2026-10-08** (data allowances, new logo and icon, updated message). Site has SEO, share card, favicons, sitemap; IndexNow pinged on every release; Google Search Console waits for the owner's verification tag. Earlier: **`v0.1.0-beta.2` published 2026-10-08** (real update from beta.1 verified with the owner's one click). Download page live at https://arshpunisher.github.io/fuselane/. Earlier: **`v0.1.0-beta.1` published 2026-10-08** (pre-release, 16 files). Verified after publishing: macOS app checksum + self-test, live update feed (4 platforms), the Homebrew cask, and the live install script. Release workflow builds macOS universal (ad-hoc signed), Windows x64, Linux x64/arm64 natively, checks contents, self-tests the packaged app, and drafts a pre-release with SHA256SUMS. Publishing a release deploys the signed update feed to https://arshpunisher.github.io/fuselane/updates/latest.json. Updater key: `~/.tauri/fuselane.key` (password in Keychain "Fuselane updater key password"; both are GitHub secrets). Homebrew tap: `ArshPunisher/homebrew-tap` (`brew install --cask arshpunisher/tap/fuselane`) (cask from `packaging/homebrew/update-cask.sh <tag>`).
- **Next steps, in order:**
  3. 4.3 SignPath for Windows: **applied 2026-10-08** (owner, signpath.org form). Waiting for their email; then add the organization ID, project slug and API token as secrets and wire the signing step into release.yml.
  5. **P5 torrents in progress:** engine (SOCKS5 proxy per network, path safety, file selection, edge-piece cleanup, credit for verified bytes, resume after restart) and the desktop app (magnet, Open .torrent, paste, drop, Choose files, torrent detail with per-network peers and credit). Verified in the real window: Debian 13.7 netinst over Ethernet + Wi-Fi at 16 MB/s, SHA-256 matched, credit 62%/38%. **Torrents also follow** speed limits, slow mode and data allowances (uploads count too), and the sidebar shows torrent speed per network. Sharing after download is opt-in with ratio and time limits. **Known gaps:** DHT and UDP tracker packets go out on the OS's default route and aren't counted toward allowances (small, ADR 0006). Opening magnet links and .torrent files from the OS works (verified on macOS with open -a; Windows and Linux packaging untested on real machines yet). CI fixed 2026-10-08: the proxy forwarded the BitTorrent handshake in two writes, which broke every peer connection on Linux (L-123); Windows runners read their Hyper-V card as virtual. Next: 5.9 upstream librqbit PR, then P3 leftovers.
- **Repo:** public at https://github.com/ArshPunisher/fuselane (pushed 2026-10-08). First CI run **green on macOS, Windows and Linux**. `main` blocks force-push and deletion. Push only `main` and only when the owner says "push"; spikes stay local.
- **Waiting on the owner, and what's next:** see [PENDING.md](PENDING.md).
- **Environment:** Rust via rustup (`source ~/.cargo/env`), cargo-nextest, cargo-deny, actionlint (Homebrew), Node 24, pnpm 11, Playwright WebKit + Chromium, gh logged in as ArshPunisher. macOS has no `timeout` command. Screen recording is granted (capture a window with `screencapture -l<id>`; find the id with a CGWindowList script); clicking is not (no Accessibility), so drive the real window with `FUSELANE_DEV_ADD=<url>` in debug builds. macOS notifications only work from a bundle: `pnpm --filter @fuselane/desktop tauri build --debug --bundles app`, then run `target/debug/bundle/macos/Fuselane.app/Contents/MacOS/fuselane-desktop`.

## Log

### 2026-10-10, evening (owner away; beta.10 finished, waiting for the owner's tag)

- **Built with four agents in git worktrees, merged into main one by one, full gate after each merge, all pushed:**
  - Throttle detection (8.2): a network that collapses while others keep going is benched, its parts handed over, retried every few minutes; the download says so.
  - Proxy per network (8.4): HTTP CONNECT and SOCKS5 with login, pinned to the network, checked on save, plain errors; system proxies ignored (L-66). Probe now retries dropped connections (found by the soak).
  - aria2 push notifications on the WebSocket; feeds can start torrents by themselves (opt-in); Flatpak manifest (`app.fuselane.Fuselane`) and winget manifests with generators; `tools/soak.sh` (3-minute run clean).
  - Hindi (8.8): a small translation layer, 808 strings, a check that fails on a missing one; Settings → Language.
  - Website redesign: the live Fuse Core hero with network switches, a scroll story, a race from measured speeds, real app screens, an illustrated Open Anyway guide (4.2), a user guide with honest limits (9.2), a strict CSP.
- **Built here:** a welcome on the first launch with platform tips (3.8); "Before midnight" hours preset (8.1); popup e2e and web-ext lint in CI (7.8; 0 errors, 0 warnings); Firefox `data_collection_permissions`; magnet links offered on Linux; the real-hardware checklist and results; the launch plan (9.4); shellcheck in the local gate.
- **Checked for real:** the packaged-style app's self-test (download over 2 networks and Nearby, byte-exact); the real window shows the welcome with this Mac's Ethernet and Wi‑Fi; AriaNg 1.3.15 over HTTP and WebSocket.
- **Version:** `chore(release): 0.1.0-beta.10` is on main. Tagging, the release workflow (even a dry run) and site deploys are refused for the agent; steps for the owner are in PENDING.md.

### 2026-10-10, night (owner away; more zero-cost features for beta.10, not released)

- **Built (tested, committed, pushed):**
  - Networks page in three tabs: Setup, Check, Usage.
  - B10.7 Phone page text both ways: a phone without the app sends text to the clipboard and copies text offered to it.
  - 8.7 Remote control for aria2 apps (ADR 0013, `fuselane-rpc` crate, Settings → Other apps, off by default): aria2 JSON-RPC over HTTP and WebSocket with a secret, Host check against DNS rebinding, open sessions closed on a new secret. Checked with the real AriaNg 1.3.15 (HTTP and its default WebSocket): it connects, lists, shows speed and adds links.
  - B10.8 Feeds (Downloads → Feeds): RSS and Atom, words to include or skip, every 15 min to daily, names from titles (podcast hosts call every episode default.mp3), torrent items wait for Open. Checked on real feeds: a 20 MB podcast feed in 10 s, SourceForge file feeds, GitHub's release feed (no files, correctly).
  - 8.3 Metalink: `.meta4`/`.metalink` links add their files with mirrors and SHA-256, as a group; `aria2.addMetalink` too. Checked on a live openSUSE Metalink.
  - Fixed: Settings scrolled sideways at 375 px (the Diagnostics buttons couldn't wrap). Flaky phone-page test fixed.
  - Site source lists the new built-ins (not deployed: goes live with beta.10).
- **Housekeeping:** `target/debug` had grown to 30 GB and filled the disk mid-build; removed (it's rebuildable).
- **Next:** owner reviews beta.10 (demo at localhost:5191) and says release; then publish beta.10, deploy the site, upload extension 0.2.0. Owner: Google Search Console Domain property for fuselane.app.

### 2026-10-10, later (beta.9 published; beta.10 round built, not released)

- **Published:** `v0.1.0-beta.9` (verified: checksums, universal binary, self-test incl. Nearby, feed for 4 platforms, probe unchanged; Homebrew cask, homepage fuselane.app). fuselane.app live; IndexNow accepted. Chrome Web Store listing live (0.1.0); new images, captions and 0.2.0 zip ready in `apps/extension`.
- **Built for beta.10 (tested, committed, pushed; not released):**
  - B10.1 Network check per network (speed via Cloudflare's free endpoint, latency, jitter, bufferbloat grade, DNS), all together, outage log, provider report. Found and fixed: some networks drop the second of two DNS queries, so lookups waited 3 s.
  - B10.2 Text and clipboard between computers (LocalSend-compatible messages; trusted computers copy straight to the clipboard).
  - B10.3 Folders: send a folder as a folder; keep a folder in sync with a trusted computer (one way, nothing deleted). Received paths are cleaned and can't escape the save folder.
  - B10.4 Video from pages with the person's own yt-dlp, downloaded over every network (246 MB 1080p over Wi-Fi + Ethernet in 18.6 s), ffmpeg join, Audio (MP3); the extension's popup hands the page over.
  - B10.5 Data used per network per day, with a 30-day chart.
  - Site: "Built in" section and two FAQ entries (deploy with beta.10).
- **Next:** owner reviews beta.10 in the demo (localhost:5191) and says release; then publish beta.10, deploy the site (`tools/deploy-site.sh`), upload extension 0.2.0 with the new images.
- **Waiting on the owner:** Google Search Console (Domain property, Cloudflare verification, submit the sitemap); the Chrome Web Store upload.

### 2026-10-10 (beta.9 round: Send redesign, site on fuselane.app, ten download features)

- **Done (tested, committed; not released):**
  - Send page as a radar (B and C hybrid) with motion: sweep, devices spring in, drop files on a device, transfer beams, progress rings, activity column; receiver Cancel; check words removed. Wide layouts for Networks and Settings; motion on Downloads.
  - Site: served at fuselane.app through Cloudflare Pages (`tools/deploy-site.sh`); canonical, FAQPage, breadcrumbs, WebSite data, dated sitemap, 404, headers. GitHub Pages stays for installed apps: its sign-in check and update feed must not move (a custom domain there would redirect the probe and every installed app would see a sign-in page).
  - B9.1–B9.10: do this one now, groups, find files on a page, ready by, the phone only for long downloads, what each network saved, checksums found by themselves, already downloaded, continue on another computer, low battery. Store schema v10–v13.
  - Fixed on the way: receiver cancel could reach the sender as a network error; refused phone-page uploads reset the connection; WebKit sent clicks during a View Transition to the page root (screens now animate with plain CSS).
- **Next:** owner presses Activate for fuselane.app in Cloudflare (Workers & Pages → fuselane → Custom domains); owner reviews the features; then release beta.9 when asked.
- **Blocked:** nothing.

### 2026-10-09 (beta.8 round, from the approved Figma designs)
- Owner approved the Figma designs (local file "Fuselane", built through the local Talk-to-Figma bridge) and asked for all of it in one release. B8.1-B8.12 done, each tested; B8.13 (screenshots, then release) waits on the owner.
- The four reported fixes: speeds add up (torrent speed = sum of its networks; sidebar total labelled), Remove asks in a dialog that waits (torrent files to the Trash), a one-line link box with magnets as a card, updates with size and a progress bar over every network (minisign-verified before install; the live beta.7 package verified after a bonded fetch).
- Downloads: start at a set time, if the name is taken (Ask/Keep both/Replace), open or unpack when done (zip/tar, path-safe, zip-bomb cap), type filter, mirrors (checked for size, ranges and the same bytes; kernel.org CDN + mirror verified: 135 MB, 28 MB from the mirror).
- Torrents: file priorities (tiers through the file selection) and Play while downloading (local stream server: 127.0.0.1, token, loopback Host check). A priority set during the check is applied after it (bug found by the test).
- Nearby (ADR 0012, NEARBY.md): new `fuselane-nearby` crate speaking LocalSend v2; pinned TLS, verified sender fingerprints, four check words, trust, "who can see this computer"; a phone page with a QR code (checked in Chromium and WebKit, both directions byte-exact). The packaged self-test now sends a file with Nearby.

### 2026-10-09 (overnight, owner away)
- **Fuse Send in the app** (6.4b, 6.7, 6.8 and most of 6.5b): a Sends service with its own reachable engine (DHT, UPnP, local discovery), the Send page, the `/s` link page on the site, `fuselane://send/` registered on all three OSes, shares restored after a restart, optional stop after one full copy. End-to-end test: one Fuselane sends 3 MB to another over loopback, byte-exact.
- **Real bug fixed:** HTTP redirects weren't followed at all, so links like GitHub release assets failed with "status 302". Now followed (5 hops, http/https only, cookies and logins dropped on another site). Verified against GitHub: the beta.4 DMG through its 302, split over en0 + en1, SHA-256 matched.
- IDM-style features: a speed limit per download (live), Download later, export/import the list, catch copied download links (opt-in, off by default), logins in links as HTTP Basic, passwords masked in the window.
- Site: Fuse Send band, "small things" grid, Send in the nav, three FAQ answers, and the `/s` page. **Not deployed yet**: it advertises Fuse Send, which ships in the next release.
- Fixed a flaky site test (hero speeds read across frames). BSL-1.0 allowed in cargo-deny (Windows clipboard crates).
- **Fuse Send on a real network:** DHT found the sender but NAT (no UPnP on this router) blocked the connection, and librqbit's local discovery never asks, so a same-LAN receiver could wait 5 minutes. Fixed both ways: the receiver sends BEP 14 searches while looking (found in under a second), and the sender listens on IPv6 too (DHT-only, over IPv6: 4.3 s). Honest copy on the site and in the error message about UPnP/IPv6.
- More: failed downloads retry by themselves (20 s → 60 min, at once when a network returns), Move to Trash for finished files, free-space check before receiving, extension popup lists a page's videos and file links (activeTab + scripting).

### 2026-10-09
- Native-messaging host: the app registers itself with installed browsers on every launch; `fuselane browsers`; e2e with a real Chromium. Chrome Web Store item submitted for review.
- Published `v0.1.0-beta.4`. Fuse Send started: link format (6.1) and encryption (6.2).
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
