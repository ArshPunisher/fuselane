#!/usr/bin/env bash
# https://fuselane.app is the Next.js site in apps/site-next. Cloudflare Pages
# (project "fuselane-web", connected to this repo) builds and deploys it on
# every push to main, so this script is only a manual fallback: it builds
# locally and uploads the same files to that project.
#
#   tools/deploy-site.sh        build apps/site-next, deploy to "fuselane-web"
#
# GitHub Pages keeps serving the plain site (apps/site) for installed apps
# (update feed, sign-in check); the update-feed workflow handles that copy.
#
# Needs `npx wrangler login` once.
set -euo pipefail
cd "$(dirname "$0")/.."

# Run from the site's folder: wrangler refuses the workspace root.
WRANGLER="npx --yes wrangler@4 --cwd apps/site-next"
PROJECT=fuselane-web

pnpm --filter @fuselane/site-next build
$WRANGLER pages deploy out --project-name "$PROJECT" --branch main --commit-dirty=true
echo "Done: https://$PROJECT.pages.dev and https://fuselane.app"
