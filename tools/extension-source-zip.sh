#!/bin/sh
# Builds the source package that addons.mozilla.org (and Edge) reviewers use to
# rebuild the Firefox add-on byte for byte: the extension, the shared capture
# package, the workspace files and the lockfile, plus the build steps.
#   tools/extension-source-zip.sh [out.zip]
set -eu
cd "$(dirname "$0")/.."
version=$(sed -n 's/.*"version": "\(.*\)".*/\1/p' apps/extension/package.json | head -1)
out=${1:-apps/extension/.output/fuselaneextension-$version-source-package.zip}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
git archive --format=tar HEAD \
  package.json pnpm-lock.yaml pnpm-workspace.yaml packages/capture apps/extension |
  tar -x -C "$tmp"
rm -rf "$tmp/apps/extension/store" "$tmp/apps/extension/e2e"
cat > "$tmp/SOURCE-README.md" <<README
# Fuselane browser extension $version: how to build

The add-on is built from this folder with WXT. Nothing is minified by hand;
WXT bundles the TypeScript sources in \`apps/extension\` and \`packages/capture\`.

Requirements: Node.js 22.12 or newer and pnpm 11 (\`corepack enable\` gives the
pnpm version pinned in package.json).

    pnpm install --frozen-lockfile --filter @fuselane/extension...
    pnpm --filter @fuselane/extension build:firefox

The built add-on is in \`apps/extension/.output/firefox-mv3\`; it matches the
uploaded zip. Source repository: https://github.com/ArshPunisher/fuselane
README
mkdir -p "$(dirname "$out")"
rm -f "$out"
(cd "$tmp" && zip -qr -X - .) > "$out"
echo "$out"
