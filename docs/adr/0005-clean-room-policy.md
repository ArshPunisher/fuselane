# 0005. Clean-room policy toward Plexo
- Status: Accepted
- Date: 2026-10-08

## Context
Plexo (MIT) inspired Fuselane. We studied its behaviour, bug history and tests in depth. We want Fuselane to be our own work, not a port.

## Decision
- We may learn **ideas, behaviours, edge cases and lessons** (these are documented in `docs/02-product/LESSONS-FROM-PLEXO.md`).
- We **do not copy code, UI layouts, copy text, icons or assets** from Plexo. Our engine is a new design in Rust.
- When writing a module, work from our design docs, not from Plexo's source files.
- The README credits Plexo as inspiration.

## Consequences
More design work up front, which the docs already cover. No licence entanglement or "clone" reputation.
