# 0001. Record architecture decisions
- Status: Accepted
- Date: 2026-10-08

## Context
Plexo's history shows repeated design churn: the stream controller was rewritten 4 times, and settings and manifest formats were rewritten several times. The reasons lived only in commit messages. Future sessions (human or Claude) need to know *why* things are the way they are.

## Decision
Record every significant technical decision as an ADR in `docs/adr`. A PR that changes a decision must add a superseding ADR.

## Consequences
A small writing cost per decision. Decisions aren't reopened without new facts.
