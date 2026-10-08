# Spike S2: macOS per-interface pinning (`IP_BOUND_IF`)

- Date: 2026-10-08 · Machine: Apple Silicon Mac, macOS 27.0.1 · Code: branch `spike/s2-macos-pinning` (`spikes/s2-macos-pin`, never merged)
- Status: **part 1 done** (single uplink). **Part 2 pending:** needs a second real network (iPhone or Android USB tether), see Q10.

## Question

Does `IP_BOUND_IF` / `IPV6_BOUND_IF` (socket2 `bind_device_by_index_v4/v6`) really pin a TCP socket to one interface on macOS, unprivileged, for IPv4 and IPv6, without silently falling back to the default route?

## Setup

One uplink: Wi-Fi `en1` (192.168.1.70 + global IPv6), default route via `en1`. Other interfaces present: `en0`, `en5–7`, `bridge0`, `awdl0`, `llw0`, `utun0–3` (system tunnels, link-local only), `lo0`.

Each test: create a socket, pin it to the interface by index, connect to `cloudflare.com:80` (v4 104.16.132.229, v6 2606:4700::6810:84e5) with a 4 s timeout, request `/cdn-cgi/trace`, read the `ip=` line.

## Results

| Interface | IPv4 | IPv6 |
|---|---|---|
| `en1` (Wi-Fi, has route) | ✅ connect 214 ms, local 192.168.1.70 | ✅ connect 45 ms, local = en1's global IPv6 |
| `en0` (no address) | ❌ `ENETUNREACH` in 1.0 ms | ❌ `EHOSTUNREACH` in 0.7 ms |
| `bridge0` | ❌ `ENETUNREACH` < 1 ms | ❌ `EHOSTUNREACH` < 1 ms |
| `utun0` (link-local only) | ❌ `ENETUNREACH` < 1 ms | ⚠️ **hung until the 4 s timeout** |
| `lo0` | ❌ `ENETUNREACH` | ❌ `EHOSTUNREACH` |
| unpinned (default route) | ✅ via en1 | – |

## Findings

1. **Pinning is enforced and unprivileged.** A socket pinned to an interface without a route fails immediately instead of leaving through the default route. That's the behaviour we need: a pinned stream can never secretly use the wrong network.
2. **Tunnel interfaces can hang instead of failing** (`utun0` IPv6). This confirms L-60 (filter virtual/tunnel adapters by default) and L-09 (every connect has a deadline). The interface filter must exclude `utun*`, `awdl*`, `llw*`, `bridge*` and `anpi*` unless they have a gateway and pass the reachability probe.
3. **iCloud Private Relay transparently relays plain HTTP, even on a pinned raw socket.**
   - Over `http://` (and our raw port-80 socket) Cloudflare saw relay egress addresses (Akamai `2a02:26f7:…`, Cloudflare `104.28.x` / `2a09:bac2:…`).
   - Over `https://` it saw the real ISP addresses (175.111.138.163 and en1's own IPv6).
   - Consequences:
     - **The public-IP / shared-upstream probe (L-62) must use HTTPS**, or it reports the relay instead of the network.
     - For **`http://` downloads on Macs with Private Relay**, we don't yet know whether relayed traffic still honours the pin. Part 2 must test an `http://` download with Wi-Fi + tether and Private Relay on and off. If it doesn't honour the pin, the app must detect Private Relay and explain it (as it does for VPNs).
     - Private Relay also covers DNS, which matters for per-network DNS (S4).

## Decisions / doc changes

- NETWORKING.md §6: the public-IP probe is HTTPS-only. A Private Relay row is added to the "can't help" table, pending part 2.
- New lesson **L-101** (Private Relay relays plain HTTP; probe over HTTPS; detect and explain) and **L-102** (exclude tunnel/AWDL interfaces; they can hang).

## Part 2 (pending hardware)

With Wi-Fi + iPhone USB (and Android via TetherKit):
- Two pinned streams at once, measured per interface (`nettop -m route`), with the sum compared to each link alone.
- `http://` vs `https://` with Private Relay on and off.
- Behaviour with a full-tunnel VPN active.
- Hot-unplug of the tether during a transfer (expect a fast error, not a hang).
