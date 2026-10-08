#!/bin/sh
# Prints where Fuselane work left off, for the Claude Code SessionStart hook.
# Reads docs/04-plan/STATUS.md ("Now" + newest log entry) and the next unticked steps.
root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
status="$root/docs/04-plan/STATUS.md"
steps="$root/docs/04-plan/STEPS.md"
echo "=== Fuselane: where we left off (from docs/04-plan/STATUS.md) ==="
awk '/^## Now/{p=1} /^## Log/{exit} p' "$status"
echo "--- Latest log entry ---"
awk '/^## Log/{l=1;next} l && /^### /{n++} l && n==1' "$status"
echo "--- Next unticked steps (docs/04-plan/STEPS.md) ---"
grep -n -m 5 '^- \[ \]' "$steps"
echo "=== Read CLAUDE.md rules, then continue from the next step. Update STATUS.md before ending. ==="
