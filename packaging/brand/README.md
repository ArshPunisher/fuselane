# Fuselane brand

- `mark.svg`: the logo. Three network lanes (Wi-Fi cyan, phone lime, Ethernet violet) fuse into one fast lane in Fuse orange.
- `app-icon.svg`: the app icon (the mark on the graphite tile, macOS icon grid).
- `favicon.svg`: heavier strokes for 16 to 64 px.
- `render.mjs`: renders every PNG from these sources (`node render.mjs out`), including the 1200x630 share card.

Regenerate app icons: `node render.mjs out`, then `pnpm --filter @fuselane/desktop exec tauri icon ../../packaging/brand/out/app-icon-1024.png -o <dir>` and copy the desktop sizes into `apps/desktop/src-tauri/icons/`. Site icons go in `apps/site-next/public/`.
