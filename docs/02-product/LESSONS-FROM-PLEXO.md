# Lessons from Plexo: rules we follow so we don't repeat its bugs

Plexo found these the hard way across 263 commits and about 60 fixes. Each rule has an ID (`L-xx`), the evidence (Plexo commit hash or issue number, from [`../01-research/plexo-forensics.md`](../01-research/plexo-forensics.md) and [`../01-research/market-research.md`](../01-research/market-research.md)), and the **guard**: the test or check that proves Fuselane follows it.

**How to use this file:** before working on an area, read its section. When a PR touches that area, its description lists the `L-xx` rules it relies on. A rule without a guard counts as unfinished work.

The full list of 120 engine edge cases, with citations, is in plexo-forensics **Part 3–5, §B.3**. Each one becomes a test case; the list is tracked in [`../05-quality/TESTING.md`](../05-quality/TESTING.md) §6.

---

## 1. Trust the server's behaviour, not its headers

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-01 | Decide range support by a `bytes=0-0` probe returning 206. Ignore `Accept-Ranges`. | 4e8f7af | testkit server with `Accept-Ranges` lying in both directions |
| L-02 | Accept a 206 only if Content-Range starts at exactly the requested offset, doesn't pass the end, and the body length matches. Cut an overlong body at the boundary and fail. | 9918671 (a 40 MB file came out as 5 MB "complete") | Fault server: capped ranges, wrong start, overrun, short body |
| L-03 | A 200 to a ranged request with start > 0 means Range was ignored: reject it and fall back to a single stream. | §B.3 #37 | Fault server ignores Range |
| L-04 | Trust Content-Length only on a 200, never on a 206. A `416 bytes */0` means a valid empty file. | 68b9939 | 0-byte file test |
| L-05 | Check the file version on **every** response (size, ETag, Last-Modified). A size change is proof the file changed: fail and discard. Any other validator change is settled by re-fetching up to 8 samples of bytes already on disk. | 029387e, 5874f1b, 194433a | ETag rotates behind a load balancer (same bytes) vs a real file change |
| L-06 | Normalize ETags: strip `W/`, quotes and `-gzip`/`-br` suffixes. | 194433a | Unit test |
| L-07 | Send `If-Range` and `Accept-Encoding: identity` on segments (Plexo didn't; compression breaks byte ranges). | plexo-forensics §C.12 | Server that gzips unless asked for identity |
| L-08 | Parse Content-Disposition per RFC 6266/5987: `filename*` wins, ISO-8859-1 is decoded byte by byte, and quoted strings keep `;`. Malformed `%` escapes fall back to the raw text. | 0819813, 4995e05 | Table-driven unit tests for both charsets |

## 2. Every wait has a deadline

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-09 | Separate timeouts for DNS, connect (about 10 s), first byte (about 20 s) and body silence (about 20 s), all inside a total deadline. DNS counts against the connect deadline. | 6893072, 1d277a4, 6888ffa | Server that stalls at each phase |
| L-10 | The probe's timeout covers the whole redirect chain. | 34541f1 | Redirect loop / slow redirect |
| L-11 | Happy Eyeballs across a network's own addresses, 250 ms apart, with the last address that worked tried first. A stale IPv6 address must not use up the whole budget. | 6888ffa, 22de4b1 | Dual-stack host with a dead AAAA record |
| L-12 | Catch slow connections, not only silent ones: reconnect a stream running under 10% of its **own network's** median speed for 10 s, at most twice per block. | 6eddb0e (stuck at 99%) | Trickle server |
| L-13 | Pause the stall watchdog while the disk is applying backpressure or a speed limit is holding the stream. | 34541f1 | Slow-disk test does not report "network unreachable" |

## 3. Know which layer failed

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-14 | Busy server (408/429/500/502/503/504): wait it out for up to 5 min, honour Retry-After (seconds or HTTP date) capped at 2 min, **for the whole network**. | 34541f1 | 429 with an HTTP-date Retry-After (Plexo never tested this) |
| L-15 | Other 4xx/5xx and protocol errors count as strikes; a stream retires after 5 strikes in a row. | §A.3 | Unit test of the retry policy |
| L-16 | Connection errors never use up retries while the network still exists. With no network at all, **wait**, don't fail, and keep the partial file. | 806a261 | Pull the interface mid-download (netns) |
| L-17 | A network that can't reach the host is marked `unreachable` and keeps 1 probe stream retrying every 5 s. | §B.3 #55 | netns: block one path's route |
| L-18 | A stream that is accepted but never answered, while others on the same network are served, counts as a **refusal**. | 8dae93f | Server that accepts N connections and ignores the rest |
| L-19 | 401/403/404/410 on a range request means the link expired: offer "Fix link" and keep the bytes. | errors.ts LINK_REFUSED | Expired signed URL test |
| L-20 | IP-locked signed URLs and per-network 403s: probe each network; drop the ones that get 403 or a redirect. | Market research §5.2, risks §7.5 | Server that allows only one client IP |
| L-21 | Sleep/wake: reset clocks, reconnect everything, refresh interfaces. Keep the machine awake while transferring (optional). | 34541f1 | Simulated suspend event |

## 4. Scheduling and concurrency

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-22 | Keep the work unit separate from the display unit (block size is never tied to the grid's square count). | 196ce9b | Unit test |
| L-23 | Interleave stream starts across networks so every network gets a first block before any gets a second. Cap a network at its share of blocks. | 3f3129b (the second network got 0 work) | Property test of the planner |
| L-24 | The scheduler is a **pure function of a snapshot**, property-tested for no deadlock and no lost blocks. | be75817, aed305b | proptest |
| L-25 | Race slow tail blocks instead of only retrying them. Hedges write the same bytes to the same offsets; the first to finish wins. | be75817, bdfa59f | Hedge test with a slow path |
| L-26 | Simple concurrency rule beats a clever one on noisy links: start at 8, double while every stream is served, cap at 32, lower only on refusals, recover one stream a minute. **Keep a manual override.** | a323149 → f9d1d72 (churn), issue #50 | Unit simulations |
| L-27 | The disk is a resource the controller sees: judge per download, cut streams in half while the disk is behind, double back after 10 s of keeping up. No hedging while the disk is behind. | e8b84b6, issue #77 (HDD 53 vs 328 Mbit/s) | Slow-disk test |
| L-28 | Any dynamic re-chunking writes at offsets; never reassemble by creation order. | 2fdb33b | SHA-256 of every completed file |
| L-29 | Serialize the lifecycle: a new run waits until the old one has fully stopped, and there is one exit path that hands work back. | bb6a711 | Pause/resume hammering test |

## 5. Measuring and showing speed

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-30 | Never derive a rate from gaps between events. Count bytes into meters and read the meters on a clock tick, over a window of at least 1 s (3 s rolling). | b0d14a2, 871dcfd, 5404d92 | Unit test with bursty input |
| L-31 | Zero a stream's speed while it waits to retry. A ticker's lifetime belongs to the run that created it. | 8f33a22 | Unit test |
| L-32 | ETA is computed once, in the core, with asymmetric smoothing (falls 30%/s, rises 10%/s). | 5404d92 | Unit test |
| L-33 | Peak = best 5 s average, never below AVG, computed from the same bytes as AVG. | 21ad8c0 (PEAK < AVG in 40 of 20k runs) | Property test |
| L-34 | Send only changed state to the UI (deltas with sequence numbers), at most about 5–10 Hz. | 5851ec0 (3 ms per push at 4096 blocks) | Benchmark |
| L-35 | Data usage rounds **down** (4.96 of 5 GB shows "4.9 of 5"). | 21ad8c0 | Unit test |

## 6. Disk and files

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-36 | One staging file at fixed offsets, on the destination volume, then rename. About 1× the file size is needed, not 2×. | 0774aa6 → 98e5896 | Disk-space test |
| L-37 | Claim names with an exclusive create, never "check whether it exists". | 515202b, d7dc405 | Two downloads with the same name at the same time |
| L-38 | Don't rely on hard links (they fail on exFAT). Check the target just before renaming, then rename. | a131436 | exFAT/FAT image test |
| L-39 | 255-**byte** path component limit, truncating whole code points, with room left for ` (9999).fuselane`. | paths.ts | Unit test with emoji/CJK names |
| L-40 | Sanitize names for every OS: separators, control characters, `<>:"\|?*`, trailing dots and spaces, CON/PRN/AUX/NUL/COM1–9/LPT1–9 (superscript digits too), NTFS alternate streams. | 9cc4db5 | Table-driven tests |
| L-41 | Mark staging files sparse on NTFS (`FSCTL_SET_SPARSE`); exFAT/FAT refusing is fine. | e8b84b6 | Windows CI: `fsutil sparse queryflag` |
| L-42 | What's on disk is the truth: reconcile with the file's real contents before resuming. Track bytes received separately from bytes the writer accepted, and resume only from accepted bytes. | 6893072, §B.3 #33 | Crash-at-any-byte chaos test |
| L-43 | Guard the expensive failure: refuse to publish unless every block is complete and the size matches. Delete partial output on a failed publish. | a5f552f | Fault-injection test |
| L-44 | Every abort path closes its file handles. Count descriptors per abort. | 7911897 | Handle-leak check on all OSes (Plexo skipped it on Windows) |
| L-45 | Windows: wait for the handle to **close** before reopening or renaming; retry renames on EPERM/EACCES/EBUSY (antivirus, indexers, sync clients). | 9cc4db5, fd411f2 | Windows CI |
| L-46 | Check free space before starting **and** while running (Plexo checked only before). Map ENOSPC to a clear, immediate pause, not 5 retry strikes. | 029387e, §C.12 | Small-volume test, ENOSPC partway through |
| L-47 | Cap any filesystem call on the startup path (a missing network drive must not block launch). | 765964f | Unmounted destination test |
| L-48 | A partial download on an unplugged drive is never deleted; tell the user "Reconnect the destination drive". | §B.3 #104 | Test |

## 7. Persistence

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-49 | Crash-safe writes: SQLite in WAL mode (our choice). For any file we write ourselves: temp file, fsync, rename, fsync the directory. Plexo never fsynced. | Plexo Part 7.3 | Power-cut simulation (kill -9 during writes) |
| L-50 | "Missing" is not "unreadable": never overwrite a file you couldn't read, and never silently reset user data on corruption. Keep a backup and tell the user. | fd411f2, Part 7.3 | Corrupt-DB test |
| L-51 | Versioned schema with forward **migrations**; never drop someone's in-progress downloads on upgrade. | Plexo cleared manifests v1–v7 | Migration tests from every released schema |
| L-52 | Freeze writes while shutting down. Quit has a hard deadline (about 3 s). | d86fc36, fc6e675 | Quit-during-download test |
| L-53 | Restored state is untrusted input: validate it with the same checks as live state. Restored downloads come back paused, never auto-started. | 036749d, §B.3 #102 | Fuzz the DB rows |
| L-54 | Write the intent to publish (path + file identity) before the rename, so a crash mid-publish recovers. | §B.3 #22 | Crash between steps |
| L-55 | fsync the staging file before recording its progress as durable. | §B.3 #101 | Power-cut test |

## 8. Networks and operating systems

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-56 | A source IP alone does **not** pin a socket to an interface. Use `SO_BINDTODEVICE` (Linux ≥ 5.7, unprivileged), `IP_BOUND_IF`/`IPV6_BOUND_IF` (macOS), `IP_UNICAST_IF`/`IPV6_UNICAST_IF` (Windows; IPv4 index in **network byte order**). | 32c1a69, issue #65 | netns lab + real-hardware matrix |
| L-57 | Detect pinning support at runtime and degrade *visibly* (the user is told only the default network can be used). | 32c1a69 | Old-kernel test |
| L-58 | Never pin loopback destinations. | 81ebb6e | Local-server test |
| L-59 | Match IP families between local and remote addresses; strip `[]` from IPv6 literals before handing them to sockets. | 6888ffa | IPv6-only host test |
| L-60 | Exclude link-local addresses (169.254/16, fe80::/10). Filter virtual and VPN adapters (Hyper-V, VMware, VirtualBox, WSL, TAP/TUN) by default. | 9cc4db5, issue #29 | Adapter classification fixtures |
| L-61 | Never parse localized CLI output. Use native APIs (SCNetworkInterface, GetAdaptersAddresses, sysfs/NetworkManager). | 21ad8c0 | Fixtures in other languages |
| L-62 | Detect setups where bonding can't help (same subnet, same public IP / upstream, Windows turning Wi-Fi off when Ethernet is plugged in) and tell the user how to fix them. | 32f5e09, issue #20 | Per-network public-IP probe |
| L-63 | Use OS change notifications for interfaces, with polling as a fallback. On an address change, wake streams; when an address is lost, drop that network's sockets. | 806a261, Part 7.3 | netns: change the address mid-download |
| L-64 | A network appearing mid-download starts **off** for that download. A vanished one goes `offline` and is reused when it returns. | §B.3 #79–80 | Test |
| L-65 | Per-network DNS: resolve through the network itself (different networks can get different CDN edges, and the primary's DNS may not answer over the secondary). | Tech research §2.4 | Per-link DNS server in the netns lab |
| L-66 | Proxies set at system level silently defeat pinning: per-interface clients ignore the system proxy unless the user picks one. | Tech research §2.1 | Test with `HTTPS_PROXY` set |

## 9. Torrents

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-67 | Use a mature engine; don't write BitTorrent yourself. Pin each peer by giving the engine a connector, and turn off any transport that bypasses it until it can be pinned. | 5404d92 | Local swarm test |
| L-68 | Sanitize torrent paths exactly as the store will write them: traversal, case-insensitive collisions, a file that is also a folder prefix, Windows-invalid names. | §B.3 #109–110 | Hostile `.torrent` fixtures |
| L-69 | Removing a torrent validates every path and refuses symlinks and folder swaps. Delete edge-piece bytes written into files the user didn't choose. | §B.3 #113–114 | Test |
| L-70 | Isolate every test's temp folders, **including a library's defaults** (shared `/tmp/webtorrent` caused 1-in-12 flakes). | 5404d92 | Test harness rule |
| L-71 | Attribute incoming peers to a network by the socket's local address. | 5404d92 | Test |

## 10. Packaging and release

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-72 | **Sign everything we can for free from the first public build, and never make users type security commands** (SignPath on Windows; ad-hoc + install script + Homebrew tap on macOS, ADR 0009). Unsigned builds plus `xattr` instructions were Plexo's top support burden. | Issues #15, #27, #35, #46 | Release job checks signatures; first-launch test per install path |
| L-73 | Build every target on its native runner; never ship one OS's native modules in another OS's package. Assert package contents after packaging. | 8cc15e4 (rc.12 Windows torrent crash) | Package-content check in CI |
| L-74 | Set the architecture explicitly for every target and put it in the artifact name. | f892510 | CI matrix |
| L-75 | Smoke-launch the **packaged** app in CI on every OS (Plexo only tested the unpackaged build). | Part 6 §5.4 | `fuselane --version` and the app's headless self-test in CI |
| L-76 | Use real semver: test that pre-release < release, and that `1.0.0` > `1.0.0-rc.14`. | updateCheck.ts (latent bug) | Unit test |
| L-77 | Know what your update API ignores (GitHub `/releases/latest` skips pre-releases). Serve our own signed feed. | 3946d64 | Updater e2e test |
| L-78 | Match versions exactly when selecting artifacts (`rc.1` must not match `rc.10`). | 873cef9 | Unit test |
| L-79 | Never drop a platform's CI job. | 98ad2d7 → 8cc15e4 | Branch protection: all OS jobs required |
| L-80 | Use each tool's official, idempotent installer, not homemade scripts. | 1861af2 | – |

## 11. UI

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-81 | Truncated text needs normal line-height (`truncate` + `leading-none` clipped descenders 3 times). | e1e2f4e, 6be78d5 | Lint rule / visual review |
| L-82 | Global CSS resets go in `@layer base`, or they beat every utility class. | 97b001c | Review |
| L-83 | Disabled controls that have explanatory tooltips must stay focusable (`aria-disabled`). | e091cfb | a11y test |
| L-84 | A rescan keeps the last result on screen. Watch for mount effects that change state which unmounts the same component (it caused 4000 scans/s). | 7d30ea6 | Test with zero networks |
| L-85 | Every error goes through **one** translator into plain language, with an action. Never show raw internal errors. | df45254, 38a954d | Snapshot of the error catalogue |
| L-86 | Consistent vocabulary: "combine" for networks, "assemble" for files, "streams", "blocks". Never two contradictory percentages. | c9ff927 | UX copy review |
| L-87 | Build a standalone repro before fixing a layout bug; don't stack workarounds. | Subgrid saga | Process |
| L-88 | Cached startup state goes stale after a UI reload; read it fresh. | 93da97a | Reload test |

## 12. Tests and CI

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-89 | Fast pure tests run on **every PR** (Plexo ran about 90 of them nightly only). | Part 6 §5.3 | CI config |
| L-90 | Check invariants on every event, and the SHA-256 of every finished file. | aed305b, 15311a6 | Test harness |
| L-91 | Treat runtime warnings and leaked handles as test failures. | aa3c256 | Harness |
| L-92 | Tests guard risks, not constants or styling. Delete tests that pin UI details. | 4a23c10, 5404d92 | Review |
| L-93 | Real multi-interface tests (network namespaces + `tc netem`), not "LAN IP to loopback". Report skipped tests loudly. | Part 6 §5.1, §5.8 | netns job required in CI |
| L-94 | Retries are off in CI; a known bug is written as an expected-failure test. | Part 6 §4 | Config |
| L-95 | When adding a CI gate, make main pass it in the same change. | 4de3788 | Process |
| L-96 | Kill whole process trees in test teardown on Windows. | 3272d6b | Harness |

## 13. Security

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-97 | Validate every IPC and API payload at runtime (Plexo's were type-checked only), and allow only http, https and magnet URLs. | Part 7.2 | Fuzz the IPC commands |
| L-98 | The UI can't choose arbitrary write paths: only user-chosen folders plus a sanitized file name. | Part 7.2 | Test |
| L-99 | Keep quarantine / Mark of the Web on downloaded files. | Tech research §5.4 | Per-OS test |
| L-100 | Test-only knobs are compiled out of release builds, not just ignored at runtime. | testKnobs.ts | Release build check |

## 14. Learned in our own spikes

| ID | Rule | Evidence | Guard |
|---|---|---|---|
| L-101 | macOS iCloud Private Relay relays plain-HTTP traffic even from pinned raw sockets. Network probes (public IP, shared upstream) use HTTPS only; detect Private Relay and explain its effect on `http://` downloads. | Spike S2 | Probe tests use https; S2 part 2 result |
| L-102 | Tunnel and peer-to-peer interfaces (`utun*`, `awdl*`, `llw*`, `anpi*`, bridges) can make pinned connects *hang* instead of failing; exclude them unless they have a gateway and pass the reachability probe, and always use a connect deadline. | Spike S2 (`utun0` IPv6 hung 4 s) | netif filter fixtures |
