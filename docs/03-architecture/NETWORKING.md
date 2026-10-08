# Networking: discovering, pinning, resolving, probing

Crates: `netif` (discover + watch) and `transport` (pin + resolve + connect + probe). This is the riskiest part of Fuselane, so it is prototyped first (Phase 1 spikes S1–S3).

## 1. Discovering interfaces (`netif`)

| OS | Source | Friendly name | Kind |
|---|---|---|---|
| macOS | `netdev` + `SCNetworkInterfaceCopyAll` (SystemConfiguration) | `SCNetworkInterfaceGetLocalizedDisplayName` ("Wi-Fi", "iPhone USB") | `SCNetworkInterfaceGetInterfaceType` (IEEE80211, Ethernet, WWAN, Bluetooth); "iPhone USB" → tether |
| Windows | `GetAdaptersAddresses` | `FriendlyName` ("Wi-Fi", "Ethernet 2") | `IfType` (71 Wi-Fi, 6 Ethernet, 243/244 WWAN), `Description`, `TunnelType`, `ConnectionType` |
| Linux | `netdev` + sysfs (`/sys/class/net/<if>/{type,uevent,wireless,device/driver}`) + NetworkManager D-Bus when present | NM connection `Id`, else the device name | `DEVTYPE=wlan/wwan/bridge/gadget`, driver `rndis_host`/`cdc_ether`/`ipheth` → tether |

**Filter (L-60):** drop loopback, link-local only (169.254/16, fe80::/10), interfaces with no gateway, and **virtual adapters by default**: Hyper-V vEthernet, WSL, VMware, VirtualBox, Docker bridges, TAP/TUN, utun. VPN tunnels are shown in an "Other" section, off by default.

**Stable id:** derived from the MAC address or adapter GUID, so renames and colours survive reboots and device renumbering. Plexo used the device name, which changes (en5 → en6).

**Change events (L-63):**

| OS | Mechanism |
|---|---|
| Linux | rtnetlink subscription (`RTMGRP_LINK`, `IPV4_IFADDR`, `IPV6_IFADDR`, `IPV4_ROUTE`) |
| macOS | SCDynamicStore keys `State:/Network/Interface/.*/IPv[46]` (or `nw_path_monitor`) |
| Windows | `NotifyIpInterfaceChange` + `NotifyUnicastIpAddressChange` |
| Fallback | Poll every 2 s if the watcher fails to start, and log that it did |

Events are debounced (about 1 s), then diffed by (id, addresses, gateway). Core receives `NetworksChanged`.

## 2. Pinning a socket to one network (`transport::Pinner`)

A source IP alone does not choose the interface (L-56). Each OS needs its own option, applied **before `connect()`**:

| OS | IPv4 | IPv6 | Notes |
|---|---|---|---|
| Linux / Android | `SO_BINDTODEVICE` (`socket2::bind_device`) | same | Unprivileged since kernel 5.7; checked at startup by binding to `lo`. Android later uses `android_setsocknetwork`. |
| macOS | `IP_BOUND_IF` (`bind_device_by_index_v4`) | `IPV6_BOUND_IF` (`bind_device_by_index_v6`) | Unprivileged. A VPN configured to "include all networks" may override it, so we detect and warn. |
| Windows | `setsockopt(IPPROTO_IP, IP_UNICAST_IF, htonl(ifindex))` | `setsockopt(IPPROTO_IPV6, IPV6_UNICAST_IF, ifindex)` | **IPv4 index in network byte order, IPv6 in host order.** Also `bind()` to the interface's address. Not in socket2, so it's written by hand with windows-sys. |

Also, on every OS:
- `bind()` to the interface's source address of the matching family (L-59).
- Never pin loopback destinations (L-58).
- If the interface disappears between listing and connecting, return an ordinary connect error, not a panic.
- **Feature detection:** `Pinner::self_test()` runs at startup. If it fails, only the default network is offered and the UI says why (L-57).

The pinned `std::net::TcpStream` is converted to tokio and wrapped as a `tower::Service<Uri>` connector for hyper-util. TLS goes through hyper-rustls on top of it, so certificates are checked against the URL host and TLS sessions are reused.

## 3. One HTTP client per network

- One hyper `Client` per `(network, origin)`, using HTTP/1.1 keep-alive with several connections (segmented downloads), so each stream pays DNS, TCP and TLS once (Plexo 5a2358c).
- **System proxy settings are ignored** on these clients unless the user configures a proxy per network (L-66).
- A stale pooled socket the server closed gets one immediate retry on a fresh socket.

## 4. DNS per network (L-65)

| OS | Approach |
|---|---|
| macOS | `DNSServiceGetAddrInfo(interfaceIndex=…)`, a scoped lookup through the system resolver |
| Linux | systemd-resolved `ResolveHostname(ifindex, …)` over D-Bus when available; otherwise hickory-resolver pinned to the link's DHCP DNS servers |
| Windows | `DnsQueryEx` with `InterfaceIndex`, or hickory pinned to the adapter's DNS servers |
| Fallback | hickory with DoH to 1.1.1.1 / 8.8.8.8 **over the pinned network** |

DNS counts against the connect deadline (L-09). Each network keeps its own small cache, keyed by the DNS TTL.

## 5. Happy Eyeballs (L-11)

For each network, take the resolved addresses filtered to families that network has, and try them **250 ms apart**, alternating families. The last address that worked for this `(network, host)` goes first. Each address gets a short connect window (at most 3 s, or half the time left); the last gets everything left.

## 6. Per-network link probes

| Probe | When | What it tells us |
|---|---|---|
| **Reachability** | On a network appearing or changing; every 60 s while in use | HTTPS HEAD to `https://<our-backend>/ping` (fallbacks: 1.1.1.1, Google 204). Detects a captive portal (a redirect or wrong body). |
| **Public IP** | With reachability, **over HTTPS only** (plain HTTP may be relayed by iCloud Private Relay, spike S2) | Two networks with the same public IP share an upstream, so bonding won't help → warning (L-62) |
| **Latency** | Same | Shown in the network row |
| **Speed test** | User-triggered, or first-run onboarding | Download (and upload) a test object for N seconds per network, then all together → "you'd get X× with both" |
| **Target probe** | At job start, per network | Range GET `bytes=0-0` to the actual URL. Drop networks that get 403, a redirect elsewhere, or a different version (IP-locked links, L-20) |

## 7. Setups bonding can't help, and how we explain them

| Setup | How we detect it | What we tell the user |
|---|---|---|
| Wi-Fi and Ethernet on the same router | Same subnet or same public IP | "Both go through the same internet line, so combining them won't add speed." |
| Windows turns Wi-Fi off when Ethernet is plugged in | Wi-Fi adapter present but disconnected while Ethernet is up | Step-by-step guide to the policy setting (`fMinimizeConnections`), with a link to Settings |
| Android USB tether on macOS | No adapter appears | Guide to install TetherKit (or an equivalent), with a link |
| Linux kernel < 5.7 | `Pinner::self_test` fails | "Only your default network can be used on this system." |
| VPN capturing all traffic | Default route via utun/tun/TAP | "Your VPN routes everything through one connection." |
| iCloud Private Relay on (macOS) | Plain-HTTP probe exits from relay ranges while HTTPS doesn't | Pending S2 part 2: if relayed `http://` traffic ignores the pin, explain it and suggest https or turning Private Relay off |
| IP-locked link | Target probe gets 403 on some networks | "This link only works from one network; using X only." |

## 8. Things to verify in Phase 1 spikes

- S1: `IP_UNICAST_IF` on Windows 10/11 x64 and ARM64, Wi-Fi + USB tether, IPv4 and IPv6.
- S2: `IP_BOUND_IF` on macOS 13–26 with iPhone USB and Android (TetherKit); a VPN present.
- S3: `SO_BINDTODEVICE` on Ubuntu 22.04/24.04, Fedora; inside Flatpak (`--share=network`).
- S4: Per-network DNS on each OS returns different answers in the netns lab.
- Capture the throughput sum vs each link alone on real hardware, and record it in `docs/05-quality/HARDWARE-RESULTS.md`.
