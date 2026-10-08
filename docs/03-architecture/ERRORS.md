# Error model

Goal: **every failure has an owner (a layer) and every user-visible error says what happened and what to do** (L-85). No raw internal errors reach the UI, and no backend error falls through to a generic 500.

## 1. Rust error kinds (`thiserror`, one enum per crate, mapped to one `ErrorKind` in core)

| Layer | Kinds | Default handling |
|---|---|---|
| `Network` | `ConnectFailed`, `Timeout{phase}`, `Reset`, `NoRouteForFamily`, `InterfaceGone`, `PinningUnsupported` | Retry while the network exists; never "fails" the job (L-16) |
| `Server` | `Busy{status, retry_after}`, `Refused{status}`, `LinkExpired{status}`, `BadRange{detail}`, `ShortBody`, `Overrun`, `VersionChanged{proof}`, `NoRanges` | Wait out / strike / Fix link / fail and discard (ENGINE-DOWNLOAD.md §7) |
| `Disk` | `NoSpace`, `QuotaExceeded`, `ReadOnly`, `PermissionDenied`, `Io`, `DriveMissing`, `FileTooLargeForFs` | **Pause the job immediately** with a specific message (not retried as strikes) |
| `Integrity` | `ChecksumMismatch`, `SizeMismatch`, `Incomplete` | Re-fetch bad blocks when possible, otherwise fail; never publish (L-43) |
| `Input` | `InvalidUrl`, `UnsupportedScheme`, `InvalidTorrent`, `TorrentTooLarge`, `UnsafeTorrentPath`, `V2Only` | Shown in the New download dialog |
| `Upload` | `TierLimit{max}`, `TypeBlocked`, `ShareExpired`, `Auth`, `RateLimited{retry_after}`, `BackendUnavailable` | Mirrors the backend codes (BONDED-UPLOADS.md §6) |
| `State` | `NotResumable`, `AlreadyPublishing`, `DifferentFile{was, now}` | Explained, with the possible actions |

Each error carries: `kind`, a `context` (job, network, URL host, *never* secrets), and an optional `retry_after`.

## 2. User-facing catalogue (`core::describe(kind) -> UserMessage { title, body, actions[] }`)

| Kind | Message | Actions |
|---|---|---|
| NoSpace | "There isn't enough space on **{drive}** to finish this download. It needs {need}; {free} is free." | Free up space · Choose another folder · Resume |
| DriveMissing | "The drive with this download isn't connected. Reconnect **{drive}** to continue." | Resume |
| LinkExpired | "This link stopped working (the server said {status}). Paste a fresh link to the same file to continue." | Fix link · Download again |
| VersionChanged | "The file on the server changed since this download started, so it can't be resumed safely." | Download again |
| Busy | "The server is busy. Retrying automatically{until}." | – |
| NoRanges | "This server doesn't support split downloads, so one connection on one network is used." | – |
| PinningUnsupported | "This system can only use your default network." + the OS-specific reason | Learn more |
| ChecksumMismatch | "The downloaded file doesn't match the checksum you gave. Bad parts were re-downloaded {n} times." | Retry · Keep file anyway |
| TierLimit (upload) | "This file is {size}. Free links allow up to {max}." | Split · Upgrade |
| ShareExpired | "This link expired on {date}. Ask the sender for a new one." | – |

The catalogue has snapshot tests: adding an `ErrorKind` without a message fails to compile (exhaustive match) and fails the snapshot test.

## 3. Backend errors

JSON body `{ "error": { "code", "message", "hint" } }` with the right status code: **400** bad input · **401** auth · **403** forbidden · **404** unknown · **409** conflicting state · **410** expired or gone · **413** too large (states the limit) · **415** type not allowed (states the reason) · **429** rate limited + `Retry-After` · **503** a dependency is down + `Retry-After`. Unknown exceptions are caught at the router boundary, logged with a request ID, and returned as **500 with a request ID and a hint**, never a stack trace. The aim is for none to reach that point (alerting covers any that do).

## 4. Logging

Errors are logged once, at the layer that decides what to do, with structured fields. Secrets are redacted by type (`Secret<T>` wrapper).
