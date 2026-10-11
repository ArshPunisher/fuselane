# 0005. Clean-room policy
- Status: Accepted
- Date: 2026-10-08

## Context
Fuselane was inspired by open-source download managers and network tools. We studied how such tools behave and fail. We want Fuselane to be our own work, not a port.

## Decision
- We may learn **ideas, behaviours, edge cases and lessons** (these are documented in `docs/02-product/LESSONS.md`).
- We **do not copy code, UI layouts, copy text, icons or assets** from other projects. Our engine is a new design in Rust.
- When writing a module, work from our design docs, not from other projects' source files.

## Consequences
More design work up front, which the docs already cover. No licence entanglement or "clone" reputation.
