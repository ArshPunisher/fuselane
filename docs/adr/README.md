# Architecture Decision Records

One file per significant decision: `NNNN-short-title.md`, never edited after it's accepted. To change a decision, add a new ADR that **supersedes** the old one, and mark the old one `Superseded by NNNN`.

Template:

```markdown
# NNNN. Title
- Status: Proposed | Accepted | Superseded by NNNN
- Date: YYYY-MM-DD
## Context
## Decision
## Consequences
```

| # | Decision | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record decisions as ADRs | Accepted |
| [0002](0002-tauri-over-electron.md) | Tauri 2 desktop shell, not Electron | Accepted (to be confirmed by spike S7) |
| [0003](0003-rust-core.md) | One Rust core for every app | Accepted |
| [0004](0004-monorepo.md) | One monorepo for all parts | Accepted |
| [0005](0005-clean-room-policy.md) | Clean-room policy toward Plexo | Accepted |
| [0006](0006-torrent-engine.md) | librqbit + per-network SOCKS5 balancer | Proposed (spike S5) |
| [0007](0007-upload-backend.md) | Cloudflare Workers + R2 + D1 for uploads | Superseded by 0011 |
| [0008](0008-sqlite-persistence.md) | SQLite for all app state | Accepted |
| [0009](0009-zero-cost-policy.md) | Zero-cost policy: open source, free services only | Accepted |
| [0010](0010-no-android-app.md) | No Android app (Android phones remain supported as tethered networks) | Accepted |
| [0011](0011-fuse-send-p2p.md) | Fuse Send: peer-to-peer sharing replaces cloud uploads | Accepted (spike S7) |
| [0012](0012-nearby-localsend.md) | Nearby speaks the LocalSend protocol | Accepted |
| [0013](0013-aria2-remote-control.md) | Remote control speaks aria2's JSON-RPC | Proposed |
