# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue:
**[Report a vulnerability](https://github.com/ArshPunisher/fuselane/security/advisories/new)** (GitHub's private reporting).

Include what an attacker could do, the steps to reproduce it, and the Fuselane version and system. You'll get a reply within a few days, and a fix is released as soon as it's ready. You're credited in the release notes unless you'd rather not be.

## Supported versions

Fuselane is in beta. Only the latest release gets security fixes; the app updates itself, so please stay on it.

## What's in scope

- The desktop app, the command-line tool and the browser extension in this repository.
- The update feed and its signatures, the installers and the install script.
- Nearby sending (pairing, trust, the phone page) and the local API.

Problems in a website a download comes from, or in a server Fuselane talks to, are out of scope, but tell us if Fuselane handles them unsafely.

## How Fuselane protects you

- Updates are signed and checked before they install.
- Nearby connections use TLS with pinned fingerprints; a new computer is trusted only after you compare its check words.
- The local API and the phone page answer only on this computer or with a one-time token.
- Downloaded file names are cleaned so nothing is written outside the folder you chose.
