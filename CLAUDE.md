# CLAUDE.md: Fuselane

Read this first, every session. It tells you what this project is, where things are, and the rules that are not negotiable.

## What Fuselane is

A cross-platform (macOS, Windows, Linux; Android later) **download and upload manager that bonds every network connection** (Wi-Fi + USB-tethered phone + Ethernet) for one transfer. Pillars: bonded HTTP and torrent downloads, **bonded uploads with share links**, a **browser extension** that captures downloads, and **signed releases with auto-update**. Inspired by Plexo (github.com/anmolkapil/plexo), studied in depth, **never copied**.

## Start of every session

1. Read [`docs/04-plan/STATUS.md`](docs/04-plan/STATUS.md): the current phase, the next step, blockers.
2. Open the next unticked step in [`docs/04-plan/STEPS.md`](docs/04-plan/STEPS.md).
3. Read the design doc for that area (map below) and the `L-xx` rules the step names in [`docs/02-product/LESSONS-FROM-PLEXO.md`](docs/02-product/LESSONS-FROM-PLEXO.md).
4. If the user asks for something not in the plan, check it against [`docs/00-overview/VISION.md`](docs/00-overview/VISION.md) and add it to STEPS.md / FEATURES.md before building it.

## End of every session

- Tick finished steps in STEPS.md (and PARITY-CHECKLIST.md when it applies). Only tick a step whose test exists and passes.
- Add a dated entry to STATUS.md: done / next / blocked.
- Write down any decision made in conversation (an answer in OPEN-QUESTIONS.md, or a new ADR) **before the session ends**.

## Doc map

| Need | File |
|---|---|
| Why we build it, for whom | `docs/00-overview/VISION.md` |
| What Plexo has (must match or beat) | `docs/02-product/PARITY-CHECKLIST.md` |
| What we add beyond Plexo | `docs/02-product/FEATURES.md` |
| **Bugs we must not repeat (L-01…L-100)** | `docs/02-product/LESSONS-FROM-PLEXO.md` |
| System design, crates, layering | `docs/03-architecture/ARCHITECTURE.md` |
| Libraries and versions | `docs/03-architecture/TECH-STACK.md` |
| Interface pinning, DNS, probes | `docs/03-architecture/NETWORKING.md` |
| HTTP engine | `docs/03-architecture/ENGINE-DOWNLOAD.md` |
| Torrents / uploads / extension | `docs/03-architecture/{TORRENT,BONDED-UPLOADS,BROWSER-EXTENSION}.md` |
| Errors and messages | `docs/03-architecture/ERRORS.md` |
| Security | `docs/03-architecture/SECURITY.md` |
| Per-OS details | `docs/03-architecture/PLATFORMS.md` |
| Decisions | `docs/adr/` |
| Phases, steps, status, open questions | `docs/04-plan/` |
| Test levels and gates | `docs/05-quality/TESTING.md` |
| Workflow, commits, releases | `docs/06-process/WORKFLOW.md` |
| Raw research (search it before re-researching) | `docs/01-research/` (`plexo-forensics.md` has every Plexo bug, constant and edge case) |

## Architecture in brief

- **Rust core** in `crates/` (netif → transport → engine-http / engine-torrent / engine-upload → core → api), with storage, limits and crypto as leaves. No crate depends on Tauri or the UI.
- **Apps** in `apps/`: `desktop` (Tauri 2 + React 19 + Vite + Tailwind 4; the core runs in-process), `cli` (`fuselane`, `fuselaned`, `--native-messaging`), `extension` (WXT MV3), `backend` (Cloudflare Worker + R2 + D1), later `android`.
- **Shared TS** in `packages/` (`ui`, `api-types` generated from Rust with specta (never hand-edit), `crypto-web`).
- **State** in SQLite (WAL) with numbered migrations. **Scheduler, concurrency, retry policy and limits are pure functions of a snapshot.**
- **OS-specific code** only behind traits (`Pinner`, `Watcher`, `FileOps`, `Integration`) with `cfg(target_os)` modules and a fake for tests.

## Non-negotiable rules

1. **Plan before code.** Follow STEPS.md. If a step changes a design, update the design doc or add an ADR first.
2. **Test first, and test at the cheapest level** (TESTING.md). No checkbox without a test. Fast tests run on every PR on all three OSes.
3. **All three OSes, always.** A feature isn't done until it works on macOS, Windows and Linux in CI. Never drop an OS's CI job.
4. **Clean room.** Learn from Plexo's behaviour and lessons; never copy its code, UI, copy text or assets (ADR 0005). Don't open Plexo's source files while writing a module; work from our docs.
5. **Correctness over speed.** Check every server response (exact Content-Range, length, version), reconcile with the disk before resuming, never publish an incomplete file, SHA-256 every file in tests.
6. **Every failure has an owner** (network / server / disk / integrity / input / state) and every user-facing error goes through the catalogue in `core::describe` with a plain message and an action. No raw errors in the UI.
7. **Backend errors** return the correct status with `{error:{code,message,hint}}`: 413 states the size limit, 415 states the reason, 410 for expired links, 429/503 with Retry-After. Nothing falls through to a generic 500.
8. **Socket pinning:** never rely on a source IP alone. Linux `SO_BINDTODEVICE`, macOS `IP_BOUND_IF`, Windows `IP_UNICAST_IF` (IPv4 index in network byte order). Never pin loopback. Per-network clients ignore the system proxy.
9. **Security:** validate every IPC/API payload at runtime; only http, https and magnet URLs; writes only inside user-chosen folders; cookies and tokens never logged; test knobs only behind the `testkit` feature.
10. **No secrets in the repo.** Signing keys and tokens live only in the CI `release` environment.

## Coding conventions

- **Rust:** stable toolchain pinned in `rust-toolchain.toml`; `cargo fmt`; `clippy -D warnings`; `thiserror` in libraries, `anyhow` only in binaries; no `unwrap()`/`expect()` outside tests and startup invariants; `tracing` for logs; tunable constants live in one `Tuning` struct per engine; public items get doc comments that say *why*.
- **TypeScript:** strict mode; no `any`; types for IPC come from `packages/api-types`; Zustand for UI state; keep engine logic out of the UI.
- Match the surrounding code's style, naming and comment density. Small modules over god files (Plexo's 1,700-line manager is the anti-pattern).

## UI work

Whenever building or changing UI (desktop, extension popup, share page, landing site):
1. Load the **`design-taste-frontend`** skill first and follow it (yes, even for app screens).
2. After the change, review with the **`web-design-guidelines`** skill and fix what it finds.
3. Check it in a real browser with **`playwright-cli`** (including the minimum window size and 375px for web pages), and take screenshots.
4. For visual direction, use a brand reference from `~/.claude/design-md/design-md/<brand>/` (ask the owner or suggest one).
5. Keep it light for WebKitGTK: virtualized lists, no heavy blur.

## Git

- Small, focused conventional commits (`feat(engine-http): …`, `docs(plan): …`), one logical change each, each one building. The body explains why.
- **The author is the repo owner only: no `Co-Authored-By`, no "Generated with Claude Code", no AI attribution** in commits or PRs.
- "commit" = commit only; "commit and push" = commit, then push the current branch. Never force-push. Never push to another branch without asking.
- Commit after each completed step; don't let work pile up uncommitted.

## Commands (fill in as the workspace grows)

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
pnpm -r lint && pnpm -r test
sudo tools/netlab/up.sh   # Linux network lab (Phase 1+)
```

## Current phase

**P0 Foundations.** Planning docs are done. Next: owner decisions in `docs/04-plan/OPEN-QUESTIONS.md` (licence, GitHub remote, signing budget, domain), then the workspace skeleton and CI (STEPS 0.7–0.13). Always confirm against STATUS.md, which is the source of truth.
