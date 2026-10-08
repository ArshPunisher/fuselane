# Pending: what's left, and who does it

The short list of open work, kept current. Details live in [STEPS.md](STEPS.md); this page
says what is waiting and on whom. Last updated 2026-10-09.

## Waiting on the owner

| What | Why it's needed | How |
|---|---|---|
| **Chrome Web Store: add and verify the publisher contact email** | The draft (item id `nggljghjikdkigiekdciocigdnnhponl`) is filled in and saved; this is the only thing the store says blocks submitting | Developer console → Settings → contact email → click the link in the verification email. Then say so, and the draft gets submitted for review |
| Firefox Add-ons and Edge Add-ons listings (free) | Same, for Firefox and Edge users | Same zip flow; Firefox gets its own build (`build:firefox`) |
| **R2 API token** (postponed by the owner) | Signing upload links (P6). Without it the service answers 503 "not set up" | Dashboard → R2 → Manage API Tokens → Object Read & Write on `fuselane-uploads` only → save into `apps/backend/.dev.vars` (never in chat) |
| Google Search Console verification (postponed) | Search indexing of the download page | Owner adds the site in Search Console and sends the verification tag |
| SignPath approval (applied 2026-10-08) | Signed Windows installers | Wait for their email, then add the secrets |
| Renovate app (optional) | Automatic dependency updates | Install the free Renovate GitHub app on the repo |
| A phone to tether (Q10) | Real tests of USB tethering on macOS, Windows and Linux | Plug in a phone when convenient |

## Waiting on others

| What | State |
|---|---|
| librqbit PR [#699](https://github.com/ikatson/rqbit/pull/699) (handshake read in pieces) | Open, no reply yet. Fuselane already works around it. |

## Next for the agent (no owner action needed)

1. **Extension:** Alt-click to skip (needs a content script); forward cookies once the engine can send them. Rebuild the store zip so the listing ships the settings page.
2. **Engine:** send request headers and cookies (lets the extension hand over logged-in downloads).
3. **Native-messaging host install** for Chrome, Firefox and Edge, per OS, once the store ids exist (7.4).
4. **Uploads (P6), once the R2 token exists:** spike S6 (one file over two networks to R2, checksums, equal parts), create the D1 database, deploy the Worker, device keys and quotas (6.2), `engine-upload` (6.3 to 6.5), encryption (6.6), share page download in the browser (6.7), link options (6.8), abuse handling (6.9), upload UI (6.10), receiving share links in the app (6.11).
5. **Torrents:** a fixed connector in librqbit (5.9 part 2) so DHT and UDP trackers also go through Fuselane; incoming peers (L-71).
6. **P2/P3 leftovers:** sleep and wake, network change watcher per OS (torrents already follow changes), a stable network id, macOS friendly names, event deltas, guided setup and speed test.
7. **Real-machine checks:** Windows and Linux installers with the new "Open with" registration (CI builds them; never run on a real PC yet).

## Done recently (2026-10-08/09)

Torrents end to end (relay per network, path safety, file choice, credit, sharing, limits and allowances, Open with), sign-in page checks, the local API and native-messaging host, the extension (Chrome and Firefox builds, settings page), the upload service's first cut (tested locally, not deployed), `--json` in the CLI, accessibility checks, and CI green on all three platforms again.
