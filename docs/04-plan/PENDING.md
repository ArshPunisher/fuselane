# Pending: what's left, and who does it

The short list of open work, kept current. Details live in [STEPS.md](STEPS.md); this page
says what is waiting and on whom. Last updated 2026-10-10.

## Waiting on the owner

| What | Why it's needed | How |
|---|---|---|
| **Review and release beta.10** | Everything since beta.9 is built, tested and pushed, but not released | Look at the demo (`pnpm --filter @fuselane/desktop dev`, then http://localhost:5191) and the notes in `docs/release-notes/v0.1.0-beta.10.md`; say "release" and the agent bumps the version, tags, verifies and publishes |
| **Approve ADR 0013** (remote control for aria2 apps) | It adds a listening port (off by default; secret required; this computer only unless allowed) | Read `docs/adr/0013-aria2-remote-control.md`; say yes, or what to change |
| Deploy the site after the release | fuselane.app lists features that ship in beta.10 | `tools/deploy-site.sh` (the agent can run it once Cloudflare is logged in on this Mac) |
| Upload extension 0.2.0 with the new store images | The listing shows 0.1.0 | Upload `apps/extension` 0.2.0 zip and the images in `apps/extension/store` in the Chrome Web Store dashboard |
| Google Search Console Domain property for fuselane.app | Search indexing | Add the domain in Search Console (verified through Cloudflare DNS), then submit `https://fuselane.app/sitemap.xml` |
| Firefox Add-ons and Edge Add-ons listings (free) | Same, for Firefox and Edge users | Same zip flow; Firefox gets its own build (`build:firefox`) |
| SignPath approval (applied 2026-10-08) | Signed Windows installers | Wait for their email, then add the secrets |
| Renovate app (optional) | Automatic dependency updates | Install the free Renovate GitHub app on the repo |
| A phone to tether (Q10) | Real tests of USB tethering on macOS, Windows and Linux | Plug in a phone when convenient |

## Waiting on others

| What | State |
|---|---|
| librqbit PR [#699](https://github.com/ikatson/rqbit/pull/699) (handshake read in pieces) | Open, no reply yet. Fuselane already works around it. |
| Chrome Web Store review of item `nggljghjikdkigiekdciocigdnnhponl` (submitted 2026-10-09, version 0.1.0) | Pending; publishes automatically once approved. The app side shipped in beta.4. **Once 0.1.0 is live, upload extension 0.2.0** (signed-in downloads: optional `cookies` + `<all_urls>`, opt-in), so the first review isn't restarted. |

## Next for the agent (no owner action needed)

1. **Extension:** Alt-click to skip (needs a content script); forward cookies once the engine can send them. Rebuild the store zip so the listing ships the settings page.
2. **Extension media grabber** (IDM's video button): list direct video/audio files a page loads and offer them to Fuselane.
3. **Edge Add-ons id:** add it to `fuselane_api::hosts` if an Edge listing is made (Chrome and Firefox ids are done; Edge users can install from the Chrome Web Store meanwhile).
4. **Fuse Send (P6):** built end to end on loopback. Left: a real two-computer test over the internet (DHT + UPnP), UDP trackers as a fallback, per-peer "arrived", QR code and drag-and-drop on the Send page, folders (zip on the fly), 6.9 netlab e2e.
5. **Torrents:** a fixed connector in librqbit (5.9 part 2) so DHT and UDP trackers also go through Fuselane; incoming peers (L-71).
6. **P2/P3 leftovers:** sleep and wake, network change watcher per OS (torrents already follow changes), a stable network id, macOS friendly names, event deltas, guided setup and speed test.
7. **Real-machine checks:** Windows and Linux installers with the new "Open with" registration (CI builds them; never run on a real PC yet).
8. **Nearby on real devices:** two computers on one Wi-Fi, and a phone with LocalSend (interop is built from the protocol and tested between Fuselanes on loopback; never tried against the LocalSend apps yet). The phone page needs a real phone scan (checked in desktop Chromium and WebKit).

## Decided 2026-10-09

- **No plans and no hosted services** whose cost grows with users. Cloud uploads (R2 + Worker) are dropped and removed; the empty `fuselane-uploads` bucket was deleted. Sharing becomes Fuse Send, direct and peer to peer ([ADR 0011](../adr/0011-fuse-send-p2p.md)).
- macOS stays ad-hoc signed and installed from the DMG; no Apple Developer account (ADR 0009).

## Done recently (2026-10-08/09)

Torrents end to end (relay per network, path safety, file choice, credit, sharing, limits and allowances, Open with), sign-in page checks, the local API and native-messaging host, the extension (Chrome and Firefox builds, settings page), `--json` in the CLI, accessibility checks, and CI green on all three platforms again.
