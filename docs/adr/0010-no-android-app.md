# 0010. No Android app
- Status: Accepted
- Date: 2026-10-08
- Supersedes: ROADMAP phase P10 (Android), the Google Play part of OPEN-QUESTIONS Q12

## Context
The roadmap ended with P10, a native Android app (uniffi core, Kotlin UI, foreground service). The owner decided on 2026-10-08: "leave android app, we will not do it."

## Decision
- Fuselane ships for **macOS, Windows and Linux** desktops, plus the browser extension (P7). There is no Android app.
- **Android phones stay fully supported as networks**: USB tethering an Android phone to a computer is a core use case (spike S2, tether guides in P3).
- `crates/ffi`, cargo-ndk and the Kotlin UI are not built. Nothing in the core is shaped around a mobile target.

## Consequences
- The roadmap ends at P9 (1.0 launch). Effort moves to the beta, torrents, bonded uploads and the extension.
- The Google Play fee question disappears; only the Chrome Web Store fee (Q12) remains.
- If mobile is wanted later, a new ADR reopens it.
