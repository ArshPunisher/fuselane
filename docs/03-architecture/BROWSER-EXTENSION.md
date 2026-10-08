# Browser extension (`apps/extension`)

Built with WXT 0.21 (MV3) for Chrome, Edge, Brave and Opera, and Firefox. Safari later through the Xcode converter. The extension is a thin capturer; the work happens in the app.

## 1. What it does

| Capability | How |
|---|---|
| Automatic capture | `downloads.onCreated` (+ `onDeterminingFilename` on Chromium) → match rules → `downloads.cancel` + `erase` → send to the app |
| Rules | Minimum size (default 1 MB), extensions, include/exclude domains, MIME types. Holding **Alt** while clicking skips capture (a content-script click listener). |
| Context menu | "Download with Fuselane" on links, video and audio; "Send to Fuselane share link" (P1) |
| Popup | App status (connected or not), recent jobs, a capture on/off switch, pairing |
| Auth forwarding | `cookies.getAll({url})` **plus partitioned cookies** (`partitionKey`), Referer, User-Agent, Accept-Language; auth headers captured with `webRequest.onSendHeaders` (`extraHeaders`), held only in `storage.session` |
| Things it must not capture | `blob:` and `data:` URLs, POST-initiated downloads and single-use tokens: let the browser handle these |

## 2. Talking to the app

**Primary: native messaging.** `runtime.connectNative("app.fuselane.host")`.
- The host is the `fuselane` CLI in `--native-messaging` mode, relaying length-prefixed JSON to the local API.
- The app's installer writes the host manifests for Chrome, Chromium, Edge, Brave and Firefox, per user (paths in [`../01-research/tech-research.md`](../01-research/tech-research.md) §5.3; Windows uses HKCU registry keys).
- `allowed_origins` / `allowed_extensions` list only our store IDs.

**Fallback: localhost WebSocket with pairing** (for Flatpak/Snap browsers, Safari, or a missing host manifest).
- Listen on `127.0.0.1` only, on a fixed port range.
- Pairing: the app shows a 6-digit code (valid 2 min). The extension sends it and gets back a long random token kept in `storage.local`.
- Every request needs the token. Requests from website origins are rejected, and the Host header is checked (prevents DNS rebinding). Pairing attempts are rate-limited.

## 3. Messages (versioned schema, validated on both sides)

```jsonc
// extension → app
{ "v": 1, "type": "download.offer", "url": "https://…", "finalUrl": "…", "referrer": "…",
  "filename": "…", "mime": "…", "size": 123, "cookies": "a=b; c=d", "headers": {"Authorization": "…"},
  "userAgent": "…", "source": "auto|contextMenu" }
// app → extension
{ "v": 1, "type": "download.accepted", "jobId": "…" }
{ "v": 1, "type": "download.declined", "reason": "ip_locked|unsupported|user_cancelled", "fallback": "browser" }
```

The extension **pauses** the browser's download while it asks. When the app accepts, the browser's copy is cancelled and erased; when it declines, or doesn't answer within 3 s, the browser's download is **resumed**, so the user never loses one and keeps the browser's own session.

**v1 limits (2026-10-08):** cookies and auth headers are not forwarded yet (no host permissions), because the engine can't send them. To avoid saving a login page in place of the file, the app previews the link itself and accepts only when the size matches what the browser saw (`apps/desktop/src-tauri/src/api_bridge.rs`). Offers that do carry a session are declined. The host manifests (§2) are written once the extension has fixed store ids (STEPS 7.4, 7.9).

## 4. Security (see SECURITY.md)

- The app treats every message as untrusted: only http, https and magnet schemes; size caps on headers; the extension can't choose paths.
- Cookies and tokens stay in memory for the job's lifetime and are **never logged**. If the user enables "resume after restart" for an authenticated download, they are encrypted with the OS keychain.
- Permissions: `downloads`, `contextMenus`, `storage`, `nativeMessaging`, `cookies`. `<all_urls>` host permission is **optional**, requested at runtime with an explanation (eases store review).
- Store listings carry a privacy policy and a justification for each permission.

## 5. Browser differences

| Browser | Notes |
|---|---|
| Chrome / Edge / Brave / Opera | Service-worker background; `onDeterminingFilename` available; partitioned cookies on Chrome 119+ |
| Firefox | MV3 with event-page `background.scripts`; `browser_specific_settings.gecko.id` required; **no `onDeterminingFilename`**; blocking `webRequest` still available |
| Safari (P2) | `safari-web-extension-converter` → an app extension embedded in the macOS app; limited `downloads` support, so capture through link clicks plus the context menu |

## 6. Tests

- Unit: rule matching, message schema, cookie header building.
- e2e (Playwright, Chromium with `--load-extension`): a test page triggers downloads; assert the host or WebSocket mock receives URL, cookies and referrer; assert the browser fallback works when the app is absent.
- Firefox: `web-ext lint` plus a scripted run with `web-ext`.
- Manual on each store build: install from a store-like zip, check native messaging on all 3 OSes.
