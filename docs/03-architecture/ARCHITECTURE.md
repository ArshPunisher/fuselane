# Architecture

Status: **proposed** (to be confirmed by the Phase 1 spikes). Decisions are recorded in [`../adr`](../adr).

## 1. System context

```text
                         ┌──────────────────────────── user's computer ────────────────────────────┐
  Browser extension ──── │ native-messaging host ─┐                                                │
  (Chrome/Edge/FF)       │                        │  local API (JSON-RPC over UDS / named pipe)    │
                         │  fuselane CLI ─────────┼──────────────┐                                 │
                         │                        ▼              ▼                                 │
                         │   ┌──────────────── Fuselane core (Rust library) ─────────────────┐     │
                         │   │ jobs · queue · limits · scheduler · persistence · events     │     │
                         │   │ engines: http · torrent · upload       storage · crypto      │     │
                         │   │ transport (pinned sockets, per-network DNS) · netif          │     │
                         │   └───────┬──────────────┬──────────────┬────────────────────────┘     │
                         │   Desktop app (Tauri 2: Rust shell + React UI) embeds the core           │
                         └───────────┼──────────────┼──────────────┼───────────────────────────────┘
                                 Wi-Fi         USB tether      Ethernet        (each socket pinned)
                                     \             |              /
                    HTTP servers · CDNs · torrent peers · Fuselane backend (Cloudflare Workers + R2 + D1)
                                                                          │
                                                       share page in the receiver's browser
```

## 2. Process model

- **The desktop app embeds the core in-process** (one binary, smallest footprint). Closing the window keeps it running in the tray while transfers are active.
- The core also starts a **local API server** on a Unix domain socket (macOS/Linux) or a named pipe (Windows), with a per-user access check. The CLI, the native-messaging host and future remote tools talk to it.
- **`fuselaned`** is the same core without a UI, for servers and NAS boxes. The `fuselane` CLI talks to whichever is running; with neither running, `fuselane get` can run the core inline.
- **Single owner:** an exclusive lock on the data directory means only one core instance runs per user. A second instance hands its request over and exits.
- The **native-messaging host** is the CLI binary in `--native-messaging` mode: a tiny stdin/stdout relay to the local API. It starts the app if it isn't running.

## 3. Monorepo layout

One git repository ([ADR 0004](../adr/0004-monorepo.md)): a Cargo workspace plus a pnpm workspace.

```text
fuselane/
├─ Cargo.toml                 # workspace; shared dependency versions; resolver 3
├─ rust-toolchain.toml        # pinned stable toolchain
├─ package.json / pnpm-workspace.yaml
├─ crates/
│  ├─ netif/          # list interfaces, friendly names, kind, virtual filter, change events (per OS)
│  ├─ transport/      # pinned sockets (per OS), per-network DNS, Happy Eyeballs, TLS, HTTP client per network, link probes
│  ├─ storage/        # staging files, preallocate/sparse, offset writer pool, name claiming/sanitizing, publish, free space
│  ├─ engine-http/    # probe, planner, scheduler, concurrency controller, streams, hedging, retry policy, validators
│  ├─ engine-torrent/ # librqbit integration + per-network peer dialer
│  ├─ engine-upload/  # multipart part planner, uploader, resume; backend client
│  ├─ crypto/         # chunked AES-256-GCM format + shared test vectors
│  ├─ limits/         # token buckets, data-usage periods, schedules
│  ├─ core/           # job manager + state machines, queue, persistence (SQLite), events, settings, history
│  ├─ api/            # local JSON-RPC server/client, extension pairing, native-messaging relay, typed schema
│  ├─ ffi/            # uniffi bindings (Android, later)
│  └─ testkit/        # fault-injecting range server, fake S3, netns helpers, fake interfaces, invariant checkers
├─ apps/
│  ├─ desktop/        # Tauri 2: src-tauri/ (thin commands → core) + src/ (React UI)
│  ├─ cli/            # `fuselane` CLI, `fuselaned` daemon, `--native-messaging` mode (clap)
│  ├─ extension/      # WXT: chrome-mv3, firefox-mv3 (Safari later)
│  ├─ backend/        # Cloudflare Worker (Hono) + D1 migrations + share page
│  ├─ site/           # landing page (later)
│  └─ android/        # Kotlin app over crates/ffi (Phase 10)
├─ packages/
│  ├─ ui/             # shared React components (desktop, extension popup, share page)
│  ├─ api-types/      # TypeScript types generated from Rust (specta) — never hand-written
│  └─ crypto-web/     # WebCrypto decryptor matching crates/crypto (same test vectors)
├─ tools/netlab/      # network-namespace + tc netem scripts for multi-interface tests
├─ docs/
└─ .github/workflows/
```

### Layering rules (checked in CI with `cargo-deny` bans and a dependency-graph test)

```text
netif → transport → { engine-http, engine-torrent, engine-upload } → core → api → apps
storage, limits, crypto are leaves used by the engines/core.
```

- Nothing under `crates/` depends on Tauri, the UI or the OS shell. The desktop app is a thin adapter.
- Engines don't know about SQLite. They take a `ProgressStore` trait, and core implements it.
- Every OS-specific piece sits behind a trait with `cfg(target_os)` modules: `netif::Watcher`, `transport::Pinner`, `storage::FileOps`. There is a fake implementation of each for tests.

## 4. Core domain model

| Entity | Meaning |
|---|---|
| `Network` | A usable interface: id (stable, derived from the MAC/GUID, not the device name), device name, index, friendly name, kind, addresses, gateway, DNS servers, metered flag, status |
| `Job` | A user-level transfer: `Download(Http\|Torrent)` or `Upload` |
| `Plan` | How a job's bytes are split: blocks (download), parts (upload), pieces (torrent) |
| `Stream` | One pinned connection doing work on one network |
| `Attempt` | One request for one block on one stream (primary or hedge) |
| `Version` | What the server says the file is: size, ETag (normalized), Last-Modified |
| `Usage` | Bytes per network per period, download and upload counted separately |

### State machines (explicit enums, exhaustive matching, transition table unit-tested)

- **Job:** `Queued → Running → {Paused, Completed, Failed{resumable}, Cancelled}`, plus `Publishing` (pause and cancel are refused while publishing) and `WaitingForNetwork` (not an error).
- **Network, per job:** `On | Off | Offline | Unreachable | Failed | AtLimit | Blocked(ip-locked)`.
- **Stream:** `Idle | Connecting | Transferring | Retrying{until} | Retiring`.

## 5. Events and UI updates

- Core publishes typed events on a broadcast bus (`JobChanged`, `BlocksChanged{seq, delta}`, `NetworksChanged`, `UsageChanged`, `LinkOffered`, …).
- The desktop shell forwards them through `tauri::ipc::Channel` at most **10 Hz per job**, sending only what changed (L-34). The CLI and API subscribers get the same stream.
- Rates are read from meters on a 500 ms engine tick, never computed from event gaps (L-30).

## 6. Persistence

- **SQLite** (rusqlite, WAL, `synchronous=NORMAL`; `FULL` around publish) in the OS app-data directory ([ADR 0008](../adr/0008-sqlite-persistence.md)).
- Tables: `jobs`, `job_networks`, `blocks` (one compact row per job: a blob bitmap plus bytes per network), `upload_parts`, `history`, `networks` (user names, colours), `usage`, `settings`, `schema_migrations`.
- Numbered **migrations** run at startup, and each one is tested against a fixture DB from every released version (L-51).
- The staging file is fsynced before its progress is recorded as durable (L-55). Checkpoints happen every 15 s, and immediately on pause, quit, relink and publish.
- On a corrupt DB: keep the file aside, open a fresh one, and tell the user (L-50).

## 7. Configuration and paths

| What | macOS | Windows | Linux |
|---|---|---|---|
| App data (DB, logs) | `~/Library/Application Support/app.fuselane` | `%APPDATA%\Fuselane` | `$XDG_DATA_HOME/fuselane` |
| Default downloads | `~/Downloads` | Known Folder Downloads | XDG download dir |
| Local API socket | `$TMPDIR/fuselane-<uid>.sock` (0600) | `\\.\pipe\fuselane-<sid>` | `$XDG_RUNTIME_DIR/fuselane.sock` |

## 8. Logging and diagnostics

- `tracing` with a rotating file appender (7 files × 10 MB), at `info` by default and `debug` per module through settings.
- Secrets (cookies, tokens, presigned URLs' query strings) are **redacted** when logged.
- "Copy diagnostics" bundles the version, OS, the list of networks (addresses masked) and the last 1,000 log lines.
- No crash-reporting service (zero-cost policy, [ADR 0009](../adr/0009-zero-cost-policy.md)); diagnostics stay local until the user copies them.

## 9. Where the details live

| Topic | Doc |
|---|---|
| Libraries and versions | [TECH-STACK.md](TECH-STACK.md) |
| Interface discovery, pinning, DNS, probes | [NETWORKING.md](NETWORKING.md) |
| HTTP download engine | [ENGINE-DOWNLOAD.md](ENGINE-DOWNLOAD.md) |
| Torrents | [TORRENT.md](TORRENT.md) |
| Bonded uploads, backend, share links | [BONDED-UPLOADS.md](BONDED-UPLOADS.md) |
| Browser extension | [BROWSER-EXTENSION.md](BROWSER-EXTENSION.md) |
| Error taxonomy and messages | [ERRORS.md](ERRORS.md) |
| Threat model | [SECURITY.md](SECURITY.md) |
| Per-OS specifics and packaging | [PLATFORMS.md](PLATFORMS.md) |
