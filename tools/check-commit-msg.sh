#!/bin/sh
# Checks one commit message (file path in $1) against our rules (WORKFLOW.md §3):
# conventional type and optional scope, a subject, and no AI attribution.
msg_file="$1"
subject=$(head -n 1 "$msg_file")
case "$subject" in Merge\ *|Revert\ *|fixup!\ *|squash!\ *) exit 0 ;; esac
if ! printf '%s' "$subject" | grep -Eq '^(feat|fix|refactor|perf|docs|test|build|ci|chore|style|revert)(\([a-z0-9-]+\))?!?: .{1,72}$'; then
  echo "commit-msg: subject must look like 'feat(scope): what changed' (max 72 chars after the colon)." >&2
  echo "  got: $subject" >&2
  exit 1
fi
if grep -Eiq '^(co-authored-by:.*(claude|anthropic)|.*generated with \[?claude)' "$msg_file"; then
  echo "commit-msg: no AI attribution in commit messages (WORKFLOW.md §3)." >&2
  exit 1
fi
