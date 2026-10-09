# ADR 0012: Nearby speaks the LocalSend protocol

- Status: accepted (2026-10-09, owner approved the Nearby design)

## Context

Nearby sends files between devices on one network without a link. Phones are the most common other device, and Fuselane has no phone app (ADR 0010). LocalSend is an open, widely installed app on every platform with a public, documented protocol (v2: multicast discovery, HTTPS transfers).

## Decision

Implement the LocalSend v2 protocol in Rust from its public description, without using LocalSend's code. Add on top, only between Fuselane devices: certificate pinning to the announced fingerprint and four check words derived from both fingerprints. Trust is per fingerprint.

## Consequences

- Phones with LocalSend work with Fuselane on day one; phones without it use the browser page (NEARBY.md).
- We follow LocalSend's port (53317) and multicast group, and must keep up with protocol changes (pinned to v2).
- LocalSend apps can't show the four words, so transfers with them are marked unverified.
