# Spike S7: Tauri 2 shell

- Date: 2026-10-08 · Code: branch `spike/s7-tauri-shell` (`spikes/s7-tauri`, never merged) · Build: `pnpm install --ignore-workspace && npx tauri build`
- Status: **macOS part done.** Windows (WebView2) and Linux (WebKitGTK, including NVIDIA + Wayland) are pending: they need the GitHub CI runners or real machines.

## What was built

- A Tauri 2.12.1 app showing the S9 Lane Weave.
- **Rust pushes engine snapshots over `tauri::ipc::Channel` at 10 Hz.** A worst-case mode (`S7_FULL_BLOCKS=1`) adds a 10,000-block progress array to every message.
- A tray icon with Show and Quit, plus the single-instance plugin (registered first).
- CSP locked to `self` plus IPC; the capability grants only `core:default` and our own commands.
- The window logs its stats to a file every 3 s, so the packaged app can be measured without a person watching.

## Results (macOS 27.0.1, Apple Silicon)

| Metric | Result | Target |
|---|---|---|
| Release `.app` size | **5.76 MiB** (fonts included) | < 20 MB (Plexo's Electron build: 100 MB+) |
| App process memory (physical footprint) | **31–34 MB** | – |
| WebContent process memory | **42 MB** (deltas) / **49 MB** (full 10k array every tick) | – |
| Total | **~75–80 MB** | < 120 MB idle including webview |
| IPC cadence | One message every **104–105 ms** at a 10 Hz send rate | 10 Hz |
| Handler time per message in JS | **< 0.1 ms**, even with 10k blocks | – |
| Single instance | Second launch → still one process, window focused | ✅ |
| Tray menu | Works | ✅ |
| Code signature | `adhoc, linker-signed` by default | Matches ADR 0009 |

## Findings

1. **Tauri 2 meets the size and memory goals by a wide margin.** ADR 0002 can move to Accepted for macOS. It stays conditional on the Windows and Linux halves.
2. **Channel IPC is cheap** even in the worst case. Still send deltas (L-34): the full array cost about 7 MB more WebContent memory for no benefit.
3. **Memory attribution needs care on macOS.** WebKit's WebContent and GPU processes are XPC services shared by the system. One early reading of 185–217 MB came from a WebContent process that couldn't be tied to our app with certainty (Safari was running). Part 2 must attribute processes properly (responsible PID through `launchctl procinfo`, or Activity Monitor's grouped view) before quoting numbers publicly.
4. **The ad-hoc signature comes for free** from the Rust linker on Apple Silicon. The install script and Homebrew tap (ADR 0009) can ship this binary as is.
5. While the window was in the background, the measured draw time dropped to about 0 ms, because WebKit throttles animation frames for occluded windows. That's good for battery and confirms MOTION.md §5 "stop when hidden". The core keeps running regardless.

## Part 2 (pending)

- Windows 11 (x64, and ARM64 if possible): WebView2 memory, NSIS installer size, tray, single instance, Channel rate.
- Linux (Ubuntu 22.04/24.04, Fedora; X11 and Wayland; Intel/AMD and NVIDIA): blank-window workarounds (`WEBKIT_DISABLE_DMABUF_RENDERER=1` and others), draw time on WebKitGTK at 30/60 fps.
- macOS at DPR 2 on a Retina panel, with properly attributed memory.
