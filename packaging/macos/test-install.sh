#!/bin/sh
# Tests install.sh against a fake local release: the happy path, then a corrupted
# download, a missing checksum and a broken archive, none of which may install.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
rel="$work/release"; mkdir -p "$rel"

make_release() { # $1 = content marker
  app="$work/build/Fuselane.app/Contents/MacOS"
  rm -rf "$work/build"; mkdir -p "$app"
  printf '#!/bin/sh\necho "fuselane-desktop %s"\n' "$1" > "$app/fuselane-desktop"
  chmod +x "$app/fuselane-desktop"
  tar -C "$work/build" -czf "$rel/Fuselane_9.9.9_macos-universal.app.tar.gz" Fuselane.app
  (cd "$rel" && shasum -a 256 Fuselane_9.9.9_macos-universal.app.tar.gz > SHA256SUMS)
}

run() { FUSELANE_RELEASE_BASE="file://$rel" FUSELANE_VERSION=v9.9.9 FUSELANE_DEST="$work/apps" sh "$here/install.sh"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

mkdir -p "$work/apps"
make_release good
run >/dev/null || fail "happy path"
"$work/apps/Fuselane.app/Contents/MacOS/fuselane-desktop" | grep -q good || fail "wrong app installed"

# Upgrading replaces the old app.
make_release newer
run >/dev/null || fail "upgrade"
"$work/apps/Fuselane.app/Contents/MacOS/fuselane-desktop" | grep -q newer || fail "upgrade kept the old app"

# Corrupted download: the checksum no longer matches. Nothing changes.
printf 'x' >> "$rel/Fuselane_9.9.9_macos-universal.app.tar.gz"
if run 2>"$work/err"; then fail "a corrupted download was installed"; fi
grep -q "SHA-256 mismatch" "$work/err" || fail "unclear corruption message"
"$work/apps/Fuselane.app/Contents/MacOS/fuselane-desktop" | grep -q newer || fail "corruption touched the installed app"

# A checksum file without our entry is refused.
make_release missing
echo "0000  something-else.tar.gz" > "$rel/SHA256SUMS"
if run 2>"$work/err"; then fail "installed without a checksum entry"; fi
grep -q "no entry" "$work/err" || fail "unclear missing-entry message"

# An archive without Fuselane.app is refused.
mkdir -p "$work/empty"; echo hi > "$work/empty/readme"
tar -C "$work/empty" -czf "$rel/Fuselane_9.9.9_macos-universal.app.tar.gz" readme
(cd "$rel" && shasum -a 256 Fuselane_9.9.9_macos-universal.app.tar.gz > SHA256SUMS)
if run 2>"$work/err"; then fail "installed an archive without the app"; fi
grep -q "doesn't contain Fuselane.app" "$work/err" || fail "unclear broken-archive message"

echo "install.sh: all cases passed"
