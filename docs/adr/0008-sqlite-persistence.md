# 0008. SQLite for all app state
- Status: Accepted
- Date: 2026-10-08

## Context
JSON state files are a known trap: never fsynced, a corrupt file silently resets data, old download formats get deleted instead of migrated, and the whole manifest is rewritten every checkpoint.

## Decision
Use one SQLite database (rusqlite, bundled, WAL) for jobs, block progress, upload parts, history, networks, usage and settings. Numbered migrations, tested against fixtures from every release. Corruption is detected and reported, never hidden.

## Consequences
Crash-safe, queryable and migratable. Bulk block progress is stored as a compact blob per job, not one row per block.
