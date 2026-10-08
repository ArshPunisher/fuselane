# Multi-interface download/upload app: technology research

> Research note (2026-10-08): written while the project was unnamed. Example names in the layouts and manifests below were changed to Fuselane. Every other mention of "Plexo" refers to the open-source project we studied (github.com/anmolkapil/plexo).

Date: 2026-10-08. Versions were pulled live from crates.io, npm and GitHub on this date. Where a claim comes from reading crate source, the file is named. Items marked **(verify)** come from memory or secondary sources and should be checked before you rely on them.

---

## 0. TL;DR

- **Shell:** Tauri **2.12.1**, with React **19.3** + Vite **8.3** + Tailwind **4.3**. The Rust core is a separate crate (`core`) behind a daemon/CLI binary. The Tauri app is a thin client and can embed the core or talk to the daemon.
- **Per-interface sockets:** `socket2` **0.6.5** gives you Linux (`bind_device`) and macOS (`bind_device_by_index_v4/v6`). **Windows support is in none of socket2, reqwest or hyper-util.** You have to call `setsockopt(IP_UNICAST_IF / IPV6_UNICAST_IF)` yourself through `windows-sys`. So: write your own hyper connector, or create the socket yourself and use reqwest's `connector_layer`. Use **one `reqwest::Client` per interface**.
- **Torrent:** **librqbit 9.0.1** can only bind the *whole session* to *one* device (`SessionOptions::bind_device_name`), and that **doesn't work on Windows**. Per-peer interface selection is not exposed, because `StreamConnector` is `pub(crate)`. **libtorrent-rasterbar 2.1.2** has `outgoing_interfaces` (a comma-separated list that it round-robins over) and `listen_interfaces`. That is the only off-the-shelf multi-interface engine, but you'd be maintaining C++ bindings (cxx 1.0.202). Recommendation: start on librqbit and keep a fork or upstream PR that exposes a per-peer connector; keep libtorrent as plan B.
- **Bonded upload:** S3-compatible multipart through presigned `UploadPart` URLs. **Cloudflare R2 requires every part except the last to be the same size.** Backend: Cloudflare Workers + R2 + D1 (no egress fees). Encryption: chunked AEAD with per-part independent framing (STREAM-style), *not* one `age` stream.
- **Extension:** WXT **0.21.4** (MV3 for Chrome/Edge/Brave/Firefox) + native messaging (primary channel) + a localhost WebSocket with a pairing token (fallback / Safari).
- **Signing:** Apple Developer ID + `notarytool`; Microsoft **Artifact Signing** (the renamed Azure Trusted Signing); SignPath Foundation for OSS; Tauri updater with minisign keys.

---

## 1. App shell

### 1.1 Tauri 2 (recommended)

Current versions (crates.io / npm):

| Package | Version |
|---|---|
| `tauri` / `@tauri-apps/cli` / `@tauri-apps/api` | 2.12.1 |
| `tauri-plugin-updater` | 2.13.2 |
| `tauri-plugin-single-instance` | 2.5.2 |
| `tauri-plugin-deep-link` | 2.6.1 |
| `tauri-plugin-notification` | 2.5.1 |
| `tauri-plugin-autostart` | 2.7.0 |
| `tauri-plugin-shell` | 2.4.1 (use `tauri-plugin-opener` 2.7.0 for "open URL/reveal file"; shell's `open` is deprecated) |
| `tauri-plugin-dialog` | 2.8.1 |
| `tauri-plugin-fs` | 2.6.0 |
| `tauri-plugin-process` | 2.4.0 (relaunch after update) |
| `tauri-plugin-log` | 2.10.0 |
| `tauri-plugin-store` | 2.5.0 |
| `wry` | 0.57.0 |
| `tauri-driver` | 2.1.0 |

Plugin notes and gotchas:

- **Permissions/capabilities (v2 ACL):** every plugin command must be granted in `src-tauri/capabilities/*.json` (for example `"updater:default"`, `"fs:allow-read-file"` scoped to paths). Keep `fs` scopes narrow. Large file I/O should happen in Rust anyway, never by streaming bytes through IPC.
- **single-instance + deep-link:** register single-instance **first**. With the deep-link plugin's `deep-link` feature, the second instance's argv (the `yourapp://` URL) is forwarded to the first. On macOS, deep links arrive through Apple Events (no second process). On Windows/Linux they arrive as argv. Linux needs a `.desktop` file with `MimeType=x-scheme-handler/yourapp`. AppImage needs `register_all()` at runtime.
- **updater:** needs `createUpdaterArtifacts: true` and a minisign keypair (`tauri signer generate`). The private key goes in `TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD`) in CI. Endpoints serve a static `latest.json` (GitHub Releases works) or a dynamic server. On Linux it only updates AppImage. deb/rpm users need the distro repo. On Windows the updater runs the NSIS/MSI installer (`installMode: "passive"`).
- **autostart:** use `MacosLauncher::LaunchAgent`. Pass an `--autostart`/`--minimized` arg so the app starts to tray.
- **notification:** on macOS, notifications from unsigned dev builds often don't show. Test with a signed build.
- **Sidecar** (`bundle.externalBin`, binaries named `name-<target-triple>`): useful if you ship the headless daemon (`fuselaned`) as a separate binary that the GUI spawns. Each sidecar must be signed and notarized too. Simpler alternative: link the core as a library into the Tauri binary *and* ship a separate CLI/daemon binary from the same workspace. **Recommended:** the GUI embeds the core in-process by default and can attach to a running daemon over a local IPC socket (Unix domain socket / Windows named pipe) when one is present.
- **IPC throughput:** use `tauri::ipc::Channel<T>` for progress streams (it's ordered and fast). Throttle to ~10 Hz per transfer. Don't emit an event per chunk.

**Webview differences and gotchas**

| | macOS: WKWebView | Windows: WebView2 (Chromium) | Linux: WebKitGTK (webkit2gtk-4.1) |
|---|---|---|---|
| Engine | Safari/WebKit of the installed OS | Evergreen Edge Chromium | Distro-packaged WebKitGTK (version varies by distro) |
| Gotchas | Behaves like Safari: no `showSaveFilePicker`, stricter autoplay. Custom protocol `tauri://` vs `http://tauri.localhost` on Windows matters for CORS/cookies. Older macOS = older Safari, so set a CSS/JS baseline to your minimum macOS. | Needs the WebView2 runtime (preinstalled on Win 11; the NSIS `webviewInstallMode` bootstrapper handles Win 10). Data dir lives in `%LOCALAPPDATA%`. | **Biggest risk.** NVIDIA + Wayland: blank/white windows, "Error 71" crashes, resize crashes. Official workarounds: `__NV_DISABLE_EXPLICIT_SYNC=1`, `WEBKIT_DISABLE_DMABUF_RENDERER=1`, last resort `WEBKIT_DISABLE_COMPOSITING_MODE=1` ([Tauri Linux graphics doc](https://v2.tauri.app/develop/debug/linux-graphics/)). Slower JS/CSS than Chromium, so avoid heavy blur/backdrop-filter and large animated lists. No WebDriver on macOS. |

- Keep the UI modest (virtualized lists, no heavy GPU effects) so WebKitGTK stays smooth. Test in CI on Ubuntu 22.04 and 24.04 images.
- **CEF runtime** for Tauri is in alpha (`tauri-cef` 3.0.0-alpha tags, `cef-rs` crate) **(verify)**. Don't plan on it for v1. It would fix Linux consistency but bring back Chromium's size.
- Tauri on Android exists (Tauri mobile). For this app, a native Kotlin UI with a uniffi core is a better fit on Android because of the `ConnectivityManager` integration (see section 7).

### 1.2 Alternatives compared

| Option | Version (2026-10) | Installer size (typical hello-world → real app) | Idle RAM | Notes |
|---|---|---|---|---|
| **Tauri 2** | 2.12.1 | ~3–10 MB | ~30–80 MB (+ system webview processes) | Rust core is native, which fits the networking-heavy design. Webview inconsistency on Linux. |
| Electron | 44.7.0 | 80–150 MB | 150–300 MB | Rejected (too heavy). Consistent Chromium everywhere. |
| Wails | v2.14.0 stable; v3.0.0-beta.28 (2026-10-05) | ~8–15 MB | similar to Tauri | Go backend. Your networking core would then be Go (also fine: `syscall.SetsockoptInt` for `IP_BOUND_IF`, `SO_BINDTODEVICE`, `IP_UNICAST_IF`). v3 is still beta. A smaller ecosystem than Tauri's plugins. |
| Flutter desktop | 3.x stable | 20–40 MB | 80–150 MB | Self-rendered, so consistent UI. Rust through `flutter_rust_bridge`. Good if you want one UI for desktop + Android. Weaker desktop-native feel (tray, menus) without plugins. |
| Slint | 1.18.1 | ~5–10 MB | 20–50 MB | Native Rust UI, tiny. Licence: GPLv3 / royalty-free desktop licence / commercial. Fewer UI devs know it, and dashboards/charts take more custom work. |
| Native ×3 (SwiftUI / WinUI 3 / GTK4) | – | smallest | smallest | Best UX, about 3× the UI cost. Only worth it later on macOS. |

Verdict: **Tauri 2** gives the best balance. The Rust core is shared with the CLI/daemon/Android, and the web UI is shared with the extension's popup/options pages.

### 1.3 Frontend

- **React 19.3.0 + Vite 8.3.3 + Tailwind CSS 4.3.3** (`@tailwindcss/vite` plugin, CSS-first `@theme` config, no `tailwind.config.js`). Tailwind v4 needs Safari 16.4+ (it uses `@property`, `color-mix()`, cascade layers), which means **macOS 13.3+ WKWebView**. Set your minimum macOS to 13. Older WebKitGTK (Ubuntu 22.04 ships 2.4x) is generally OK for v4 **(verify on 22.04)**.
- State: TanStack Query for daemon calls + Zustand. Lists: TanStack Virtual. Router: TanStack Router (or none, since this is a small app).
- Generate TS types from Rust with `specta` + `tauri-specta` (or `ts-rs`) so the IPC is typed end to end.
- Alternatives: SolidJS or Svelte 5 give smaller bundles and faster rendering on WebKitGTK. React is fine if lists are virtualized.

---

## 2. Rust networking per interface

### 2.1 Socket-level interface pinning: what each library actually exposes

I read the source of socket2 0.6.5 (`src/sys/unix.rs`, `src/sys/windows.rs`), reqwest 0.13.5 and hyper-util 0.1.21:

| Platform | OS mechanism | socket2 0.6.5 (`features = ["all"]`) | reqwest 0.13.5 `ClientBuilder::interface()` | hyper-util 0.1.21 `HttpConnector::set_interface()` |
|---|---|---|---|---|
| Linux / Android | `SO_BINDTODEVICE` | `Socket::bind_device(Some(b"wlan0"))` (linux, android, fuchsia) **and** `bind_device_by_index_v4/v6` (`IP_BOUND_IF`-style is also cfg'd for linux/android) | ✅ | ✅ (uses `SO_BINDTODEVICE`) |
| macOS / iOS | `IP_BOUND_IF` / `IPV6_BOUND_IF` (takes ifindex) | `bind_device_by_index_v4(NonZeroU32)` / `bind_device_by_index_v6` (macos, ios, tvos, watchos, visionos, illumos, solaris, linux, android) | ✅ (the cfg list includes macos/ios) | ✅ |
| **Windows** | `IP_UNICAST_IF` (level `IPPROTO_IP`) / `IPV6_UNICAST_IF` (level `IPPROTO_IPV6`) | ❌ **not exposed** (grep finds nothing for `UNICAST_IF`) | ❌ (`interface()` doesn't exist on Windows; it is cfg'd out) | ❌ |

`local_address()` / `set_local_address()` (bind to a source IP) is available on all platforms in reqwest and hyper-util.

**Windows implementation** (with `windows-sys` 0.61):
```rust
use windows_sys::Win32::Networking::WinSock::{setsockopt, IPPROTO_IP, IPPROTO_IPV6, IP_UNICAST_IF, IPV6_UNICAST_IF};
// IPv4: the index MUST be in network byte order. IPv6: host byte order. (Classic gotcha.)
let v4: u32 = (if_index as u32).to_be();
setsockopt(sock, IPPROTO_IP, IP_UNICAST_IF, &v4 as *const _ as _, 4);
let v6: u32 = if_index as u32;
setsockopt(sock, IPPROTO_IPV6 as i32, IPV6_UNICAST_IF, &v6 as *const _ as _, 4);
```
Set it **before `connect()`**. Also `bind()` to the interface's unicast address. Windows uses the strong host model, so a source bind plus `IP_UNICAST_IF` is reliable. Docs: <https://learn.microsoft.com/windows/win32/winsock/ipproto-ip-socket-options>.

**Platform gotchas:**
- **Linux:** binding only to a source IP is *not* enough. Linux is weak-host and routes by the main table, so packets leave the default interface with the wrong source address (dropped by `rp_filter`/upstream). Use `SO_BINDTODEVICE`. Unprivileged `SO_BINDTODEVICE` has been allowed since kernel **5.7** (before that it needed `CAP_NET_RAW`). An alternative is policy routing (`ip rule add from <ip> table N`), but that needs root, so avoid it.
- **macOS:** source-IP binding alone also doesn't pin the egress interface. Use `IP_BOUND_IF`. It works unprivileged. VPNs (utun) with "include all networks" may override.
- **All OSes:** VPN/system proxies. reqwest's default feature `system-proxy` reads the OS proxy settings, which silently defeats pinning. Call `.no_proxy()` on the per-interface clients (or make it a user choice).
- **IPv6/Happy Eyeballs:** resolve per interface, then filter candidate addresses to families the interface actually has (a cellular link may be IPv6-only behind NAT64).

### 2.2 Wiring into hyper/reqwest

Options, from simplest to most flexible:
1. **Linux/macOS only:** `reqwest::Client::builder().interface("en1").no_proxy().build()`, one client per interface.
2. **Cross-platform (recommended):** your own connector. Create the socket with `socket2::Socket::new`, apply the platform pinning (`bind_device` / `bind_device_by_index_v4` / Windows `setsockopt`), bind to the source IP, set non-blocking, then convert it to a `tokio::net::TcpSocket::from_std_stream` → `connect()`. Wrap it as a `tower::Service<Uri>` and plug it into either:
   - `hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(connector)` with `hyper-rustls` (`HttpsConnectorBuilder::wrap_connector`), or
   - reqwest 0.13's `ClientBuilder::connector_layer(...)` (present in 0.13.5). Note that the layer wraps reqwest's built-in connector, so for full control over socket creation, hyper-util is cleaner.
3. Plug in a per-interface DNS resolver with `ClientBuilder::dns_resolver(Arc<impl Resolve>)` (reqwest) or a custom `Resolve` service for hyper-util.

**TLS:** reqwest 0.13 now defaults to **rustls** (`default-tls = ["rustls"]` → `aws-lc-rs` + `rustls-platform-verifier`). rustls is at **0.23.45**. `aws-lc-rs` needs CMake/NASM on Windows CI. Switch to the `ring` provider if that is painful. `rustls-platform-verifier` uses the OS trust store (corporate CAs work).

**HTTP/2 implications:** an HTTP/2 client keeps **one TCP connection per origin per pool**, and streams multiplex over it. So:
- One `Client` per interface is the unit of aggregation. Inside one interface, multiple H2 streams **don't** add bandwidth (one cwnd, TCP head-of-line blocking under loss).
- For segmented downloads, prefer **HTTP/1.1 with N parallel connections per interface** (`http1_only()`, `pool_max_idle_per_host(N)`). Many servers/CDNs throttle per connection, which is why IDM/aria2 split into connections. Make it adaptive: start with 2 per interface and grow while throughput improves (aria2 `-x/-s` style), capped at around 8 per host to stay polite.
- Range request checklist: send a `HEAD` or `GET` with `Range: bytes=0-0` and check for `206` + `Content-Range` + `Accept-Ranges: bytes`. Store the `ETag`/`Last-Modified` and send `If-Range` on every segment, because **different interfaces may hit different CDN edges/origins** (DNS per interface) with different object versions. Send `Accept-Encoding: identity`, since compressed transfer breaks byte ranges. Handle a server answering `200` (ranges ignored) by falling back to a single stream. Handle presigned URLs that expire mid-download (refresh through the extension/cookies).
- Work stealing: split into many small segments (e.g. 4–16 MiB) pulled from a shared queue. When the queue is empty, split the slowest in-flight segment (the "endgame") so a slow cellular link doesn't hold up completion.

**HTTP/3 / QUIC:** `quinn` **0.11.12**, `quinn-udp` 0.6.3. Create the UDP socket with socket2, pin it (on Linux/Android, `bind_device` works on UDP; on macOS `IP_BOUND_IF` works on UDP; on Windows `IP_UNICAST_IF` works on UDP), and pass it to `quinn::Endpoint::new(EndpointConfig, None, std_udp_socket, Arc<TokioRuntime>)`. One endpoint per interface. reqwest's `http3` feature is still **unstable** (needs `RUSTFLAGS="--cfg reqwest_unstable"`) and doesn't let you supply a custom UDP socket, so drive `h3` + `h3-quinn` directly if you want it. QUIC connection migration/multipath is not useful here (you want independent paths). **Recommendation: skip H3 for v1.** Most download hosts serve H1/H2, and per-socket pinning matters more.

**MPTCP:** socket2 exposes `Protocol::MPTCP` (Linux 5.6+). It only helps when the server supports MPTCP (rare). Optional, Linux-only.

### 2.3 Interface enumeration

| Crate | Version | Notes |
|---|---|---|
| **`netdev`** | 0.46.3 (2026-09) | Successor of `default-net` (0.22.0, last release 2024; deprecated). Gives index, name, friendly name (Windows), MAC, IPv4/IPv6 + prefixes, gateway, DNS servers (some platforms), `if_type`, operstate, MTU, default interface. **Recommended.** |
| `if-addrs` | 0.15.0 | Minimal (name, IP, index). Good fallback. |
| `network-interface` | 2.0.5 | Similar to if-addrs. |
| `if-watch` | 3.2.2 | Cross-platform address up/down *stream* (libp2p). Linux netlink, Windows `NotifyIpInterfaceChange`, macOS SystemConfiguration/fallback polling. |
| `netwatcher` | 0.8.0 (2026-08) | Cross-platform interface/address change callback. Worth evaluating against if-watch. |
| `rtnetlink` | 0.23.0 | Linux: subscribe to `RTMGRP_LINK | RTMGRP_IPV4_IFADDR | RTMGRP_IPV6_IFADDR | RTMGRP_IPV4_ROUTE`. |
| `system-configuration` | 0.8.0 | macOS SCDynamicStore bindings (watch `State:/Network/Interface/.*/IPv4` and `State:/Network/Global/IPv4`). |
| `windows` | 0.62.2 | `NotifyIpInterfaceChange`, `NotifyUnicastIpAddressChange`, `GetAdaptersAddresses`, `NotifyRouteChange2`. |

**Change notifications per OS:**
- **Linux:** netlink (`rtnetlink`). NetworkManager D-Bus for "metered"/connection type if available.
- **Windows:** `NotifyIpInterfaceChange` + `NotifyUnicastIpAddressChange` (iphlpapi). `INetworkListManager` / `Windows.Networking.Connectivity.NetworkInformation.NetworkStatusChanged` for cost/metered.
- **macOS:** `nw_path_monitor` (Network.framework, gives `nw_path_enumerate_interfaces`, `nw_interface_get_type` (wifi/cellular/wired/loopback/other), `nw_path_is_expensive`, `is_constrained`), or SCDynamicStore. Calling Network.framework from Rust: `objc2` + `block2`, or the small C shim / Swift package that most projects use.
- On every change, debounce (~1–2 s), diff by (ifindex, addresses), cancel sockets on vanished interfaces, and reschedule their segments.

**Friendly names / type:**
- **macOS:** `SCNetworkInterfaceCopyAll()` → `SCNetworkInterfaceGetBSDName` (en0) + `SCNetworkInterfaceGetLocalizedDisplayName` ("Wi-Fi", "USB 10/100/1000 LAN", "iPhone USB") + `SCNetworkInterfaceGetInterfaceType` (`kSCNetworkInterfaceTypeIEEE80211`, `…Ethernet`, `…WWAN`, `…Bluetooth`). Also check `nw_interface_get_type`. iPhone tethering shows as Ethernet (`iPhone USB`), so detect it by name.
- **Windows:** `GetAdaptersAddresses(AF_UNSPEC, GAA_FLAG_INCLUDE_GATEWAYS, …)` → `FriendlyName` ("Wi-Fi", "Ethernet 2"), `Description`, `IfType` (`IF_TYPE_IEEE80211`=71, `IF_TYPE_ETHERNET_CSMACD`=6, `IF_TYPE_WWANPP`=243, `IF_TYPE_WWANPP2`=244), `OperStatus`, `IfIndex`/`Ipv6IfIndex`, `FirstDnsServerAddress`, `FirstGatewayAddress`. Filter out virtual adapters (Hyper-V vEthernet, VirtualBox, WSL, VPN TAP) using `IfType` + `Description` + the `ConnectionType`/`TunnelType` fields.
- **Linux:** `/sys/class/net/<if>/type` (1 = ARPHRD_ETHER, 772 = loopback, 65534 = none/tun). `/sys/class/net/<if>/wireless/` or `phy80211/` exists for Wi-Fi. `/sys/class/net/<if>/uevent` has `DEVTYPE=wlan|wwan|bridge|vlan`. `/sys/class/net/<if>/device` is missing for virtual interfaces. USB tethering shows as `usb0`/`enx…` with `DEVTYPE=gadget`, or the driver is `rndis_host`/`cdc_ether`/`ipheth` (readlink `device/driver`). Friendly names come from NetworkManager D-Bus (`org.freedesktop.NetworkManager.Device.Interface` + the active connection `Id`), falling back to the ifname.
- Only offer interfaces that have a default gateway (or a reachable test endpoint). Run a short **per-interface probe** (HTTPS HEAD to a known endpoint over the pinned socket) to confirm real internet, because captive portals are common on Wi-Fi.

### 2.4 DNS per interface

The system resolver (`getaddrinfo`) uses the primary interface's DNS, which may not be reachable through, or may return poor CDN edges for, the secondary link.
- **`hickory-resolver` 0.26.3:** `NameServerConfig` has a `bind_addr: Option<SocketAddr>` (config.rs:350), and the `RuntimeProvider` trait has `connect_tcp(server_addr, bind_addr, timeout)` and `bind_udp(local_addr, server_addr)`. Implement a **custom `RuntimeProvider` that wraps `TokioRuntimeProvider`** and applies interface pinning (`IP_BOUND_IF` / `SO_BINDTODEVICE` / `IP_UNICAST_IF`) to every socket. A `bind_addr` alone isn't enough on macOS/Linux, as explained above. Point it at that interface's own DNS servers (from `netdev` / `GetAdaptersAddresses` / SCDynamicStore `State:/Network/Service/<id>/DNS` / systemd-resolved per link), with public DoH/DoT (1.1.1.1, 8.8.8.8) as fallback through the same pinned sockets. Then plug it into reqwest as a `Resolve` implementation.
- **macOS:** it has *scoped* resolvers (`scutil --dns` shows "scoped" entries per `if_index`). The proper API is Network.framework: `nw_parameters_require_interface` makes DNS scoped automatically. `DNSServiceGetAddrInfo(…, interfaceIndex, …)` (dns_sd) also does a scoped lookup. That's the easiest correct option on macOS. Call it through a small FFI.
- **Windows:** `DnsQueryEx` with `DNS_QUERY_REQUEST.InterfaceIndex` (Win 8+), or hickory pinned to that adapter's DNS servers.
- **Linux:** systemd-resolved D-Bus `org.freedesktop.resolve1.Manager.ResolveHostname(ifindex, name, family, flags)` does per-link resolution. Otherwise use hickory pinned to the interface plus the DHCP-provided DNS (from NetworkManager).
- **Android:** `Network.getAllByName()` or NDK `android_getaddrinfofornetwork(net_handle, …)`.

### 2.5 Disk I/O: sparse files, preallocation, positional writes, fsync

- **Positional writes:** `std::os::unix::fs::FileExt::write_all_at` (pwrite) / `std::os::windows::fs::FileExt::seek_write` (note: on Windows it *moves the file cursor* and can write short, so loop). Run them on a dedicated blocking writer pool (`tokio::task::spawn_blocking` or a fixed thread pool fed by a bounded channel), batching segments into ≥1 MiB writes. `tokio::fs` is just a threadpool wrapper, so control the pool yourself.
- **Preallocation:**
  - Linux: `fallocate(fd, 0, 0, len)` (`nix::fcntl::fallocate` / `rustix::fs::fallocate`). It's fast on ext4/xfs/btrfs. On filesystems without support (some FUSE/NFS) it returns `EOPNOTSUPP`, so fall back to `set_len`.
  - macOS: `fcntl(F_PREALLOCATE, fstore_t{F_ALLOCATECONTIG→F_ALLOCATEALL, F_PEOFPOSMODE, 0, len})` then `ftruncate(len)`. APFS is copy-on-write, so preallocation is advisory but avoids ENOSPC mid-download.
  - Windows: `SetFileInformationByHandle(FileAllocationInfo)` reserves clusters without extending EOF. Then `SetEndOfFile` / `set_len`. Gotcha: `set_len` on NTFS without sparse makes Windows **zero-fill lazily up to the highest written offset** (Valid Data Length), so a write at offset 9 GB first stalls while it zeroes. Either mark the file sparse (`DeviceIoControl(FSCTL_SET_SPARSE)`), or write segments roughly in order, or (admin only) use `SetFileValidData`. **Recommended on Windows: `FSCTL_SET_SPARSE` + `set_len`.**
  - `fs4` 1.1.0 provides `allocate()` cross-platform (fallocate / F_PREALLOCATE / SetFileInformationByHandle) plus file locks.
- **Sparse files:** Linux/macOS (APFS) files are sparse by default with `set_len`. Windows needs `FSCTL_SET_SPARSE`. FAT32/exFAT (USB drives) have no sparse support and a **4 GiB file limit on FAT32**, so check the free space and filesystem first.
- **Resume metadata:** store a bitmap of completed segments in a sidecar `.part.json` (or redb 4.3 / rusqlite 0.40 for the job DB). Only mark a segment done **after** its data is durable, with a write barrier (`fdatasync` per N MiB or per segment completion, not per write). On completion: `fsync` the file, rename `.part` → final, `fsync` the directory (Unix). macOS: `fsync` doesn't flush the disk cache. `F_FULLFSYNC` is only needed for the final commit, if at all.
- Verify with hashes when available (`Digest`/`Content-MD5` headers, user-provided SHA-256, torrent piece hashes). Use `xxh3`/`blake3` for internal segment checksums.
- Downloading to a "Downloads" folder on macOS needs the user-selected-file entitlement only if sandboxed. You'll ship Developer ID **non-sandboxed** (App Store sandboxing would block much of this).

---

## 3. Torrent engine

| | librqbit | libtorrent-rasterbar | cratetorrent | others |
|---|---|---|---|---|
| Version | **9.0.1** (2026-08-20), `librqbit-utp` 0.7.0 | **2.1.2** (2026-09-25) | 0.1.0 (2020, abandoned) | `transmission` (C, daemon over RPC), `anacrolix/torrent` (Go) |
| Language | Pure Rust, tokio | C++ (Boost.Asio). Rust bindings: `libtorrent-sys` 1.0.0 / `libtorrent` 0.1.1 are stale (2022), so write your own with `cxx` 1.0.202 | Rust | |
| uTP | ✅ (librqbit-utp, its own implementation; listen + connect) | ✅ (mature, LEDBAT) | ❌ | |
| DHT / PEX / magnet / trackers / UPnP | ✅ / ✅ / ✅ / HTTP+UDP / ✅ (`librqbit-upnp`) | ✅ all, plus BEP 52 (v2), web seeds, SSL torrents, I2P | minimal | |
| Bind to an interface | **One device per session:** `SessionOptions::bind_device_name: Option<String>` covers DHT, BT-UDP, BT-TCP, trackers and LSD ("On OSX will use IP(V6)_BOUND_IF, on Linux will use SO_BINDTODEVICE", session.rs:423). **On Windows it returns `Error::BindDeviceNotSupported`** (`librqbit-dualstack-sockets` 0.7.0, bind_device.rs:26–60). | **`outgoing_interfaces`** = comma-separated list of device names or IPs; outgoing peer connections are **round-robined** across them. **`listen_interfaces`** = `"0.0.0.0:6881,[::]:6881,eth1:6881…"`. Docs: <https://www.libtorrent.org/reference-Settings.html#outgoing_interfaces>. On Windows it binds by IP (strong host makes this effective) | – | Transmission: `bind-address-ipv4` (single) |
| Per-peer interface choice | ❌ **not exposed.** `StreamConnector` / `StreamConnectorArgs` are `pub(crate)`. `ConnectionOptions` only has `proxy_url` (SOCKS5), `enable_tcp`, `peer_opts`. | Round-robin across interfaces; no per-peer policy API, but a custom `session_proxy` isn't needed | – | anacrolix (Go) allows custom dialers |
| API/embedding | Clean Rust API, HTTP API (`http-api` feature), `Session::add_torrent`, storage-factory traits (custom storage = direct pwrite into your files), streaming | Very feature-complete, but a large C++ build (Boost, OpenSSL) for 3 OSes + Android NDK | | |
| Licence | Apache-2.0 | BSD-3 | | |

**How to get per-interface peers:**
1. **Fork/PR librqbit (recommended):** add a public `PeerConnector` trait (or `Arc<dyn Fn(SocketAddr) -> BoxFuture<TcpStream>>`) to `ConnectionOptions`. The code path is already isolated in `stream_connect.rs`. Implement round-robin or weighted (by measured per-interface throughput) assignment of peers to interfaces. Also add Windows support to `BindDevice` with `IP_UNICAST_IF` (an easy, upstreamable PR). For uTP, you'd need one `UtpSocketUdp` per interface. Start with TCP-only per-interface and uTP on the default interface. DHT can stay on one interface.
2. **SOCKS5 trick (no fork):** librqbit supports `proxy_url: socks5://…`. Run an *in-process* SOCKS5 server (one listener per interface) that dials out through pinned sockets. This only gives one proxy per session, so it still doesn't split peers. It would split only if your SOCKS server itself load-balances peer connects across interfaces. **That actually works:** a local SOCKS5 server that assigns each CONNECT to an interface (round-robin/weighted) gives you per-peer interface pinning with an unmodified librqbit (TCP only; disable uTP). A good v1 hack.
3. **libtorrent:** `settings_pack::outgoing_interfaces = "en0,en5"` works out of the box, including uTP. The cost is a C++ toolchain, cxx bindings and Android NDK builds. Also, its interface-change handling requires you to update the settings yourself.

Recommendation: **librqbit + the in-process SOCKS5 load-balancer for v1** (pure Rust, fast to ship, Windows works because your SOCKS dialer does the pinning), then upstream a proper connector hook. Keep libtorrent as the fallback if uTP-heavy swarms underperform.

Gotchas: the BitTorrent peer ID / listen port is shared, and inbound connections arrive on whichever interface has port mapping (UPnP per gateway). Seeding/upload ratio across a metered cellular link needs per-interface upload caps. Some ISPs/cellular carriers block or throttle BT.

---

## 4. Bonded upload + share links

### 4.1 Protocol constraints

| Service | Parallel parts? | Rules |
|---|---|---|
| **AWS S3** multipart | ✅ any order | Part size **5 MiB–5 GiB** (the last part may be smaller), **max 10,000 parts**, **max object 5 TiB** (AWS raised the max object size to **50 TB** in Dec 2025, **(verify)** whether the 10k-part / 5 GiB-part limits changed with it). Parts may differ in size on AWS. `CreateMultipartUpload` → `UploadPart` (presignable) → `CompleteMultipartUpload` with the ETag list. Use a lifecycle rule `AbortIncompleteMultipartUpload`. Checksums: `x-amz-checksum-crc32c`/`crc64nvme` per part. <https://docs.aws.amazon.com/AmazonS3/latest/userguide/qfacts.html> |
| **Cloudflare R2** | ✅ | Min 5 MiB (except last), max 5 GiB per part, 10,000 parts, 5 TiB object. **All parts except the last must be the same size** (an R2-specific constraint). Incomplete uploads auto-abort after **7 days** by default (configurable in lifecycle). Presigned S3 URLs work, including `UploadPart` via aws-sdk presigning against `https://<acct>.r2.cloudflarestorage.com`. Workers can also use the R2 binding's `createMultipartUpload` / `resumeMultipartUpload(key, uploadId).uploadPart()`, but routing bytes through a Worker hits request-body limits (100 MB on Free/Pro, up to 500 MB on Enterprise **(verify)**), so **upload parts directly to R2 with presigned URLs**. <https://developers.cloudflare.com/r2/objects/upload-objects/> |
| **Backblaze B2** | ✅ | Native API: `b2_start_large_file` → `b2_get_upload_part_url` (**one URL per concurrent thread**) → `b2_upload_part` → `b2_finish_large_file`. Parts 5 MB–5 GB, 10,000 parts, 10 TB file. Also S3-compatible. Egress free up to 3× stored, and free through Cloudflare (Bandwidth Alliance). |
| **tus 1.0** | ✅ via the **concatenation** extension | `Upload-Concat: partial` per part (each one its own upload), then a final `Upload-Concat: final;/files/a /files/b`. Server: `@tus/server` 2.4.5 (Node) or `tusd` (Go). Useful if you self-host. <https://tus.io/protocols/resumable-upload#concatenation> |
| **Google Drive** resumable | ❌ **sequential** | Chunks must be multiples of 256 KiB and sent in order to one session URI. You could create multiple separate files, but that isn't bonding. |
| **YouTube** resumable | ❌ sequential (same protocol as Drive) | |
| **Dropbox** upload sessions | ✅ with `session_type: "concurrent"` in `upload_session/start` | For concurrent sessions, `start` and `finish` must carry **no data**. Data goes through `upload_session/append_v2` with an explicit offset, in any order. Each append ≤ 150 MiB, and chunks must be a **multiple of 4 MiB (4,194,304 B) except the last** **(verify in the official docs)**. Max session size 2^41−2^22 B (~2 TiB, current docs say up to 350 GB per file **(verify)**). Sessions expire after **7 days**. <https://www.dropbox.com/developers/documentation/http/documentation#files-upload_session-start> |
| OneDrive / Graph upload sessions | ❌ sequential byte ranges (fragments must be in order, multiples of 320 KiB) | |

**Client:** the Rust core requests a batch of presigned `UploadPart` URLs from the backend (e.g. 50 at a time), then each per-interface `reqwest::Client` PUTs parts from a shared queue. Use the same work-stealing as downloads: since R2 parts are fixed-size, slow links simply take fewer parts. Retry the same part number on any interface. Choose a part size so `ceil(size/part) ≤ 10,000`. Default 16 MiB (≤160 GiB), and scale up for larger files (e.g. `max(16 MiB, ceil(size/9000))` rounded to a MiB).

Crates: `aws-sdk-s3` 1.152.0 (heavy; use it on the backend or for presigning) or **`rusty-s3` 0.10.2** (tiny, sans-IO presigner; good for the client if the client ever signs). `object_store` 0.14.2 is also an option. The client should **not** hold credentials: the backend presigns.

### 4.2 Backend

**Recommended: Cloudflare Workers + R2 + D1** (wrangler 4.148.0, Hono 4.13.13 as the router).
- R2 egress is **$0**. Storage $0.015/GB-month. Class A ops (writes, incl. UploadPart) $4.50/million, Class B $0.36/million. Free tier 10 GB storage, 1M Class A, 10M Class B per month. <https://developers.cloudflare.com/r2/pricing/>
- Flow: `POST /uploads` (auth, quota) → the Worker calls `CreateMultipartUpload` through the S3 API (or the R2 binding), stores the row in D1 (`id, key, upload_id, size, part_size, expires_at, owner, enc_header`) → `POST /uploads/:id/parts?from=…&n=…` returns presigned URLs (15–60 min TTL) → the client uploads → `POST /uploads/:id/complete` with ETags (the Worker calls `CompleteMultipartUpload`) → returns a share link `https://share.example/<slug>` (slug ≥ 128-bit random, base62).
- **Download:** `GET /s/:slug` serves a landing page. The file is served either by redirecting to a short-lived presigned GET (cheapest, no Worker CPU per byte) or by streaming through the Worker (`env.BUCKET.get(key, {range})`). Range support is free, which means **receivers can also use the bonded downloader**.
- **Expiry:** store `expires_at` in D1. A Cron Trigger (every hour) deletes expired objects and rows. Also add an R2 lifecycle rule (object age, e.g. 30 days) as a safety net, plus `AbortIncompleteMultipartUpload` after 1–2 days. Optional download-count limits (D1 counter, atomic `UPDATE … SET n=n+1 WHERE n<max RETURNING`).
- **Abuse:** a share host attracts malware/CSAM. You need ToS, a report endpoint, rate limits (Workers rate-limiting binding), Turnstile on anonymous creates, size caps per tier, and the right to scan unencrypted uploads. E2E encryption limits scanning, so decide your policy (Firefox Send was shut down over abuse).
- **CORS:** R2 bucket CORS must allow `PUT` from your app's origins and expose `ETag` (`ExposeHeaders: ["ETag"]`), or the browser/webview can't read part ETags. Native Rust uploads don't need CORS.

Alternatives: AWS S3 + Lambda + DynamoDB (egress ~$0.09/GB, a big cost for a sharing product), Backblaze B2 + Fly.io/Hetzner (cheap, B2 egress free through Cloudflare), Supabase Storage (tus support built in), MinIO self-hosted.

### 4.3 Optional client-side E2E encryption, compatible with parallel parts

- Don't use one streaming `age` file. `age` (crate 0.12.1) uses STREAM with 64 KiB chunks and *sequential* nonces, which is technically seekable/parallelizable (the nonce is a counter), but the `age` crate's API is a streaming writer. Rolling your own on the same construction is simpler.
- **Scheme (like Firefox Send's ECE / Wormhole):** random 256-bit file key `K`, an HKDF-SHA256 salt, a payload key `PK = HKDF(K, salt, "payload")`. Split the plaintext into **fixed 64 KiB–1 MiB records**. Record `i` = `XChaCha20-Poly1305` or `AES-256-GCM` under `PK` with nonce = `prefix(11 bytes) || be32(i) || last_flag` (the STREAM construction, Hoang–Reyhanitabar–Rogaway–Vizár). Ciphertext size is deterministic (`plaintext + 16 B tag per record`), so you can compute each ciphertext part's byte range up front and encrypt each part independently, in parallel, on any interface. Make the multipart part size a multiple of the record size. Crates: `chacha20poly1305` 0.11.0 / `aes-gcm` 0.11.1 (RustCrypto), or `aws-lc-rs` AEAD (faster AES-GCM with AES-NI/ARMv8).
- Put a small header (version, salt, record size, the encrypted metadata with filename/MIME) as object metadata, or as record 0 of the object.
- The key travels in the URL **fragment** (`https://share.example/s/<slug>#k=<base64url K>`), so it never reaches the server. Browser receivers decrypt with WebCrypto (AES-GCM is native; XChaCha isn't, so **use AES-256-GCM** for browser compatibility). Stream-decrypt with a Service Worker + `ReadableStream`, or the File System Access API on Chromium, so large files don't need RAM. Optional password: wrap K with Argon2id.
- Because ranges map to whole records, the receiver's bonded downloader can also decrypt per segment.

---

## 5. Browser extension

### 5.1 Framework and manifest

- **WXT 0.21.4** (Vite-based, one codebase producing MV3 for Chrome/Edge/Brave/Opera and MV3 for Firefox; Safari through the converter). Plasmo 0.90.5 is in maintenance-ish mode, so WXT is preferred.
- **Chrome/Edge/Brave:** MV3 with a service-worker background. Permissions: `downloads`, `cookies`, `nativeMessaging`, `contextMenus`, `storage`, optionally `webRequest` (observational only in MV3) + `host_permissions: ["<all_urls>"]` (needed for cookies of arbitrary sites; make it optional and request at runtime to ease store review).
- **Firefox:** MV3 uses `background.scripts` (event page, **not** service worker), `browser_specific_settings.gecko.id` is required, and blocking `webRequest` is still available. **`downloads.onDeterminingFilename` doesn't exist in Firefox.**

### 5.2 Capturing downloads

- `chrome.downloads.onCreated(item)` gives `url`, `finalUrl`, `referrer`, `mime`, `fileSize`, `filename` (often empty at this point). To take over: `chrome.downloads.cancel(item.id)` then `chrome.downloads.erase({id})` to remove it from the shelf. Race: Chrome may already have written a bit. That's fine.
- `chrome.downloads.onDeterminingFilename(item, suggest)` (Chrome only) fires once the filename/MIME is known, which is better for filtering by extension/size before cancelling. You must call `suggest()` or return `true` for async.
- Filtering: skip `blob:`/`data:` URLs (the browser must handle those, since the app can't fetch them; or read them through a content script and stream), skip small files (<1 MB configurable), and respect a "hold Alt to bypass" modifier (detected through a content-script click listener, as IDM/FDM do).
- Context menu: "Download with <App>" on links/media (`contexts: ["link","video","audio"]`).
- **Auth for the app's own re-fetch:**
  - Cookies: `chrome.cookies.getAll({url})`. Include **partitioned cookies (CHIPS)** with `getAll({url, partitionKey: {topLevelSite}})` in Chrome 119+ when the download is in a third-party frame. Build a `Cookie:` header.
  - Headers: `User-Agent` (`navigator.userAgent`), `Referer` (`item.referrer`), `Accept-Language`. Capture per-request headers (Authorization, custom tokens) with `chrome.webRequest.onSendHeaders` + `["requestHeaders","extraHeaders"]` keyed by URL/requestId. Keep it in a short-lived in-memory map (service workers die, so use `chrome.storage.session`).
  - POST-initiated downloads, single-use tokens and IP-bound signed URLs can't be replayed reliably, especially over **a different interface/IP**. Detect a 403 on the first ranged probe and fall back to letting the browser download, or download on the primary interface only.
  - Many CDNs bind signed URLs to the client IP. Bonding across interfaces breaks those, so make "probe each interface with a Range GET; drop interfaces that get 403/redirect" part of the scheduler.

### 5.3 Talking to the app

**A) Native messaging (primary):** `chrome.runtime.connectNative("app.fuselane.host")`. The host is a tiny binary (the CLI with `--native-messaging`) that relays over stdin/stdout (32-bit length-prefixed JSON, max 1 MB host→browser, 64 MiB browser→host) to the daemon's local IPC socket. Host manifest:
```json
{ "name": "app.fuselane.host", "description": "…", "path": "/abs/path/fuselane-nm",
  "type": "stdio", "allowed_origins": ["chrome-extension://<ID>/"] }   // Firefox: "allowed_extensions": ["extension@fuselane.app"]
```
Manifest locations (per-user; the installer writes them):

| Browser | macOS | Linux | Windows (registry key → default value = manifest path) |
|---|---|---|---|
| Chrome | `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/` | `~/.config/google-chrome/NativeMessagingHosts/` | `HKCU\Software\Google\Chrome\NativeMessagingHosts\app.fuselane.host` |
| Chromium | `~/Library/Application Support/Chromium/NativeMessagingHosts/` | `~/.config/chromium/NativeMessagingHosts/` | (uses the Chrome key) |
| Edge | `~/Library/Application Support/Microsoft Edge/NativeMessagingHosts/` | `~/.config/microsoft-edge/NativeMessagingHosts/` | `HKCU\Software\Microsoft\Edge\NativeMessagingHosts\…` (Edge also reads the Chrome key) |
| Brave | `~/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts/` | `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/` | uses the Chrome key |
| Firefox | `~/Library/Application Support/Mozilla/NativeMessagingHosts/` | `~/.mozilla/native-messaging-hosts/` | `HKCU\Software\Mozilla\NativeMessagingHosts\…` |

System-wide variants: macOS `/Library/Google/Chrome/NativeMessagingHosts`; Linux `/etc/opt/chrome/native-messaging-hosts`, `/usr/lib/mozilla/native-messaging-hosts`; Windows HKLM. **Flatpak/Snap browsers can't spawn host binaries** without portal workarounds (a known pain point), so fall back to B. Docs: <https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging>, <https://developer.mozilla.org/docs/Mozilla/Add-ons/WebExtensions/Native_manifests>.

**B) Localhost WebSocket/HTTP (fallback, and for Safari/Flatpak):** the daemon listens on `127.0.0.1:<port>` (try a fixed port range, never `0.0.0.0`). Pairing: the app shows a 6-digit code or QR, the extension posts it, and the daemon returns a long random token stored in `chrome.storage.local`. Every request carries `Authorization: Bearer`. The daemon checks the `Origin` header against an allow-list of the extension IDs (`chrome-extension://<id>`, `moz-extension://<uuid>`; the Firefox UUID is random per install, so rely on the token there). Reject requests whose `Origin` is a website (CSRF/DNS-rebinding defence) and check `Host` is `127.0.0.1:port`. Chrome's **Local Network Access** permission prompts apply to *web pages* calling localhost. Extension contexts with host permissions are not affected **(verify on current Chrome)**.

**C) Safari:** `xcrun safari-web-extension-converter` produces an Xcode app-extension project that must be embedded in a **macOS .app** (it can be a small helper app inside your Tauri bundle, or a separate App Store app). It talks to the containing app through `browser.runtime.sendNativeMessage` → `SFSafariExtensionHandler`/`NSExtensionRequestHandling` in Swift, which forwards to your daemon through XPC/UDS. Safari's `downloads` API support is limited **(verify)**, so capture through link-click interception in content scripts plus a context menu. Distribution needs signing; the App Store is optional for Developer ID-signed Safari extensions (users must enable them in Safari settings).

### 5.4 Security

- Treat every message from the extension as untrusted: validate URL schemes (`http`/`https`/`magnet` only), don't let the extension choose arbitrary local paths (only the app's configured download directory + a sanitized filename, so no `..`, reserved Windows names or ADS `:`), and cap header sizes.
- Cookies are sensitive: hold them in memory for the job lifetime only and never log them. Don't persist them unless the user enables resume-after-restart (then encrypt with the OS keychain via the `keyring` crate).
- Native-messaging host: `allowed_origins` restricts it to your extension ID. Sign the host binary.
- The localhost server must require the token, check Origin, rate-limit pairing attempts, and expire pairing codes after 2 min.
- Mark downloaded files with quarantine/MOTW: macOS `com.apple.quarantine` xattr; Windows `Zone.Identifier` ADS (`ZoneId=3`, `HostUrl=`, `ReferrerUrl=`). Browsers do this, and an app that bypasses them weakens Gatekeeper/SmartScreen. IDM and others are criticized for skipping it.
- Store review: `<all_urls>` + `cookies` + `nativeMessaging` triggers a manual review on the Chrome Web Store and AMO. Write a clear justification and a privacy policy.

### 5.5 How others do it

- **IDM:** the "IDM Integration Module" extension + a native messaging host (`com.tonec.idm`, registered by the IDM installer). It captures via `downloads.onCreated`/`webRequest` and sends URL+cookies+referrer to IDM **(verify host name)**.
- **FDM:** the "Free Download Manager" extension + a native messaging host installed by FDM (`org.freedownloadmanager.fdmchromeextension`-style) **(verify)**. It passes cookies/headers.
- **Motrix:** no first-party extension. Users pair "Aria2 Explorer"/"Aria2 for Chrome" extensions with Motrix's aria2 **JSON-RPC on localhost:16800 + secret token**. It's simple, but because the RPC also listens for any local process, the token is essential.
- **Gopeed:** official extension (Chrome/Edge/Firefox) talking to Gopeed's **localhost REST API** (configurable port + API token). It forwards cookies/headers/referer.
- **Takeaway:** native messaging is the most robust and needs no ports. A localhost API with a token is easier and works for Safari and sandboxed browsers. Ship both.

---

## 6. Code signing, packaging, updates, CI

### 6.1 macOS
- **Apple Developer Program** $99/yr → a "Developer ID Application" certificate (+ "Developer ID Installer" for .pkg).
- Tauri signs when the env has `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD` and `APPLE_SIGNING_IDENTITY`, and notarizes with either `APPLE_API_KEY`/`APPLE_API_ISSUER`/`APPLE_API_KEY_PATH` (an App Store Connect API key, preferred) or `APPLE_ID`/`APPLE_PASSWORD` (app-specific)/`APPLE_TEAM_ID`. It runs `xcrun notarytool submit --wait` and staples. Hardened Runtime is on, so it needs entitlements for e.g. `com.apple.security.cs.allow-jit` (WKWebView doesn't need it in the host process).
- Sign the sidecar/CLI and the native-messaging host too. Notarize the standalone CLI `.zip`/`.pkg` separately if distributed via Homebrew (`brew install --cask` for the app, a formula/tap for the CLI).
- Universal binary: `--target universal-apple-darwin` (needs both rustup targets).

### 6.2 Windows
- **Microsoft Artifact Signing** (the renamed **Azure Trusted Signing**; the pricing URL `azure.microsoft.com/pricing/details/artifact-signing/` now redirects from `/trusted-signing/`). Two tiers: **Basic** (5,000 signatures/month, 1 cert profile of each type) and **Premium** (100,000 signatures/month, 10 profiles), with per-signature overage. Prices were **$9.99/month Basic and $99.99/month Premium** when it was Trusted Signing; the current pricing page didn't render numbers, so **verify**. Billing starts when the account is created and isn't prorated. Eligibility: organizations (with 3+ years of verifiable history in some regions) and individual developers in the US/Canada **(verify current eligibility)**. It gives instant SmartScreen reputation comparable to EV in practice (the certs are short-lived, 3 days, timestamped).
  - CI: `azure/trusted-signing-action` (GitHub Action) or `signtool` + `Azure.CodeSigning.Dlib`. In Tauri, set `bundle.windows.signCommand` to `trusted-signing-cli -e <endpoint> -a <account> -c <profile> %1` (crate `trusted-signing-cli`) so each binary *and* the installer gets signed.
- **SignPath.io:** free for OSS through the **SignPath Foundation** (the certificate is issued to the Foundation; requires an OSI licence, builds from a public repo on GitHub Actions, and their review). Paid tiers for companies.
- Alternatives: an EV/OV cert on a cloud HSM (DigiCert KeyLocker, SSL.com eSigner), roughly $300–600/yr. Since 2023 all OV/EV keys must be on an HSM, which is why cloud signing is standard.
- Installers: NSIS (`-setup.exe`, recommended, supports per-user install without admin) and/or MSI (WiX). Also publish to **winget** (`winget-releaser` action).

### 6.3 Linux
- Tauri bundles **AppImage** (the auto-updater works), **.deb**, **.rpm**. Gotcha: AppImage bundles WebKitGTK only partially, so build on the **oldest supported distro** (Ubuntu 22.04 runner) for glibc compatibility.
- **Flatpak:** not produced by the Tauri bundler. Write a manifest (`org.gnome.Platform` runtime has WebKitGTK) and publish on Flathub. Sandbox notes: for interface enumeration and binding inside Flatpak, `--share=network` gives the host network namespace, so `SO_BINDTODEVICE` works (unprivileged, kernel ≥5.7). The native-messaging host from Flatpak browsers is problematic (5.3). Downloads directory: `--filesystem=xdg-download`.
- Snap: optional. `network-observe`/`network-control` interfaces need manual connection.
- Sign packages: deb repo with GPG (`apt` repo on Cloudflare R2/Pages), rpm `rpmsign`. AppImage can carry an embedded signature (`--sign`), but users rarely verify it.

### 6.4 Tauri updater
- `tauri signer generate -w ~/.tauri/fuselane.key` produces a minisign keypair. The public key goes in `tauri.conf.json > plugins.updater.pubkey`. CI signs artifacts and produces `.sig` files. `latest.json` lists per-platform URLs + signatures (`darwin-aarch64`, `darwin-x86_64`, `windows-x86_64`, `linux-x86_64`). **Losing the private key means you can't update existing installs**, so back it up in a password manager/HSM.
- `tauri-apps/tauri-action` builds, uploads to a GitHub Release and writes `latest.json`.
- The OS signature (Apple/Windows) and the updater signature are independent. You need both.

### 6.5 GitHub Actions matrix
```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - { os: macos-15,       target: aarch64-apple-darwin }     # or macos-26 (macos-latest moved to 26 in mid-2026)
      - { os: macos-15-intel, target: x86_64-apple-darwin }      # Intel images are being phased out (Apple Silicon only from ~Fall 2027) → or cross-compile x86_64 on arm64 / build universal
      - { os: windows-latest, target: x86_64-pc-windows-msvc }
      - { os: windows-11-arm, target: aarch64-pc-windows-msvc }   # optional
      - { os: ubuntu-22.04,   target: x86_64-unknown-linux-gnu }  # oldest glibc
      - { os: ubuntu-22.04-arm, target: aarch64-unknown-linux-gnu } # optional
```
- Linux deps: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libssl-dev`.
- Reproducibility: pin `rust-toolchain.toml` (exact version), `Cargo.lock` committed, `cargo build --locked`, `pnpm install --frozen-lockfile` with `packageManager` pinned in package.json (corepack), actions pinned by SHA, `Swatinem/rust-cache`, `SOURCE_DATE_EPOCH` and `--remap-path-prefix` for deterministic paths. Signing/notarization happen only in a separate release job with an `environment:` protection rule holding the secrets.
- Supply chain: `cargo-deny` (licences/advisories), `cargo-auditable`, `actions/attest-build-provenance` (SLSA provenance on release assets).

---

## 7. Android (and iOS) later

### 7.1 Android
- **Using cellular while on Wi-Fi:**
  ```kotlin
  val cm = getSystemService(ConnectivityManager::class.java)
  val req = NetworkRequest.Builder()
      .addTransportType(NetworkCapabilities.TRANSPORT_CELLULAR)
      .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
      .build()
  cm.requestNetwork(req, object : ConnectivityManager.NetworkCallback() {
      override fun onAvailable(network: Network) { core.addNetwork(network.networkHandle, "cellular") }
      override fun onLost(network: Network) { core.removeNetwork(network.networkHandle) }
  })
  ```
  This needs `CHANGE_NETWORK_STATE` + `ACCESS_NETWORK_STATE`. The system keeps the cellular radio up while the request is held, so release it when idle. Wi-Fi: `TRANSPORT_WIFI`, plus `registerDefaultNetworkCallback`. Ethernet/USB tethering: `TRANSPORT_ETHERNET`/`TRANSPORT_USB`.
- **Binding sockets:** in Kotlin, `network.bindSocket(socket)` / `network.bindSocket(fd: FileDescriptor)` / `network.openConnection(url)` / `network.socketFactory`. **From Rust (best):** the NDK API `android_setsocknetwork(net_handle_t, fd)` (API 23+) in `<android/multinetwork.h>`, applied to the raw fd from socket2 before `connect`. Pass the `network.networkHandle` (a `long`) to Rust over uniffi. Also `android_getaddrinfofornetwork(net_handle, …)` for per-network DNS. `SO_BINDTODEVICE` also works on Android, but `android_setsocknetwork` is the supported route (it also sets the right DNS/routing marks). `ndk-sys` exposes the multinetwork functions **(verify)**, or use a 5-line `extern "C"` block linking `libandroid.so`.
- **Rust core through uniffi 0.32.2:** proc-macro (`#[uniffi::export]`), async functions map to Kotlin coroutines, callback interfaces for progress/events. Build with `cargo-ndk` (targets `aarch64-linux-android`, `x86_64-linux-android`, optionally `armv7`) and generate Kotlin bindings with `uniffi-bindgen`. Package as an AAR. Use `rustls-platform-verifier`'s Android component (it needs the JNI init call, via `jni` 0.22) for system CA trust.
- **Background:** use a **foreground service** of type `dataSync` (Android 14+ requires a foreground-service type and the `FOREGROUND_SERVICE_DATA_SYNC` permission; Android 15 caps `dataSync` at **6 h per 24 h** **(verify)**). Alternatively, user-initiated data transfer jobs (`JobInfo.Builder#setUserInitiated(true)`, API 34+), which are meant for exactly this and have no fixed time cap. Respect Data Saver (`ConnectivityManager.getRestrictBackgroundStatus`) and metered networks (`NET_CAPABILITY_NOT_METERED`).

### 7.2 iOS (if ever)
- **Network.framework:** `NWParameters.requiredInterfaceType = .cellular` / `.wifi` / `.wiredEthernet`, `prohibitedInterfaceTypes`, `requiredInterface` (a specific `NWInterface` from `NWPathMonitor`). One `NWConnection` per interface. Multi-interface is possible (Apple supports it, e.g. for Wi-Fi Assist), and `multipathServiceType` (MPTCP) helps only with MPTCP servers. You would write HTTP/1.1 range requests over `NWConnection` + TLS yourself (`URLSession` can't pin to an interface, only `allowsCellularAccess`/`allowsExpensiveNetworkAccess` booleans).
- **Background:** `URLSession` background configuration hands transfers to `nsurlsessiond`. It gives no interface control and is opportunistically scheduled and rate-limited (it relaunches the app on completion; discretionary transfers can be delayed). Bonded transfers only run while the app is foreground or in a short `beginBackgroundTask` (~30 s). iOS 26 adds `BGContinuedProcessingTask` for user-initiated long tasks with progress UI **(verify applicability to network)**. Conclusion: iOS bonding is foreground-only.
- Rust on iOS: uniffi Swift bindings + an XCFramework. `IP_BOUND_IF` works on iOS BSD sockets too, but Apple recommends Network.framework (BSD sockets don't trigger the cellular radio wake-up/VPN-on-demand).

---

## 8. Testing

| Tool | Version | Use |
|---|---|---|
| **cargo-nextest** | 0.9.146 | Default test runner (process-per-test, retries, JUnit for CI, test groups for netns tests that need serialization). |
| **proptest** | 1.11.0 | Segment scheduler invariants (ranges cover [0,len) exactly, no overlap; resume bitmap round-trip; part-size calc ≤10,000 parts; encryption record/offset math). |
| **turmoil** | 0.7.2 | Deterministic simulated network for tokio (hosts, latency, partitions, message loss). Good for scheduler/retry logic. Abstract your "dialer per interface" trait so tests can inject turmoil `TcpStream`s. It doesn't simulate interfaces/bandwidth precisely; use it for logic, not throughput. |
| **Linux netns + veth + tc netem** | iproute2 | The real end-to-end multi-interface test on GitHub `ubuntu-*` runners (sudo is available). |
| **toxiproxy** | Shopify toxiproxy 2.x server (Rust client crates are stale; use its HTTP API directly) | Cross-platform latency/bandwidth/reset/timeout "toxics" between the client and a local server. Simulates per-interface behavior when combined with binding to different loopback aliases (127.0.0.2/3 on Linux/macOS via `ifconfig lo0 alias`). |
| `wiremock` / `axum` test server | axum 0.8.9 | Range server with failure injection (ignore Range → 200, ETag change mid-download, 403 on some IP, slow trickle). |
| **Playwright** | 1.64.0 | Extension E2E (Chromium). Frontend component tests in plain browsers (mock the Tauri `invoke` with `@tauri-apps/api/mocks` `mockIPC`). |
| **tauri-driver** | 2.1.0 | WebDriver for the real app: Linux (WebKitWebDriver) and Windows (msedgedriver). **No macOS support** (WKWebView has no WebDriver). Use WebdriverIO or Selenium. Playwright can't drive WebKitGTK/WKWebView inside Tauri; on Windows it can attach to WebView2 over CDP (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` + `chromium.connectOverCDP`). |

**netns recipe (CI):**
```bash
# Server namespace with two "ISPs"; client sees two interfaces with different shapes
sudo ip netns add srv
for i in 1 2; do
  sudo ip link add c$i type veth peer name s$i
  sudo ip link set s$i netns srv
  sudo ip addr add 10.$i.0.2/24 dev c$i && sudo ip link set c$i up
  sudo ip netns exec srv ip addr add 10.$i.0.1/24 dev s$i
  sudo ip netns exec srv ip link set s$i up
done
sudo ip netns exec srv ip link set lo up
# Shape: link1 = 50 Mbit / 20 ms; link2 = 10 Mbit / 80 ms / 1% loss
sudo tc qdisc add dev c1 root netem rate 50mbit delay 20ms
sudo tc qdisc add dev c2 root netem rate 10mbit delay 80ms loss 1%
sudo ip netns exec srv tc qdisc add dev s1 root netem rate 50mbit delay 20ms
sudo ip netns exec srv tc qdisc add dev s2 root netem rate 10mbit delay 80ms loss 1%
# Serve the same file on both addresses
sudo ip netns exec srv ./test-range-server --bind 10.1.0.1:8080 --bind 10.2.0.1:8080 &
# Assert aggregate throughput ≈ 55–60 Mbit and correct SHA-256; then `ip link set c2 down` mid-transfer to test failover
```
Put the *client* in its own netns as well for full isolation (then SO_BINDTODEVICE to c1/c2 is exercised realistically). Netem on egress only shapes one direction, so shape both sides (as above) or use an `ifb` device for ingress. For a "same hostname resolves differently per interface" test, run a small DNS server (hickory-server) per link.

For Windows/macOS CI, use loopback aliases + toxiproxy for logic, and test real NIC pinning manually or on self-hosted runners with USB Ethernet/phone tethering.

**Extension testing with Playwright:** `chromium.launchPersistentContext(userDataDir, { channel: 'chromium', args: ['--disable-extensions-except=dist/chrome-mv3', '--load-extension=dist/chrome-mv3'] })`. New headless mode supports extensions. Get the service worker with `context.waitForEvent('serviceworker')`. Trigger downloads on a test page and assert that the native host or WebSocket mock received the URL, cookies and referrer. Native messaging in tests: install a test host manifest into the temporary profile's `NativeMessagingHosts` dir. Firefox: `web-ext run`/`web-ext lint` (Playwright's Firefox can't load extensions normally).

---

## 9. Monorepo layout

```
fuselane/
├─ Cargo.toml                 # [workspace] resolver = "3", workspace.dependencies pinned
├─ rust-toolchain.toml
├─ pnpm-workspace.yaml        # packages: ["apps/*", "packages/*"]
├─ package.json               # packageManager: pnpm@<pinned>; turbo or just pnpm -r scripts
├─ crates/
│  ├─ netif/                  # interface enumeration, friendly names, change watcher, per-OS pinning (socket2 + windows-sys + SC/NW FFI), per-interface DNS
│  ├─ transport/              # per-interface connectors → hyper/reqwest clients, probes, throughput estimator, QUIC later
│  ├─ engine-http/            # segmented range downloader, scheduler/work stealing, resume, disk writer
│  ├─ engine-torrent/         # librqbit integration + SOCKS5 per-interface dialer
│  ├─ engine-upload/          # multipart uploader (S3/R2/B2/tus/Dropbox backends), E2E encryption (crypto submodule)
│  ├─ core/                   # job manager, persistence (redb/rusqlite), event bus, settings; no UI deps
│  ├─ ipc/                    # daemon API (UDS/named pipe, JSON-RPC or tarpc), pairing tokens, localhost WS for the extension
│  ├─ ffi/                    # uniffi bindings (Android/iOS)
│  └─ testkit/                # range server, netns helpers, turmoil adapters
├─ apps/
│  ├─ desktop/                # Tauri 2: src-tauri/ (thin: commands → core) + src/ (React 19 + Vite + Tailwind 4)
│  ├─ cli/                    # `fuselane` CLI + `fuselaned` daemon (+ `--native-messaging` host mode); clap
│  ├─ extension/              # WXT (chrome-mv3, firefox-mv3); safari/ Xcode wrapper generated
│  ├─ backend/                # Cloudflare Worker (Hono) + D1 migrations + wrangler.toml
│  └─ android/                # Gradle project consuming crates/ffi AAR (later)
├─ packages/
│  ├─ ui/                     # shared React components (desktop + extension popup + share page)
│  ├─ api-types/              # TS types generated from Rust (specta/ts-rs) + backend zod schemas
│  └─ crypto-web/             # WebCrypto decryptor for share pages (mirrors Rust format; test vectors shared)
└─ .github/workflows/         # ci.yml (fmt, clippy, nextest, netns e2e, vitest, playwright), release.yml (matrix, sign, notarize, updater json)
```
Principles: `core` and the engines are pure libraries (no Tauri dependency) so the CLI, daemon, desktop and Android all share them. The desktop app uses `core` in-process and also connects to `fuselaned` if one is running (single owner of jobs, so no two engines fight over the same files). Shared test vectors (JSON) for the encryption format are used by both Rust and TS tests.

---

## 10. Final recommended stack

| Area | Choice | Version (2026-10-08) | Notes |
|---|---|---|---|
| Desktop shell | Tauri 2 | 2.12.1 (wry 0.57) | Plugins: updater 2.13.2, single-instance 2.5.2, deep-link 2.6.1, notification 2.5.1, autostart 2.7.0, dialog 2.8.1, fs 2.6.0, opener 2.7.0, process 2.4.0, log 2.10.0, store 2.5.0 |
| UI | React + Vite + Tailwind | 19.3.0 / 8.3.3 / 4.3.3 | Min macOS 13.3 (Safari 16.4 features). TanStack Query/Virtual, Zustand, tauri-specta |
| Async runtime | tokio | 1.53.2 | |
| HTTP | hyper + hyper-util (custom connector), or reqwest per interface | 1.12.0 / 0.1.21 / reqwest 0.13.5 | One client per interface, HTTP/1.1 multi-connection for segments, `.no_proxy()` |
| TLS | rustls (+ rustls-platform-verifier) | 0.23.45 | reqwest 0.13 default is aws-lc-rs |
| Sockets | socket2 (`all` feature) + windows-sys | 0.6.5 / 0.61.2 | Windows `IP_UNICAST_IF` done by hand |
| QUIC (later) | quinn + h3 | 0.11.12 | Custom pinned UDP socket per interface |
| Interfaces | netdev + if-watch/netwatcher; rtnetlink (Linux), system-configuration (macOS), windows (Win) | 0.46.3 / 3.2.2 / 0.8.0 / 0.23.0 / 0.8.0 / 0.62.2 | Friendly names via SC / GetAdaptersAddresses / sysfs + NM |
| DNS | hickory-resolver with custom RuntimeProvider; DNSServiceGetAddrInfo on macOS | 0.26.3 | Per-interface DNS servers |
| Disk | fs4 (allocate/locks) + pwrite/seek_write, FSCTL_SET_SPARSE on Windows | 1.1.0 | fsync per segment batch, rename on completion |
| Job DB | redb (or rusqlite) | 4.3.0 / 0.40.2 | |
| Torrent | librqbit + in-process SOCKS5 per-interface dialer; upstream a connector hook. Plan B: libtorrent 2.1.2 via cxx | 9.0.1 | librqbit `bind_device_name` is single-interface and not on Windows |
| Upload client | presigned UploadPart via backend; rusty-s3 if signing locally | rusty-s3 0.10.2 | Fixed part size (R2 rule), ≤10k parts |
| Encryption | Chunked AES-256-GCM STREAM (aws-lc-rs or aes-gcm) | aes-gcm 0.11.1 | Key in URL fragment; WebCrypto decrypt |
| Backend | Cloudflare Workers + R2 + D1, Hono, Cron Triggers | wrangler 4.148.0, hono 4.13.13 | $0 egress; lifecycle + abort-incomplete rules |
| Extension | WXT, MV3 (Chrome/Edge/Brave/Firefox), Safari via converter | 0.21.4 | Native messaging + localhost WS with pairing token |
| Mobile bridge | uniffi + cargo-ndk; android_setsocknetwork | 0.32.2 | Foreground service `dataSync` / user-initiated jobs |
| Signing | Apple Developer ID + notarytool; MS Artifact Signing (or SignPath Foundation for OSS); minisign for the updater | – | Separate release job with protected secrets |
| Packaging | DMG/.app (universal), NSIS + winget, AppImage + deb + rpm, Flatpak (manual manifest) | – | Build Linux on Ubuntu 22.04 |
| Testing | cargo-nextest, proptest, turmoil, netns + tc netem, toxiproxy, Playwright, tauri-driver | 0.9.146 / 1.11.0 / 0.7.2 / – / – / 1.64.0 / 2.1.0 | |

### Top risks to prototype first
1. **Windows interface pinning** (IP_UNICAST_IF byte order, virtual adapter filtering) and macOS per-interface DNS. Build a 200-line spike: download one file split across Wi-Fi + phone tether on all three OSes.
2. **IP-bound signed URLs/cookies** breaking multi-interface downloads. The probe-and-drop strategy must be in v1.
3. **librqbit per-peer interface control.** Validate the SOCKS5 load-balancer approach with a real swarm.
4. **WebKitGTK rendering** on NVIDIA/Wayland. Keep the UI light, and ship the documented env-var fallbacks behind a setting.
5. **Share-link abuse/legal** posture before launch.

## Sources
- crates.io API (versions), npm registry, GitHub releases API: queried 2026-10-08
- socket2 0.6.5 source `src/sys/unix.rs` (`bind_device` L1968, `bind_device_by_index_v4` L2023), `src/sys/windows.rs`
- reqwest 0.13.5 source `src/async_impl/client.rs` (`interface()` cfg L1750–1762, `connector_layer` L2475), `Cargo.toml` (default-tls = rustls)
- hyper-util 0.1.21 `src/client/legacy/connect/http.rs` (`set_interface` L413)
- hickory-resolver 0.26.3 `src/config.rs` L350 (`bind_addr`), `RuntimeProvider::connect_tcp/bind_udp`
- librqbit 9.0.1 `src/session.rs` L421–443, `src/stream_connect.rs`; librqbit-dualstack-sockets 0.7.0 `src/bind_device.rs`
- Tauri Linux graphics: https://v2.tauri.app/develop/debug/linux-graphics/
- Tauri runtime-wry releases: https://v2.tauri.app/release/tauri-runtime-wry/ ; cef-rs: https://github.com/tauri-apps/cef-rs
- R2 multipart: https://developers.cloudflare.com/r2/objects/upload-objects/ ; R2 pricing: https://developers.cloudflare.com/r2/pricing/
- S3 limits: https://docs.aws.amazon.com/AmazonS3/latest/userguide/qfacts.html
- Dropbox upload sessions: https://www.dropbox.com/developers/documentation/http/documentation#files-upload_session-start ; https://dropboxforum.com/discussions/101000014/system-argumentnullexception-when-inititating-concurrent-upload/477340
- tus concatenation: https://tus.io/protocols/resumable-upload#concatenation
- Artifact Signing pricing: https://azure.microsoft.com/pricing/details/artifact-signing/
- Winsock IP options: https://learn.microsoft.com/windows/win32/winsock/ipproto-ip-socket-options
- libtorrent settings: https://www.libtorrent.org/reference-Settings.html#outgoing_interfaces
- Native messaging: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging ; https://developer.mozilla.org/docs/Mozilla/Add-ons/WebExtensions/Native_manifests
- GitHub macOS runners (secondary): https://depot.dev/changelog/2026-08-10-macos-26-default-github-actions ; https://tenki.cloud/blog/github-actions-runner-image-selection-2026
- Android multinetwork: https://developer.android.com/ndk/reference/group/networking ; ConnectivityManager.requestNetwork docs
