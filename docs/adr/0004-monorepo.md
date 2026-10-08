# 0004. One monorepo for all parts
- Status: Accepted
- Date: 2026-10-08

## Context
Fuselane has a core, a desktop app, a CLI, an extension, a backend, a share page and shared types. Their contracts (the local API, the extension message schema, the backend API, the crypto format) change together.

## Decision
One git repository: a Cargo workspace (`crates/*`, `apps/cli`, `apps/desktop/src-tauri`) plus a pnpm workspace (`apps/desktop`, `apps/extension`, `apps/backend`, `packages/*`). CI runs only the jobs affected by the changed paths, but the contract tests always run.

## Consequences
A change to a contract and every user of it lands in one commit. A single version tag for desktop + CLI; the extension and backend are versioned separately but released from the same repo.
