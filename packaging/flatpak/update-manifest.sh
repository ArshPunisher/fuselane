#!/usr/bin/env bash
# Points the Flatpak manifest at a published release's .deb packages and adds the
# release to the AppStream metainfo. The SHA-256s come from the release's own
# SHA256SUMS (a public download: no GitHub token needed). Usage:
#
#   packaging/flatpak/update-manifest.sh v0.1.0-beta.10
#
# Running it again with the same tag changes nothing. The publish date comes from
# GitHub's API; set RELEASE_DATE=YYYY-MM-DD to give it yourself (for example when
# the API's hourly limit is used up). Works with the macOS and Linux tools alike
# (only curl and POSIX awk; no `sed -i`).
set -euo pipefail

repo="ArshPunisher/fuselane"
here=$(cd "$(dirname "$0")" && pwd)
manifest="$here/app.fuselane.Fuselane.yml"
metainfo="$here/app.fuselane.Fuselane.metainfo.xml"

die() {
  printf 'update-manifest: %s\n' "$*" >&2
  exit 1
}

[ $# -eq 1 ] || die "give one release tag, for example: $0 v0.1.0-beta.9"
tag=$1
[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z]+(\.[0-9A-Za-z]+)*)?$ ]] ||
  die "\"$tag\" isn't a release tag. Use vX.Y.Z or vX.Y.Z-beta.N, for example v0.1.0-beta.9."
version=${tag#v}
command -v curl >/dev/null || die "curl isn't installed. Install it and run this again."
if [ ! -f "$manifest" ] || [ ! -f "$metainfo" ]; then
  die "app.fuselane.Fuselane.yml or app.fuselane.Fuselane.metainfo.xml is missing from $here. Restore them from git and run this again."
fi

base="https://github.com/$repo/releases/download/$tag"
sums=$(curl -fsSL --retry 3 "$base/SHA256SUMS") ||
  die "couldn't download $base/SHA256SUMS. Check the tag is right and the release is published (a draft is private), then run this again."

deb_x64="Fuselane_${version}_linux-x64.deb"
deb_arm64="Fuselane_${version}_linux-arm64.deb"
sha_of() {
  # No early `exit` in these awk scripts: with pipefail, a writer cut off by
  # SIGPIPE would fail the whole command.
  printf '%s\n' "$sums" | awk -v f="$1" '!found && ($2 == f || $2 == "*" f) { print $1; found = 1 }'
}
sha_x64=$(sha_of "$deb_x64")
sha_arm64=$(sha_of "$deb_arm64")
for pair in "$deb_x64:$sha_x64" "$deb_arm64:$sha_arm64"; do
  [[ ${pair#*:} =~ ^[0-9a-f]{64}$ ]] ||
    die "$tag's SHA256SUMS has no checksum for ${pair%%:*}. Check the release has both Linux .deb files ($base), or that the Release workflow finished."
done

date=${RELEASE_DATE:-}
if [ -z "$date" ]; then
  api="https://api.github.com/repos/$repo/releases/tags/$tag"
  json=$(curl -fsSL --retry 3 -H 'Accept: application/vnd.github+json' "$api") ||
    die "couldn't read $tag's publish date from $api (GitHub allows 60 requests an hour without a token). Wait, or set RELEASE_DATE=YYYY-MM-DD, and run this again."
  date=$(printf '%s\n' "$json" | awk '
    !found && match($0, /"published_at": *"[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/) {
      s = substr($0, RSTART, RLENGTH); print substr(s, length(s) - 9); found = 1
    }')
fi
[[ $date =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] ||
  die "no publish date for $tag (got \"$date\"). Publish the release first, or set RELEASE_DATE=YYYY-MM-DD, and run this again."

case $version in
  *-*) kind=development ;;
  *) kind=stable ;;
esac

tmp=$(mktemp -d "${TMPDIR:-/tmp}/fuselane-flatpak.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

# Manifest: in each source item whose url is a linux-x64 or linux-arm64 .deb,
# rewrite the url and the sha256 (in whatever order they appear). Two passes:
# the first finds which item is which architecture.
awk -v base="$base" -v version="$version" -v sx="$sha_x64" -v sa="$sha_arm64" '
  FNR == 1 { item = 0 }
  /^ *- type:/ { item++ }
  NR == FNR {
    if ($0 ~ /^ *url: .*_linux-x64\.deb *$/) arch[item] = "x64"
    else if ($0 ~ /^ *url: .*_linux-arm64\.deb *$/) arch[item] = "arm64"
    next
  }
  (item in arch) && /^ *url: / {
    sub(/url: .*/, "url: " base "/Fuselane_" version "_linux-" arch[item] ".deb")
    urls[arch[item]]++
  }
  (item in arch) && /^ *sha256: / {
    sub(/sha256: .*/, "sha256: '\''" (arch[item] == "x64" ? sx : sa) "'\''")
    shas[arch[item]]++
  }
  { print }
  END {
    if (urls["x64"] != 1 || urls["arm64"] != 1 || shas["x64"] != 1 || shas["arm64"] != 1) exit 3
  }
' "$manifest" "$manifest" >"$tmp/manifest" ||
  die "app.fuselane.Fuselane.yml doesn't have exactly one x86_64 and one aarch64 .deb source, each with a url and a sha256. Restore it from git and run this again."

# Metainfo: replace this version's <release> where it is, or add it in date order
# (AppStream lists the newest first).
awk -v version="$version" -v date="$date" -v kind="$kind" \
  -v details="https://github.com/$repo/releases/tag/$tag" '
  BEGIN {
    key = "<release version=\"" version "\""
    block = "    <release version=\"" version "\" date=\"" date "\" type=\"" kind "\">\n" \
            "      <url type=\"details\">" details "</url>\n" \
            "    </release>"
  }
  NR == FNR {
    if (index($0, key)) known = 1
    if ($0 ~ /^ *<releases> *$/) opens++
    if ($0 ~ /^ *<\/releases> *$/) closes++
    next
  }
  skipping { if ($0 ~ /<\/release>/) skipping = 0; next }
  # Already listed: replace it where it is.
  index($0, key) {
    if (!done) print block
    done = 1
    if ($0 !~ /\/> *$/ && $0 !~ /<\/release>/) skipping = 1
    next
  }
  # New: before the first release published the same day or earlier, else last.
  inlist && !known && !done && /<release / {
    d = ""
    if (match($0, /date="[^"]*"/)) d = substr($0, RSTART + 6, RLENGTH - 7)
    if (d <= date) { print block; done = 1 }
  }
  inlist && !known && !done && /^ *<\/releases> *$/ { print block; done = 1 }
  /^ *<releases> *$/ { inlist = 1 }
  { print }
  END { if (opens != 1 || closes != 1 || !done) exit 3 }
' "$metainfo" "$metainfo" >"$tmp/metainfo" ||
  die "app.fuselane.Fuselane.metainfo.xml needs exactly one <releases> ... </releases> list, each tag on its own line. Restore it from git and run this again."

# Write back with cat (keeps each file's permissions) only when something changed.
put() {
  if cmp -s "$1" "$2"; then
    echo "  $(basename "$2"): already up to date"
  else
    cat "$1" >"$2"
    echo "  $(basename "$2"): updated"
  fi
}
echo "Fuselane $version ($kind, published $date)"
echo "  x86_64   $deb_x64  $sha_x64"
echo "  aarch64  $deb_arm64  $sha_arm64"
put "$tmp/manifest" "$manifest"
put "$tmp/metainfo" "$metainfo"
