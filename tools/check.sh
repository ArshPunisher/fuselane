#!/bin/sh
# Every local gate, in CI's order. Fails on the first problem; never pipe its output
# through `head` in a commit chain (that hides the exit code).
set -eu
cd "$(dirname "$0")/.."
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
echo "== rustfmt";  cargo fmt --all --check
echo "== clippy";   cargo clippy --workspace --all-targets --locked -q -- -D warnings
echo "== tests";    cargo nextest run --workspace --locked --no-fail-fast --status-level fail --final-status-level fail
echo "== deny";     cargo deny check -s 2>/dev/null || cargo deny check
echo "== prettier"; pnpm -s format:check
echo "== tsc";      pnpm -s typecheck >/dev/null
echo "All checks passed."
