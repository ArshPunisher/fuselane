# ADR 0013: Remote control speaks aria2's JSON-RPC

- Status: accepted (2026-10-10, owner approved in chat)

## Context

STEPS 8.7 asks for remote control: a way for other programs and other devices to add downloads and watch them. Many tools already speak aria2's JSON-RPC: the AriaNg web interface, the "Aria2" browser extensions, phone remote apps, and media automation (Sonarr, Radarr and similar). Inventing our own protocol would mean nobody can use it on day one.

## Decision

Add a `fuselane-rpc` crate that speaks the aria2 JSON-RPC methods that make sense for Fuselane (addUri, tellStatus, tellActive/Waiting/Stopped, pause/unpause, remove, getGlobalStat, options for the folder and how many run at once, multicall, getVersion). It is written from aria2's public documentation. Metalinks are taken too (addMetalink); torrent uploads are refused for now.

- **Off by default.** Turned on in Settings, which shows the address and the secret.
- **A secret is always needed**: a random 128-bit value, sent as `token:<secret>` like aria2's `--rpc-secret`. It is compared in constant time, and a wrong one waits 250 ms before the reply.
- **This computer only** (127.0.0.1) unless the person allows devices on the local network.
- **DNS rebinding blocked:** the Host header must be `localhost` or an IP address.
- CORS is open (`*`), because web front ends run on other origins; the secret is what guards it.
- HTTP POST and WebSocket (AriaNg's default) on the same `/jsonrpc`. Stopping the server (off, or a new secret) closes open WebSockets.
- **Push notifications on WebSockets** (added 2026-10-10): `aria2.onDownloadStart`, `onDownloadPause`, `onDownloadStop`, `onDownloadComplete` and `onDownloadError`, shaped as aria2 sends them (`{"jsonrpc":"2.0","method":…,"params":[{"gid":…}]}`) and listed by `system.listNotifications`. A socket gets them only after one of its calls carried the right secret, so an unauthenticated socket learns nothing, not even GIDs. The app diffs the download list it already publishes: running → start, paused → pause, finished → complete, failed → error, removed or cancelled before finishing → stop; waiting, adding paused and clearing a finished one say nothing, as in aria2. A slow socket that falls behind drops the oldest notifications; front ends poll as well, so it costs seconds, never correctness. The phone page uses the same WebSocket to refresh at once and keeps its 2-second polling as the fallback; its CSP names its own `ws://` address, since not every browser counts it as `'self'`. aria2's `onBtDownloadComplete` is not sent (aria2 tools don't hand torrents to this endpoint).
- Requests are capped at 1 MB.
- A small remote page at `/` for phones (list, add, pause, resume), with no outside resources and a strict CSP. Settings shows its link as a QR code only on request; the secret rides in the link's `#fragment`, which browsers never send to the server, and the page removes it from the address bar after reading it.

## Consequences

- aria2 tools work with Fuselane without changes, and downloads they add use every network.
- We report aria2 version 1.37.0 so clients enable the features we support; methods we don't support return clear errors.
- A new listening port is a new attack surface (SECURITY.md T15). With LAN access on, the secret travels in plain HTTP on the local network.
