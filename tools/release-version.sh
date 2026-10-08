#!/bin/sh
# Prints the workspace version and, given a tag, fails unless the tag is exactly
# "v" + that version (L-78: rc.1 must never match rc.10).
set -eu
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
[ -n "$version" ] || { echo "no workspace version in Cargo.toml" >&2; exit 1; }
if [ $# -gt 0 ]; then
  tag=${1#refs/tags/}
  if [ "$tag" != "v$version" ]; then
    echo "tag $tag does not match the workspace version $version (expected v$version)" >&2
    exit 1
  fi
fi
echo "$version"
