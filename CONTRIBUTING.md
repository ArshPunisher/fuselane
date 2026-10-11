# Contributing to Fuselane

Thanks for helping. Bug reports, tested fixes and translations are all welcome.

## Reporting a bug

Open an [issue](https://github.com/ArshPunisher/fuselane/issues) with:

- your system (macOS, Windows or Linux, and its version) and the Fuselane version (About in the app);
- which networks were connected (Wi-Fi, Ethernet, a phone over USB, ...);
- what you did, what you expected and what happened instead.

The app's **Report a problem** button collects a log without file names or addresses; attach it if you can.

Security problems go privately instead: see [SECURITY.md](SECURITY.md).

## Setting up

You need Rust (the version in `rust-toolchain.toml`), Node 24 and pnpm 11.

```sh
pnpm install
pnpm --filter @fuselane/desktop tauri dev     # the desktop app
cargo run -p fuselane-cli -- --help           # the command-line tool
```

Keep test runs away from a real install: set `FUSELANE_HOME` to an empty folder before running a build, so it doesn't touch your own downloads list.

## Before a pull request

```sh
tools/check.sh
```

It runs everything CI runs: rustfmt, clippy, the Rust tests, cargo-deny, shellcheck, Prettier, TypeScript, the extension tests and builds, and the UI tests in WebKit and Chromium. A pull request needs it green.

- **Small, focused commits** with [conventional](https://www.conventionalcommits.org) messages and a scope: `fix(engine-http): ...`, `feat(desktop): ...`. The body says why.
- **Tests with the change.** A fix comes with a test that fails without it.
- **Every UI string in Hindi too** (`apps/desktop/src/locales/`), and checked at the smallest window size (360 x 560), in light and dark, and with reduced motion.
- **No code copied from other projects** ([ADR 0005](docs/adr/0005-clean-room-policy.md)).
- **No paid services.** Fuselane runs only on free ones ([ADR 0009](docs/adr/0009-zero-cost-policy.md)).

The pull request template asks what you checked and on which systems. [`docs/06-process/WORKFLOW.md`](docs/06-process/WORKFLOW.md) has the full workflow.

## Licence

By contributing you agree that your work is released under the project's [Apache-2.0](LICENSE) licence.
