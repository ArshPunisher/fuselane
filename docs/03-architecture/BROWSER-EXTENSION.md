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
- The host is **the desktop app's own binary** (or the `fuselane` CLI): started with the extension's origin as its argument, it relays length-prefixed JSON to the local API and never opens a window (`fuselane_api::native::run_host`). No second program to install.
- **The app writes the host manifests itself on every launch** (`fuselane_api::hosts`), because macOS (drag the DMG) and AppImage have no installer, and a moved or updated app must fix its own paths. Per user, only for browsers that are installed: Chrome (stable, Beta, Dev, Canary), Chromium, Edge, Brave, Vivaldi and Firefox on macOS and Linux; HKCU registry keys for Chrome, Chromium, Edge, Brave and Firefox on Windows (the NSIS uninstaller removes them). `fuselane browsers` does the same for CLI-only setups.
- An AppImage registers the `.AppImage` file (`$APPIMAGE`), not its temporary mount. An app running from the disk image or an App Translocation path registers nothing and says "move it to Applications" instead.
- `allowed_origins` / `allowed_extensions` list only our store IDs (Chrome `nggljghjikdkigiekdciocigdnnhponl`, Firefox `fuselane@fuselane.app`). `FUSELANE_EXTRA_EXTENSION_IDS` adds unpacked development ids (validated as Chrome ids).
- Flatpak and Snap browsers can't start hosts outside their sandbox; they need the localhost fallback below.
- Tested end to end on macOS (local gate) and Linux (CI): a real Chromium loads the built extension and pings the host (`apps/extension/e2e`). Chromium reads host manifests from inside its profile folder too, so the test never touches real browser folders.

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

**v1 limits (2026-10-08):** cookies and auth headers are not forwarded yet (no host permissions), because the engine can't send them. To avoid saving a login page in place of the file, the app previews the link itself and accepts only when the size matches what the browser saw (`apps/desktop/src-tauri/src/api_bridge.rs`). Offers that do carry a session are declined. The host manifests (§2) are written by the app on launch (STEPS 7.4).

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
