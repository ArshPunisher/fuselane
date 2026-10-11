# 0001. Record architecture decisions
- Status: Accepted
- Date: 2026-10-08

## Context
Download tools' histories show repeated design churn: stream controllers rewritten several times, settings and manifest formats rewritten again and again. The reasons lived only in commit messages. Future sessions (human or Claude) need to know *why* things are the way they are.

## Decision
Record every significant technical decision as an ADR in `docs/adr`. A PR that changes a decision must add a superseding ADR.

## Consequences
A small writing cost per decision. Decisions aren't reopened without new facts.
