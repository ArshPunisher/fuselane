# Plexo: deep-dive analysis

Date: 2026-10-08. Subject: [anmolkapil/plexo](https://github.com/anmolkapil/plexo), version `1.0.0-rc.14`, MIT, about 16.5k lines of source, 6.9k lines of tests, 263 commits in 25 days, one main author.

This is the first-pass analysis that started the Fuselane project. For the detailed evidence:
- [`plexo-forensics.md`](plexo-forensics.md): every feature, bug, constant and test, with file and commit citations
- [`market-research.md`](market-research.md): its issues, its users and its competitors

---

## 1. What it is

A desktop download manager that splits one download across **several internet connections at the same time** (for example Wi-Fi plus a phone tethered over USB). It handles direct links, magnet links and `.torrent` files, and has a queue plus speed and data limits per network. It needs no VPN, no relay server and no admin rights.

## 2. Tech stack

| Layer | Choice |
|---|---|
| Shell | Electron 44, electron-vite, electron-builder |
| UI | React 19, TypeScript, Tailwind v4, Base UI (shadcn-style), Zustand, Lucide |
| Download engine | Plain Node `http`/`https`/`net`, no download library |
| Torrents | WebTorrent 3.0.21, modified with `patch-package` (a connect hook; simple-peer patched so it doesn't load the native WebRTC module) |
| Native OS calls | `koffi` FFI: libc on Linux (`SO_BINDTODEVICE`), kernel32 on Windows (`FSCTL_SET_SPARSE`) |
| Tests | Playwright end-to-end tests plus fast-check property tests, chaos tests, and `@disk` tests on small tmpfs drives |
| CI | GitHub Actions: lint, typecheck and smoke tests on Ubuntu, a Windows torrent job, and a nightly full suite on Ubuntu and Windows |
| Website | Static HTML/JS on GitHub Pages (getplexo.app). The downloads list reads the GitHub Releases API. |

## 3. How it works on each platform

1. **The core trick.** Each stream's socket is opened with `localAddress` set to one network's IP. macOS and Windows mostly choose the outgoing interface from that source address.
2. **Linux is different.** Linux routes by the routing table, so the source IP alone isn't enough. Plexo creates the socket through libc (via koffi), sets `SO_BINDTODEVICE` (allowed without root since kernel 5.7), and hands the fd to Node. If that fails it falls back and warns the user.
3. **Adapter names.**
   - macOS: parses `networksetup -listallhardwareports` by *position*, so it works on any language.
   - Windows: PowerShell `Get-NetAdapter` plus `NdisPhysicalMedium`.
   - Linux: regexes over systemd device names.
4. **Windows disk fix.** The staging file is marked sparse. Without that, NTFS zero-fills the gap before a write far into the file, so most of the file is written twice.
5. **Torrents stay on the chosen network.** uTP, WebRTC, UPnP, NAT-PMP, LSD and web seeds are turned off, because their sockets can't be pinned to one network.
6. **Polite OS integration.** It is offered as an *Alternate* handler for magnet and `.torrent` links and never takes over an existing torrent client. It also uses a single-instance lock, `open-url`/`open-file`, `powerSaveBlocker` and native title-bar overlays.
7. **Installers.** DMG for x64 and arm64, one NSIS installer for x64+ARM64, and AppImage, deb and rpm packages.

## 4. Engineering ideas worth learning from

- **Shared block queue.** Blocks are 1–8 MiB. A stream takes the next free block, so faster networks naturally fetch more blocks.
- **Racing slow blocks at the end.** Once the queue is empty, a free stream races a slow block and the first to finish wins.
- **No silent corruption.** It checks validators again before resuming and refuses if the file changed, settling ambiguous cases by sampling bytes already on disk.
- **A testable controller.** The stream-count rule is a pure function of a snapshot (time passed in), so it can be property-tested.
- **Honest docs.** For example: "Wi-Fi plus Ethernet to the same router gives no speed-up."

## 5. What should have been done better

1. **Code signing (the biggest problem).** Users have to run `xattr -dr com.apple.quarantine` on macOS and click through SmartScreen on Windows. Issues #15, #27 and #35 show this blocks real users.
2. **No real auto-update.** `publish.url` is a placeholder (`https://example.com/auto-updates`). The update check uses a naive version comparison: **1.0.0 will compare as older than rc.14**, so pre-release users will never be told about it.
3. **macOS is never tested in CI**, even though it's a primary target. Installers are never built or smoke-tested in CI, and rc.12 shipped a macOS native binary inside the Windows installer.
4. **Electron is heavy for this kind of tool.** 100 MB+ installers and high RAM use for a utility that mostly runs in the background.
5. **No browser integration.** That's IDM's main feature. A community PR (#18) is still unmerged.
6. **God files.** `downloadManager.ts` is 1,722 lines and `httpTransfer.ts` 1,186.
7. **Fast tests run only nightly.** About 90 pure-logic tests aren't tagged `@smoke`, so they never run on PRs.
8. **Windows interface pinning isn't reliable.** `localAddress` alone doesn't force the interface (issue #65). The fix is `IP_UNICAST_IF`.
9. **Smaller gaps.**
   - HTTP/1.1 only.
   - No mirrors or checksums, and no proxy, cookies or auth.
   - Network changes found by polling every second, with PowerShell on Windows.
   - No preallocation.
   - The JSON state files are never fsynced, and a corrupt file silently resets the user's data.
   - IPC is not validated at runtime.
   - Electron fuses are off.
   - **No uploads at all.**

## 6. Ideas this inspired

The real lesson is the pattern: *the OS leaves something you already own sitting idle. Combine it without needing a server or admin rights, and be very careful about correctness.*

1. **Bonded uploads, a WeTransfer that uses every connection ⭐ (now part of Fuselane).** S3, R2 and B2 multipart uploads accept parts in parallel and in any order, so they can be striped across networks, with resume per part and a share link at the end.
2. **Mobile version (Wi-Fi + cellular).** Android's `requestNetwork` gives a socket per network, and iOS Network.framework has `requiredInterfaceType`. Daily data caps already map onto per-network limits. (Fuselane, later phase.)
3. **Connection doctor.** Per-network latency, loss and DNS probes, with outage reports to send your ISP. (Partly in Fuselane as the per-network speed test and health view.)
4. **Model-weight and dataset downloader.** Combined networks, SHA-256 verification and a deduplicated cache for 10–200 GB AI weights. (Could be built on the Fuselane CLI.)
5. **Shared download cache for a LAN.** Download once and serve it to every machine on the network. (Future idea.)
6. **Browser extension plus a native helper.** Capture downloads like IDM. (Now part of Fuselane.)
7. **Home render farm.** Split ffmpeg exports by time range across the machines on a LAN. (Separate future idea.)

## 7. Conclusion for Fuselane

Build everything Plexo has (see [`../02-product/PARITY-CHECKLIST.md`](../02-product/PARITY-CHECKLIST.md)), avoid every mistake it made (see [`../02-product/LESSONS-FROM-PLEXO.md`](../02-product/LESSONS-FROM-PLEXO.md)), and add the pillars it lacks: **bonded uploads, browser capture, signed releases with auto-update, a native Rust core, and a headless CLI and daemon**.
