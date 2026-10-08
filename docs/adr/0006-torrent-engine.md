# 0006. librqbit with a per-network SOCKS5 balancer
- Status: Proposed — confirmed or rejected by spike S5
- Date: 2026-10-08

## Context
We need each peer pinned to a network. librqbit (pure Rust) binds the whole session to one device and doesn't do that on Windows. libtorrent-rasterbar supports multiple outgoing interfaces but is C++.

## Decision
v1: librqbit with `proxy_url` pointing at an in-process, authenticated loopback SOCKS5 server, which chooses a network for each CONNECT and dials through `transport::Pinner`. uTP, UPnP, LSD and web seeds stay off. v2: upstream a connector hook. Plan C: libtorrent via cxx.

## Consequences
TCP-only peers at first (like Plexo). An extra local hop per peer (measured in S5). No C++ toolchain.
