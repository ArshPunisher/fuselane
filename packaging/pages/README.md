# GitHub Pages files

What https://arshpunisher.github.io/fuselane serves for apps already installed.
The website itself is https://fuselane.app (`apps/site-next`, Cloudflare Pages).

- `updates/latest.json` — the signed update feed, written by
  `.github/workflows/update-feed.yml` on each release (not stored here).
- `s/` — Fuse Send links made by older versions point here; it forwards to the
  same link on fuselane.app. The key stays in the `#fragment`, which browsers
  never send to a server.
- `index.html`, `404.html` — send visitors to fuselane.app.
- The sign-in check (`/probe`) needs no file: GitHub Pages answers any plain
  http request with a redirect to https, which is what the app checks for.
