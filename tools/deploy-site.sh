#!/usr/bin/env bash
# Publishes the website to https://fuselane.app (Cloudflare Pages, free).
# GitHub Pages keeps serving the same site for installed apps (update feed,
# sign-in check); this only adds the fuselane.app copy.
#
#   tools/deploy-site.sh        build, then deploy to the "fuselane" project
#
# Needs `npx wrangler login` once. The first run creates the project and asks
# Cloudflare to serve it at fuselane.app and www.fuselane.app.
set -euo pipefail
cd "$(dirname "$0")/.."

# Run from the site's folder: wrangler refuses the workspace root.
WRANGLER="npx --yes wrangler@4 --cwd apps/site"
PROJECT=fuselane

pnpm --filter @fuselane/site build

if ! $WRANGLER pages project list 2>/dev/null | grep -q "│ $PROJECT "; then
  # --force keeps classic Pages (static files) instead of converting to a Worker.
  $WRANGLER pages project create "$PROJECT" --production-branch main --force
fi

$WRANGLER pages deploy dist --project-name "$PROJECT" --branch main --commit-dirty=true

# Custom domains (no-op when already added). Cloudflare then shows them under
# Workers & Pages -> fuselane -> Custom domains; press "Activate" there if it
# asks, which adds the DNS record for you.
token=$(grep -E '^oauth_token' "$HOME/Library/Preferences/.wrangler/config/default.toml" 2>/dev/null | cut -d'"' -f2 || true)
account=$($WRANGLER whoami 2>/dev/null | grep -oE '[0-9a-f]{32}' | head -1 || true)
if [[ -n "$token" && -n "$account" ]]; then
  for d in fuselane.app www.fuselane.app; do
    curl -fsS -X POST -H "Authorization: Bearer $token" -H "Content-Type: application/json" \
      "https://api.cloudflare.com/client/v4/accounts/$account/pages/projects/$PROJECT/domains" \
      -d "{\"name\":\"$d\"}" >/dev/null && echo "Asked Cloudflare to serve $d" ||
      echo "$d: already added, or add it in the dashboard"
  done
fi
echo "Done: https://$PROJECT.pages.dev now, https://fuselane.app once the domain is active."
