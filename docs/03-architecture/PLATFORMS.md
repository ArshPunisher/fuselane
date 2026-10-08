# Platforms: specifics, quirks, packaging

## How we build for three OSes together

- **One codebase, written for all three from the first line.** OS-specific code is isolated behind traits (`netif::Watcher`, `transport::Pinner`, `storage::FileOps`, `os::Integration`) with `cfg(target_os)` modules plus a fake for tests.
- **Develop daily on macOS** (the main dev machine), but **CI builds and tests on macOS, Windows and Linux on every PR from the first commit** (L-79).
- **Features are built as vertical slices:** a slice is done only when it works on all three. No "Windows later".
- **Real hardware checks** (Wi-Fi + phone tether) on each OS at the end of every phase, recorded in [`../05-quality/TESTING.md`](../05-quality/TESTING.md) §5.

## Minimum versions (proposed)

| OS | Minimum | Why |
|---|---|---|
| macOS | 13.3 Ventura | WKWebView with Safari 16.4 features (needed by Tailwind v4) |
| Windows | 10 21H2 (x64, ARM64) | WebView2 runtime, `IP_UNICAST_IF` |
| Linux | kernel ≥ 5.7, glibc of Ubuntu 22.04, WebKitGTK 4.1 | Unprivileged `SO_BINDTODEVICE`; build on 22.04 for glibc compatibility |

## macOS

| Topic | Detail |
|---|---|
| Pinning | `IP_BOUND_IF` / `IPV6_BOUND_IF` |
| Names | SystemConfiguration (`SCNetworkInterface*`) |
| Changes | SCDynamicStore / `nw_path_monitor` |
| DNS | `DNSServiceGetAddrInfo` scoped to the interface |
| Disk | APFS sparse by default; `F_PREALLOCATE`; `F_FULLFSYNC` only on the final publish |
| Tethering | iPhone USB works natively. Android needs a third-party RNDIS driver (TetherKit); the app has a guide. |
| Integration | Deep links via Apple Events; `LSHandlerRank = Alternate` for magnet/torrent; dock progress and badge; menu-bar mode; quarantine xattr on published files |
| Packaging | Universal `.app` in a DMG, Developer ID signed, notarized with `notarytool`, stapled; Homebrew cask |
| CI | macOS arm64 runner for build + tests; Intel built as part of the universal binary |
| Gotcha | WKWebView has no WebDriver, so real-app UI e2e isn't possible; use the packaged smoke test + Vitest/Playwright against a mocked IPC + manual runs |

## Windows

| Topic | Detail |
|---|---|
| Pinning | `IP_UNICAST_IF` (IPv4 index in network byte order) / `IPV6_UNICAST_IF` + source bind |
| Names | `GetAdaptersAddresses` FriendlyName / IfType; filter Hyper-V, WSL, VPN TAP |
| Changes | `NotifyIpInterfaceChange`, `NotifyUnicastIpAddressChange` |
| DNS | `DnsQueryEx` with InterfaceIndex |
| Disk | `FSCTL_SET_SPARSE`; `SetFileInformationByHandle(FileAllocationInfo)`; retry renames on EPERM/EBUSY (antivirus); wait for the handle to close |
| Policy gotcha | Windows may disconnect Wi-Fi when Ethernet is plugged in, so the app has a guided fix |
| Integration | No default handler registration (respect existing torrent clients); "Open with" works; taskbar progress; tray; Zone.Identifier on published files |
| Packaging | NSIS per-user installer (folder can be changed), x64 + ARM64, signed (Microsoft Artifact Signing or SignPath); winget |
| CI | `windows-latest` for build + tests + tauri-driver e2e; `windows-11-arm` optional |
| Gotchas | NSIS declaration order matters and warnings are errors; kill whole process trees in tests (L-96); `aws-lc-rs` needs CMake/NASM, so use the `ring` provider if that's painful |

## Linux

| Topic | Detail |
|---|---|
| Pinning | `SO_BINDTODEVICE` (checked at runtime; kernel ≥ 5.7) |
| Names | sysfs + NetworkManager D-Bus |
| Changes | rtnetlink |
| DNS | systemd-resolved per-link `ResolveHostname`, or hickory pinned |
| Disk | `fallocate` (fall back to `set_len` on EOPNOTSUPP); sparse by default |
| Integration | `.desktop` MimeType for magnet/torrent (offered, not default); tray via AppIndicator |
| Packaging | AppImage (auto-update), deb, rpm (x64 + arm64), Flatpak (`--share=network`, `--filesystem=xdg-download`). Snap skipped (its sandbox limits interface binding). |
| CI | `ubuntu-22.04` build (oldest glibc); netns network-lab job; tauri-driver e2e with Xvfb (`-noreset`) |
| Gotchas | WebKitGTK on NVIDIA + Wayland shows blank windows: ship the documented environment-variable workarounds behind a setting; keep the UI light (no heavy blur); AppImage needs FUSE 2 on newer Ubuntu (document it) |

## Android (Phase 10)

`ConnectivityManager.requestNetwork(TRANSPORT_CELLULAR)` while on Wi-Fi; pass `network.networkHandle` to Rust and pin sockets with `android_setsocknetwork`; per-network DNS with `android_getaddrinfofornetwork`; foreground service `dataSync` or user-initiated data transfer jobs; a Kotlin/Compose UI over uniffi bindings.
