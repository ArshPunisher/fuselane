# Security and threat model

## Assets

User files and destinations · cookies and auth headers forwarded by the extension · share-link keys · device tokens · signing keys (Apple, Windows, updater minisign) · the update channel · the backend (R2, D1).

## Threats and controls

| # | Threat | Control |
|---|---|---|
| T1 | **A malicious update or installer** (FDM in 2020–22 and JDownloader in 2026 had their sites compromised) | Signed and notarized builds. The updater checks minisign signatures. Build provenance attestations. Releases only from CI with protected environments. The download page lists SHA-256 checksums. |
| T2 | **A compromised UI** (XSS from a file name or torrent metadata) drives the core | Strict CSP (no inline scripts). Tauri v2 capabilities: only our commands, `fs` scope empty (all file I/O in Rust). **Every command payload validated at runtime** in Rust (L-97). Paths only inside user-chosen folders (L-98). Only http, https and magnet schemes. |
| T3 | **A local process abuses the local API** | Unix socket at 0600 in a per-user directory; Windows named pipe with a per-user ACL. The localhost WebSocket requires a pairing token and checks Origin and Host (prevents DNS rebinding). |
| T4 | **A malicious website reaches the localhost fallback** | Reject web origins; token required; rate-limited pairing; codes expire after 2 min. |
| T5 | **Cookie or token leakage** | In memory only, scoped to a job. Redacted in logs. Keychain-encrypted only if the user opts into resume-after-restart. |
| T6 | **Path traversal from servers or torrents** | Sanitization for every OS (L-40, L-68). Never follow symlinks when removing (L-69). |
| T7 | **Downloads bypass Gatekeeper/SmartScreen** | Set `com.apple.quarantine` (macOS) and the `Zone.Identifier` alternate stream (Windows) on published files (L-99). |
| T8 | **SSRF through the probe** (intranet addresses) | It's a desktop app, so the risk is low, but block `file:` and other schemes; optionally warn on private IPs when a link came from the extension. |
| T9 | **Share-link abuse** (malware, CSAM, piracy) | ToS, a report button, takedowns, quotas, rate limits, no public listing. Policy on scanning encrypted content is in OPEN-QUESTIONS. |
| T10 | **Guessing share links** | 128-bit random slugs; the key in the fragment; optional password; expiry. |
| T11 | **Losing the updater signing key** | Kept offline in a password manager plus a hardware backup; documented key rotation. Losing it means existing installs can't be updated. |
| T12 | **Supply chain** (crates, npm packages) | Lockfiles committed; `cargo-deny` (licences + advisories); `cargo-audit`; `pnpm audit`; Renovate with review; GitHub Actions pinned by SHA. |
| T13 | **Test knobs reaching production** | Compiled behind a `testkit` cargo feature that release builds never enable (L-100); a CI check greps the release binary. |
| T14 | **Denial of service on servers** (seen as a leech) | Polite concurrency limits, honouring Retry-After, a remembered per-host ceiling, an honest User-Agent. |

## Tauri hardening checklist

- [ ] CSP set in `tauri.conf.json`; no `unsafe-inline` for scripts.
- [ ] `capabilities/*.json`: minimal permissions per window.
- [ ] `dangerousRemoteDomainIpcAccess` not used.
- [ ] `opener` allowlist: only http/https, plus revealing our own published files.
- [ ] Navigation guard: the main webview can't navigate away from the app origin.
- [ ] No `shell` plugin command execution.

## Secrets in CI

Apple certificate and API key, the Windows signing credentials, the updater key and the Cloudflare API token live only in a GitHub **environment** (`release`) with required reviewers. PR workflows never see them.
