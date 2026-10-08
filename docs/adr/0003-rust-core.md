# 0003. One Rust core for every app
- Status: Accepted
- Date: 2026-10-08

## Context
Per-interface socket pinning needs OS-level socket options (`SO_BINDTODEVICE`, `IP_BOUND_IF`, `IP_UNICAST_IF`). Node can't set them without FFI. We also want a CLI, a daemon and, later, Android to share the same engine.

## Decision
The whole engine is Rust crates under `crates/`, with no UI or Tauri dependency. Apps are thin adapters. Android uses uniffi bindings.

## Consequences
One implementation to test. Rust skills are required. Some of the UI's TypeScript types are generated from Rust (specta).
