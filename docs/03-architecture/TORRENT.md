# Torrent engine (`crates/engine-torrent`)

Decision: [ADR 0006](../adr/0006-torrent-engine.md). Use a mature engine and don't write BitTorrent ourselves (L-67; Plexo learned this in 5404d92).

## Engine: librqbit 9.x

- Pure Rust (tokio), Apache-2.0. Supports magnets, DHT, PEX, HTTP and UDP trackers, uTP, and pluggable storage.
- **Limitation:** it binds the *whole session* to one device, and not at all on Windows (`BindDeviceNotSupported`). It can't choose a network per peer, because its connector is crate-private.

## Pinning each peer to a network

**v1: an in-process SOCKS5 balancer, so librqbit needs no fork.**

```text
librqbit ──(proxy_url = socks5://127.0.0.1:<random port>, auth token)──► Fuselane SOCKS5 server
                                                                    │ for each CONNECT:
                                                                    │  choose network (weighted)
                                                                    ▼
                                                     transport::Pinner → peer over Wi-Fi / tether / Ethernet
```

- The SOCKS server listens only on loopback with username/password auth, so other local apps can't use it.
- **Choosing a network:** start with the network that has the fewest peers. Once throughput is known, weight the choice by each network's measured download rate (+ over Plexo's fewest-peers rule). Respect `AtLimit`/`Off` states, and give an `Unreachable` network one dial at a time.
- **Turn off what bypasses the proxy** until it can be pinned: uTP, UPnP/NAT-PMP, LSD, web seeds. DHT and UDP trackers may run on the default network (they're light); spike S5 measures this.
- **Incoming peers:** listen on all interfaces; attribute each one to a network by its socket's local address (L-71).
- **Unreachable rule:** a network is marked `Unreachable` after 5 dials with no answer and 60 s without one, if it has no peers and another network does.

**v2: an upstream connector hook.** Open a PR against librqbit adding a public `PeerConnector` trait and Windows `IP_UNICAST_IF` binding. Once merged, drop the SOCKS hop. Then add uTP per network (one UDP socket per network).

**Plan C: libtorrent-rasterbar 2.x** through `cxx` bindings, using `outgoing_interfaces`. Only if librqbit can't do the job (poor swarm performance or missing features). The cost is a C++ toolchain on 3 OSes plus Android.

## Features (parity + better)

| Feature | Notes |
|---|---|
| Magnet / `.torrent` (URL, file, drag-drop, "Open with") | `.torrent` up to 10 MB; magnet metadata timeout 3 min; a newer lookup cancels the older |
| File selection before and during the download | A finished file stays selected; at least one file must be chosen |
| Path safety | Traversal, case-insensitive collisions, file-vs-folder prefix, Windows-invalid names, 255-byte components (L-68) |
| Storage | Written in place under the claimed name ("Name (1)"); bytes in unchosen edge pieces are deleted at publish (L-69) |
| Credit per network | Bytes are attributed only after a piece is **verified**, split across networks in proportion |
| Seeding (+) | Off by default. Optional ratio or time limit; never on a metered network unless allowed. |
| Limits | Download and upload throttled; data usage counts torrent bytes (upload separately) |
| Removal | Only files we created; refuse symlinks and folder swaps |
| v2-only torrents | Supported if librqbit supports them; otherwise a clear message |

## Spike S5 must answer

1. Does librqbit route **every** peer TCP connection through the SOCKS proxy (outgoing peers, and tracker HTTP)?
2. With uTP off, how does a real public swarm perform with and without the proxy hop (Ubuntu ISO torrent)?
3. Do Windows and macOS behave the same?
4. What is the CPU cost of the proxy hop at 500 Mbit/s?
