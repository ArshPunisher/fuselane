# ADR 0013: Remote control speaks aria2's JSON-RPC

- Status: proposed (2026-10-10, built while the owner was away; needs the owner's review before release)

## Context

STEPS 8.7 asks for remote control: a way for other programs and other devices to add downloads and watch them. Many tools already speak aria2's JSON-RPC: the AriaNg web interface, the "Aria2" browser extensions, phone remote apps, and media automation (Sonarr, Radarr and similar). Inventing our own protocol would mean nobody can use it on day one.

## Decision

Add a `fuselane-rpc` crate that speaks the aria2 JSON-RPC methods that make sense for Fuselane (addUri, tellStatus, tellActive/Waiting/Stopped, pause/unpause, remove, getGlobalStat, options for the folder and how many run at once, multicall, getVersion). It is written from aria2's public documentation. Torrent and Metalink uploads are refused for now.

- **Off by default.** Turned on in Settings, which shows the address and the secret.
- **A secret is always needed**: a random 128-bit value, sent as `token:<secret>` like aria2's `--rpc-secret`. It is compared in constant time, and a wrong one waits 250 ms before the reply.
- **This computer only** (127.0.0.1) unless the person allows devices on the local network.
- **DNS rebinding blocked:** the Host header must be `localhost` or an IP address.
- CORS is open (`*`), because web front ends run on other origins; the secret is what guards it.
- HTTP POST only for now (no WebSocket); AriaNg works with its HTTP setting.
- Requests are capped at 1 MB.

## Consequences

- aria2 tools work with Fuselane without changes, and downloads they add use every network.
- We report aria2 version 1.37.0 so clients enable the features we support; methods we don't support return clear errors.
- A new listening port is a new attack surface (SECURITY.md T15). With LAN access on, the secret travels in plain HTTP on the local network.
