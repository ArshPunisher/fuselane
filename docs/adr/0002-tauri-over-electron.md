# 0002. Tauri 2 desktop shell, not Electron
- Status: Accepted (to be confirmed by spike S7: Tauri window on all 3 OSes, incl. Linux NVIDIA/Wayland)
- Date: 2026-10-08

## Context
Plexo uses Electron: 80–150 MB installers, 150–300 MB idle RAM, and it still needed FFI (koffi) for socket options. A download manager runs in the background for hours.

## Decision
Use Tauri 2 (2.12.x) with a React 19 + Vite + Tailwind 4 UI. The Rust core runs in the same process.

## Consequences
- Installers around 10 MB and much lower RAM.
- Three different webviews (WKWebView, WebView2, WebKitGTK), so the UI is kept modest and virtualized, and tested on Linux in CI.
- No WebDriver on macOS, so macOS UI e2e relies on mocked-IPC Playwright tests, the packaged smoke test and manual runs.
- Electron stays a fallback if spike S7 shows WebKitGTK problems we can't work around.
