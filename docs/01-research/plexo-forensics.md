# Plexo: forensic study for our own multi-network download manager

**Subject:** `/Users/arshramgarhia/Documents/sample/plexo` (read-only; nothing modified).

**Project facts:**
- Version `1.0.0-rc.14`, MIT licence.
- 263 commits between 2026-09-13 and 2026-10-07, tagged rc.1 to rc.14.
- Stack: Electron 44, electron-vite 5, React 19, Zustand, Tailwind 4 with shadcn/Base UI, webtorrent 3.0.21 (patched), and koffi for native calls.
- Tests: Playwright plus fast-check.

**Purpose:** learn the lessons, bugs, edge cases and design decisions, so that our own project (written from scratch, no code copied) doesn't repeat them.

**How to read it:** the report has seven parts plus a closing list of cross-cutting rules.
1. Feature inventory (our parity checklist)
2. Bug and lesson catalogue from git history
3. Error taxonomy, 4. hard-won edge cases, and 5. algorithms and constants (one combined part)
6. Test catalogue and CI
7. Weaknesses, tech debt and risks
8. Cross-cutting rules for us

Paths are relative to the repo root unless they are absolute. Short hashes are git commits in the plexo repo.

## Source map (largest files)

| File | Lines | Role |
|---|---|---|
| `src/main/download/downloadManager.ts` | 1722 | Lifecycle, queue, persistence, restore, notifications, sleep blocking |
| `src/main/download/httpTransfer.ts` | 1186 | HTTP run loop: streams, scheduler wiring, retries, hedging, crawl/silent detection |
| `src/main/download/chunkDownloader.ts` | 422 | One ranged request: validates the 206 / Content-Range, then writes to the staging file |
| `src/main/download/torrent/torrentTransfer.ts` | 434 | WebTorrent adapter |
| `src/main/network/routes.ts` | 418 | `StreamConnection`: DNS, Happy-Eyeballs, keep-alive agent, binding |
| `src/main/download/probe.ts` | 218 | First request: size, ranges, validators, filename |
| `src/main/download/concurrency.ts` | 202 | Streams-per-network controller, DiskWatch |
| `src/main/download/scheduler.ts` | 157 | Pure `pickWork` (primary and hedge selection) |
| `src/main/network/limits.ts` | 197 | Speed limits, data caps, usage accounting |
| `src/renderer/src/screens/DownloadsScreen.tsx` | 835 | The home list |
| `e2e/fixtures.ts` + `e2e/origin.ts` | 691 + 338 | Test harness and a fault-injecting origin server |



---

# Part 1: Feature inventory (parity checklist)

Read-only study, nothing modified. App version in `package.json` is `1.0.0-rc.14`. All paths below are relative to `/Users/arshramgarhia/Documents/sample/plexo`.

---

### 0. Architecture at a glance

- **Window** (`src/main/index.ts`): one `BrowserWindow`.
  - Size 760×640, min 720×620. `titleBarStyle: 'hidden'`.
  - macOS traffic lights sit at {x:16, y:9}. On Windows/Linux a `titleBarOverlay` uses the theme colours (dark `#202325`/`#eae7e2`, light `#fafafa`/`#1d1d1f`, height 44).
  - Background is `#1c1c1e` dark or `#ffffff` light. It is updated on `nativeTheme` `updated`, and the overlay is updated too on non-mac.
  - `autoHideMenuBar: true`. No custom app menu (`setApplicationMenu` is never called).
  - `optimizer.watchWindowShortcuts` from `@electron-toolkit/utils` enables F12 devtools in dev and blocks reload shortcuts in prod.
  - `setWindowOpenHandler` passes only `http(s)` URLs to `shell.openExternal` and denies everything else.
  - AppUserModelId is `com.plexo.app`. `app.setName('Plexo')`.
- **Renderer**: React 19 + Zustand store (`src/renderer/src/store/useAppStore.ts`).
  - There is no router. `view` is either `{name:'list'}` or `{name:'download', id}`.
  - `App.tsx` picks the screen by status:
    - queued, downloading or paused → `DownloadingScreen`
    - completed, or a history entry → `CompleteScreen`
    - error → `ErrorScreen`
    - cancelled → falls back to `DownloadsScreen`
  - Always mounted: `TitleBar`, `StatusBar`, `NewDownloadDialog`, `UpdateDialog`, `NetworkBindingDialog`.
- **Preload** (`src/preload/index.ts`): exposes only a typed `window.plexo`, never the raw ipcRenderer.
  - `initialState` is read with **sync** `ipcRenderer.sendSync` before first paint.
  - Also exposes `platform` and `pathForFile(file)` (`webUtils.getPathForFile`, used for drag-dropped `.torrent` files).

---

### 1. Direct HTTP downloads

#### 1.1 New download dialog (`src/renderer/src/components/NewDownloadDialog.tsx`)

**Opening and dismissal**
- Ways to open it:
  - the "New download" button in the list header or the empty state
  - ⌘N / Ctrl+N
  - pasting a link outside a text field
  - dropping a link or `.torrent` on the window
  - a link handed over by the OS
  - "Download again" on a failed download
- Modal, width up to 560px. `disablePointerDismissal`, so clicking outside does not close it.
- The form is mounted only while open, so every opening starts fresh.

**Link field**
- Placeholder: "Paste a link: https:// or magnet:". Monospace, autofocused, `aria-label="Link"`.
- A Clear (X) button with tooltip "Clear link" refocuses the input.
- **Clipboard autofill**: if the draft is empty on open, it reads the clipboard via IPC `readClipboardText`. It fills the field only if the text passes `acceptedLink` and differs from `startedUrl` (the link last started, so it is not offered again).
- `acceptedLink` (`src/renderer/src/utils/format.ts`) accepts:
  - `https?://` or `magnet:?`
  - an absolute POSIX or `X:\` path ending in `.torrent`
- The draft URL lives in the store (`draftUrl`), so it survives remounts.

**"Open .torrent…" button**
- Opens a native file dialog filtered to `*.torrent` (`chooseTorrentFile`) and puts the path in the link field.

**Probe**
- Debounced **600 ms** after the URL changes, then calls `probeUrl`. A stale result is discarded when the URL changes.
- States: idle → "Checking the link…" → ready, or error. On error the field gets a red border and an AlertTriangle message (passed through `describeError`).
- How `src/main/download/probe.ts` works:
  - Magnet → fetches metadata from peers.
  - Absolute `.torrent` path → reads the file.
  - Otherwise: a 1-byte Range GET following up to 5 redirects, User-Agent `Plexo/1.0`.
  - Ranges are supported only if the server answers **206**.
  - Size comes from `Content-Range` (or Content-Length on a 200).
  - A `416` with `bytes */0` is a valid empty file.
  - `Content-Type: application/x-bittorrent` or a URL ending `.torrent` → treated as a torrent; the `.torrent` is downloaded (max 10 MB).
  - Records ETag and Last-Modified for resume checks.
  - Status ≥ 400 → "Server responded with status N".

**File info row (when ready)**
- A file-type badge of 40px: the first 4 characters of the extension upper-cased, "FILE" if there is none, "DIR" for a multi-file torrent.
- **Editable filename** for HTTP downloads. Inline input, `aria-label="File name"`. Starts as `suggestedFileName`.
  - `suggestedFileName` comes from Content-Disposition (RFC 6266/5987: `filename*` wins, then quoted `filename`, then token), else the decoded last URL path segment, else "download".
  - A blank override falls back to the suggested name.
- Meta line, joined by " · ":
  - torrent file count
  - size, or "size unknown"
  - one of "resumable" / "Uses one connection. The file size is unknown." / "Uses one connection. This server doesn't support parallel downloads."

**"Save to" row**
- Shows `toDisplayPath` (paths under home become `~/...`).
- "Change…" opens a folder picker with `openDirectory` and `createDirectory`.
- The chosen folder is persisted as `destinationDir`.
- Default is the OS Downloads folder. A saved folder is used only if it still exists, checked at launch with a **300 ms** cap.

**"Networks" chip group**
- One pill per detected interface, coloured with the network's visual. `aria-pressed`.
- Starts from the last pick: networks saved with `off: true` start deselected. A newly detected network starts selected.
- At least one network must stay selected.
- A network that has used up its data limit:
  - is `aria-disabled` (deliberately not `disabled`, so the tooltip still works)
  - is struck through, and cannot be picked
  - has tooltip "Data limit reached"
- A network with a speed limit has tooltip "Limited to X".
- **Single-stream file** (no range support or unknown size):
  - only one network can be selected, and clicking a chip switches to it
  - note shown: "This download uses one connection on one network."
- **Subnet conflict warning**: two selected networks on the same IPv4 subnet → "A and B share a subnet (x), so the computer sends both down one route and they can't be combined."
- **Data-limit warnings**:
  - "X has/have reached its/their data limit. Choose another network for this download." (when the remembered pick is all used up; nothing is auto-substituted)
  - "Every network has reached its data limit. Raise one in Speed & data limits."

**"Streams" toggle (HTTP, splittable only)**
- Values: Auto, 4, 8, 16, 32, labelled "per network". Default Auto.
- Not remembered: the next download starts on Auto again.
- Items are 24px high (meets WCAG 2.5.8).

**Footer and Start**
- Shows an inline start error. Buttons are Cancel and Submit.
- Submit label is one of "Starting…", "Checking…", "Add to queue" (when downloading count ≥ downloadsAtOnce) or "Download".
- `canStart` requires: probe ready, at least 1 torrent file chosen (for a torrent), at least 1 network, a destination, and not already starting.

**After a successful start**
- Saves the `off` flag per network to remember the pick. This is skipped for a single-stream file.
- Sets `startedUrl`, clears the draft, and opens the download's own screen, unless it was only queued.

#### 1.2 Server-side start validation (`src/main/download/downloadManager.ts`, `start()`)
- Rejects with "Select at least one network" when none is valid.
- Creates the destination directory.
- `ensureDiskSpace` (via statfs): "Not enough disk space: this download needs X GB but only Y GB is free".
- Resolves the host and filters out networks that cannot route to it (`NoCompatibleRouteError`).
- Block planning (`src/main/download/plan.ts`):
  - maximum block size 8 MiB, minimum 1 MiB, at least 2 blocks per stream
  - Auto starts at 8 streams per network and doubles once all are receiving, up to 32
- Backoff (`src/main/download/concurrency.ts`): if the server refuses (403/429/503/no answer), that network drops to the streams it accepted, then regains one stream every 60 s. A slow disk also caps streams (`diskLimited` flag).

#### 1.3 Filename conflicts and staging (`src/main/download/paths.ts`)
- Name sanitizing:
  - `/` and `\` become `_`; control characters become `_`
  - on Windows, `<>:"|?*` become `_` and trailing dots/spaces are stripped
  - reserved names (CON, PRN, AUX, NUL, COM1-9, LPT1-9) get a `_` prefix
  - an empty or dots-only name becomes "download"
- Conflicts: `name.ext`, then `name (1).ext`, `name (2).ext` … up to 10,000 attempts. The name is trimmed to fit 255 bytes including ` (9999)` and the suffix.
- HTTP downloads write to `<final>.plexo`, created exclusively and marked sparse on Windows, then renamed to the final name when complete.
- Torrents claim the final name (a folder or a file) directly. A folder name has no extension, so it becomes "Show.S01 (1)".

#### 1.4 Fix link / relink (`src/renderer/src/components/FixLinkDialog.tsx`; `relink` in the manager)
- Triggered when an HTTP download is in error and its message matches `status (401|403|404|410) for range request`, shown to the user as "This link no longer works. Paste a new link to continue."
- Dialog:
  - title "Paste a new link"
  - description "Paste a new link to the same file: NAME (SIZE). Plexo resumes from where the download stopped."
  - input placeholder `https://…`
  - button "Continue download", or "Checking…" while working
- Server-side checks:
  - HTTP only
  - status must be error or paused
  - must be resumable
  - the new probe must be HTTP, with the **same size** ("That link is to a different file: X GB, not Y GB")
  - range support is required if the download was splittable
- Then it resumes.

#### 1.5 Pause / resume / resilience (main)
- Pause: streams are paused, in-flight blocks go back to pending, and the transfer is reset and persisted.
- Pausing a queued download makes it paused and takes it out of the queue.
- Resume:
  - waits for the old run to finish, refreshes the networks, and checks that the partial file still exists ("The partial download file is unavailable. Reconnect the destination drive and try again.")
  - if all slots are busy, it is queued **at the front**
  - otherwise it begins
  - the version check (ETag/Last-Modified) happens on the first chunk; a changed file makes it fail as non-resumable
- Switching off the **last** network of a running download pauses it. Switching one back on resumes it.
- A download that can't be split runs on exactly one network; switching another on moves it there.
- `powerMonitor` `resume` restarts streams. Address changes on a network restart that network's streams.
- Retries: up to 5 per chunk with backoff capped at 15 s. A 5 s retry when unreachable. `Retry-After` honoured up to 120 s. Hedging ("BACKUP" stream) near the end.
- On quit (`before-quit`): every running download is paused and persisted (`suspendAll`), with a 3 s forced exit. At the next launch they come back paused.

---

### 2. Torrents

- **Inputs**: a magnet link, an http(s) link to a `.torrent`, a local `.torrent` path (from the Open button, drag-drop, OS "Open with", or the command line).
- **Magnet metadata** (`src/main/download/torrent/metadata.ts`):
  - fetched with a long-lived probe client (keeps the DHT warm)
  - a newer magnet cancels the older lookup
  - timeout message: "No peers responded to this magnet link. Try again later or open a .torrent file."
  - invalid magnet: "This magnet link isn't valid"
  - the last 8 probed `.torrent` files are cached by infoHash; starting a torrent needs that cache ("Add the torrent again to start downloading.")
- **Limits and errors**: `.torrent` files up to 10 MB ("Plexo supports .torrent files up to 10 MB."). v2-only torrents are rejected ("This torrent uses BitTorrent v2 only, which Plexo doesn't support yet"). Other parse failures: "This isn't a torrent Plexo can read".
- **Path safety** (`src/main/download/torrent/paths.ts`): characters `<>:"/\|?*` and control characters are stripped, Windows reserved names are handled, and names are capped at 255 bytes.
- **Engine** (`src/main/download/torrent/engine.ts`): WebTorrent with uTP, web seeds, WebRTC, UPnP, NAT-PMP and LSD all **off**. The DHT is controlled by a test knob. Peers are found via HTTP/UDP trackers and DHT, and every peer is pinned to one network's address. Up to 30 peers per network. A network is marked unreachable after 5 attempts or 60 s.
- **File selection before start** (`TorrentFileList` in `src/renderer/src/components/TorrentFiles.tsx`):
  - a scrollable box (max-h 44), monospace
  - a folder header row whose checkbox is tri-state and selects all
  - per-file checkbox, path inside the torrent, size
  - unticked files are sent as `selectedFiles` (indexes); the field is omitted when everything is chosen
  - Start is disabled when 0 files are chosen
  - the meta line shows "N files" or "X of N files", and the size counts only chosen files
- **Changing files while it runs** (`TorrentFiles`):
  - opened from the "N files ▼" chip on the Downloading screen
  - each file shows Skipped, Done or a %
  - a finished file's checkbox is disabled, and it stays chosen
  - unticking every file gives the error "Keep at least one file."
  - server-side errors: "This download has no files to choose", "This download's files can't change now", "A file that's already downloaded stays", and an insufficient-disk-space check for files being added
- **Pieces**: the block grid shows pieces, including a `skipped` state (dashed, 30% opacity) and unverified bytes held as `provisionalBytes`.
- **Peers**: rows show "Peer #N", the client name chip (e.g. "qBittorrent 4.6.2"), ↓/↑ speeds and received/sent bytes. Each peer's Share column is its share of that network's total.
- **Uploading and seeding**:
  - Uploads happen on every network **while the download runs**. They are shown per network and per peer, and the Complete screen shows "uploaded X".
  - **There is no seeding after completion**: the client is destroyed when the run ends, and there is no seeding UI or setting.
- **Data limits** count torrent download bytes. Uploads don't count.
- **Torrent badge**: a neutral outline "TORRENT" badge next to the name in the list and on the Downloading screen (`src/renderer/src/components/TorrentBadge.tsx`).

---

### 3. Queue and scheduler

- `downloadsAtOnce`: default **2**, minimum 1, maximum 8 (`DOWNLOADS_AT_ONCE` in `src/shared/types.ts`).
- When a new download starts and slots are full, it goes to the back of the queue (`queuedAt` = max of existing + 1). Resuming a download by name puts it at the front.
- `pump()` starts queued downloads in `queuedAt` order whenever a slot frees, a queued one leaves, or the count is raised.
- Lowering the count does not stop running downloads.
- Time spent queued counts as paused time (`pausedAt`/`totalPausedMs`), so the elapsed time on screen excludes it.
- In the UI:
  - status "queued"
  - the list's "Queued" group, sorted by `queuedAt`
  - row text "Waiting for a turn · sizes"
  - a QUEUED badge on the detail screen
  - the footer shows "N waiting"
  - the New download submit button reads "Add to queue"
- **No manual reordering of the queue.**

---

### 4. Downloads list and history (`src/renderer/src/screens/DownloadsScreen.tsx`)

**Header (no selection)**
- Filter menu (`src/renderer/src/components/DownloadFilterMenu.tsx`), a Base UI Menu radio group inside an `<h1>`. Options, each with a count:
  - All downloads
  - In progress (queued, downloading, paused)
  - Finished
  - Needs attention (error)
- Changing the filter clears the selection.
- A summary appears only when there is more than one group, e.g. "2 downloading · 1 queued · 1 paused · 1 need attention", or "all done".
- `NetworksMenu` button (§7) and a "+ New download" button.

**Groups**, in order and only when non-empty:
- Downloading
- Queued (sorted by queue order)
- Paused
- Needs attention
- Finished (completed but not yet in history, then history newest first)
- Each group header has a select-all checkbox (tri-state), a label and a count.
- The Finished group has "Clear finished list" with tooltip "Downloaded files stay on your computer". This calls `clearHistory`; files are kept.
- Running downloads are sorted by `startedAt` ascending. Cancelled ones are hidden.

**Row (`DownloadRow`, memoized)**
- Checkbox, then a button that opens the detail view. The button contains:
  - the file-type badge ("DIR" for a torrent folder)
  - the name, plus the TORRENT badge
  - network dots, shown only when some network is off, with a tooltip grid of name and state (Off / status text / speed / On)
  - live speed while downloading
  - a segmented progress bar: each network's share in its colour; red on error; 40% opacity when paused or queued; none when finished
  - a detail line
- Detail line by status:
  - downloading: "NN% · X of Y · ETA"
  - queued: "Waiting for a turn · …"
  - paused: "Paused at NN% · …"
  - error: the described error, in red
  - finished: "size · host (HTTP only) · when", or "moved or deleted"
- Per-row action:
  - Pause (icon), for downloading or queued
  - Resume (Play icon), for paused
  - error with an expired link → "Fix link"
  - error that is resumable → Retry (RotateCw icon)
  - otherwise "Download again", which removes the entry and opens New download with its URL
- A chevron button also opens the detail view.
- `formatWhen`: "Just now", "N min ago", "Today HH:MM", "Yesterday HH:MM", or "D Mon YYYY". The clock ticks every 30 s.

**Selection toolbar** (`role="toolbar"`, `aria-busy`)
- "Deselect all" (X), "N selected", and "Select all" (applies within the current filter).
- Actions, each with a count. They run one item at a time; a failure shows a `role="alert"` bar.
  - **Pause (n)**: downloading or queued
  - **Resume (n)**: paused
  - **Retry (n)**: error, resumable, and the link not expired
  - **Remove from list (n)**: finished; no confirmation
  - **Cancel downloads… (n)**: unfinished; destructive and confirmed. Confirm text: "This stops the selected unfinished downloads and deletes their downloaded data. Finished downloads stay unchanged."
  - **Move files to Trash/Recycle Bin… (n)**: finished, not missing, and for a torrent folder from history it needs `downloadedFiles`. Confirmed: "This moves the selected finished downloads' files to the Trash and removes them from the list. Unrelated files stay in place."
- The wording is "Recycle Bin" when `platform === 'win32'`.

**Empty states**
- No downloads:
  - a muted `CombineDiagram` with 3 placeholder networks
  - "No downloads yet" and "Paste a link (⌘V/Ctrl+V) or drop a .torrent anywhere in this window."
  - a "New download ⌘N" button
- No networks at all:
  - "No networks connected" and "Plexo needs at least one active network. Join a Wi-Fi network, plug in Ethernet, or connect your phone using USB tethering."
  - buttons "Scan again" (`loadInterfaces`) and "Network settings…" (`openNetworkSettings`), which opens:
    - Windows `ms-settings:network-status`
    - macOS `x-apple.systempreferences:com.apple.preference.network`
    - Linux `gnome-control-center network || nm-connection-editor`
- Filtered and empty:
  - "No downloads in progress" / "No finished downloads" / "No downloads need attention"
  - a "Show all downloads" button

**History (`src/main/download/history.ts`)**
- Capped at **500** entries, newest first; the oldest are dropped.
- Entries are validated on read.
- `missing` is computed by `stat` on every list.
- The history list is re-read on window `focus`, and 150 ms after the debounced `historyChanged` push.
- Reveal ("Show in Finder" / "Show in folder"):
  - main checks that the path exists, then calls `shell.showItemInFolder`
  - if the file is gone, it re-sends history and the entry is marked missing
- Trash (`src/main/download/trashDownload.ts`): `shell.trashItem`. For a torrent folder, only the files it downloaded are trashed.

---

### 5. Per-download detail views

#### 5.1 DetailHeader (`src/renderer/src/components/DetailHeader.tsx`)
- "‹ Downloads" back button, then the Remove button, then a screen-specific primary action.
- Remove is an AlertDialog:
  - unfinished downloads: "Cancel download…" (destructive). Title "Cancel download: NAME?", text "This stops the download and deletes its downloaded data."
  - finished downloads: "Remove from list…". Title "Remove from list: NAME?", text "This removes the download from your list. Its files stay on your computer."
    - an optional checkbox "Also move downloaded files to the Trash/Recycle Bin. Unrelated files stay in place.", hidden if missing or if a torrent folder has no `downloadedFiles`
    - the action label changes to "Move files to Trash"
    - errors are shown inline
    - on success it returns to the list

#### 5.2 DownloadingScreen (`src/renderer/src/screens/DownloadingScreen.tsx`), for queued, downloading and paused
- **Primary button**: Pause / Resume / "Resuming…". Resume is the primary variant.
- **Window title**: "Plexo — NN%", "Plexo — downloading", "Plexo — Paused (NN%)" or "Plexo — Queued (NN%)". Reset to "Plexo" on unmount.
- **HeroBand** (`src/renderer/src/components/HeroBand.tsx`): a gradient panel driven by `--hero-bg`.
  - `CombineDiagram` (`src/renderer/src/components/CombineDiagram.tsx`): SVG with each network's label and speed, dashed curves animated by `plexo-dash` and staggered per network, merging into one thick stream. Dims when paused. The label column grows with the name (up to 18 characters).
  - **TOTAL SPEED**:
    - a big 38px number with its unit
    - label suffix " · SLOW MODE X" or " · LIMIT X"
    - a "Slow drive" amber warning icon when `diskLimited`, with tooltip "Your drive can't save any faster, so this download is going slower."
    - "—" when paused
  - "AVG X · PEAK Y". Peak is the best speed held for 5 s; "—" until then.
  - Under the speed, either the error, a "waiting" message, or the cyclable chip:
    - waiting messages: "Can't reach the server. Retrying…" / "Can't reach peers. Retrying…", "Every network in use has reached its data limit. Raise one in Speed & data limits.", "Waiting for a network. Reconnect one or switch one on."
    - **CyclableChip** (`src/renderer/src/components/CyclableChip.tsx`): "N.N× WIFI ALONE" in the network's colours, only when ratio ≥ 1.05 and more than one network is in play. Clicking cycles through the networks, and a ⇄ hint shows when there is more than one. Tooltip "Total speed is N× faster than X alone (click to toggle)".
  - **THROUGHPUT · LAST 60S** with the ThroughputChart (§5.4). 45% opacity when paused.
- **File info row**:
  - 44px icon, or a folder icon for a torrent folder
  - `TruncatedText` name, plus the TORRENT badge
  - "X of Y · NN% · N peers · [N files ▼ chip] · ETA left · PAUSED/QUEUED badge · N retries"
  - the folder path, right-aligned, with tooltip "Saving to: full path"
- **BlockGrid** (§5.3).
- **Network table** (`role="table"`):
  - sticky column headers: Status, Network, Progress (for a torrent it has a tooltip "Percentage of the download completed with verified data from this network."), Share, Speed, Downloaded / Transferred
  - one `NetworkRow` per network (§7.2), including networks that are off

#### 5.3 BlockGrid (`src/renderer/src/components/BlockGrid.tsx`)
- One 13px-tall square per 8 MB chunk (or per piece), with a 3 px gap and at least 8 columns. Width-driven wrapping via ResizeObserver.
- At most 4 visible rows, then it scrolls. Rendering is virtualized with 2 buffer rows.
- Square colour:
  - the network that delivered the most bytes, from `bytesByInterface`
  - unknown origin is grey (`--text-tertiary`)
  - downloading squares glow in the network colour; the glow is off when paused (0.6 opacity)
  - completed squares are 0.92 opacity
  - skipped squares are dashed at 0.3
  - the fill width has a 6% minimum once started
- Legend shows network dots and names.
- Readout on the right: "N chunks · 8.0 MB each". On hover it becomes "Chunk #N · X verified [· Y received, awaiting verification] / Z · WiFi 60% · USB 40%" (or queued / "skipped: in no file chosen").
- A floating ↑/↓ button "Show the chunks/pieces in progress" scrolls to the nearest active row. It respects `prefers-reduced-motion`.
- For a single-block or unknown-size download it falls back to a segmented bar: network shares plus what remains, or a solid primary bar when the size is unknown.

#### 5.4 ThroughputChart (`src/renderer/src/components/ThroughputChart.tsx`)
- A stacked SVG area chart, one layer per network at fill opacity 0.62, with a total outline in `--node-accent`.
- Fixed 60 s window, one sample per second, newest at the right.
- Y-axis ticks from `speedTicks`: 2-3 gridlines at steps of 1, 2, 2.5 or 5 ×10ⁿ, labelled in the current unit.
- X labels: "60s ago … now", or "60s before the pause/end … pause/end".
- Hover shows a crosshair and a popover with each network's speed at that second plus a Total. The popover flips side past the midpoint.
- `aria-label`: "Combined throughput over the last minute. Currently X, fastest Y."

#### 5.5 CompleteScreen (`src/renderer/src/screens/CompleteScreen.tsx`)
- Primary button "Show in Finder" (macOS) or "Show in folder", disabled when the file is missing.
- `sr-only` `role="status"` text: "Download complete: NAME".
- Hero: a check icon, the name, "N files · size · folder" (or "moved or deleted since"), and AVERAGE speed (total size divided by active time).
- A 5-cell stat strip:
  - Size
  - Time (`m:ss` or `h:mm:ss`)
  - Peak
  - Networks (count that contributed)
  - Streams or Peers (peak count)
- "Speed over the download": the chart with `endsAt="end"`.
- "Contribution by network": a segmented bar plus rows of name, bytes and %.
- Footer: "written in N chunks/pieces · uploaded X (torrent) · N retries".

#### 5.6 ErrorScreen (`src/renderer/src/screens/ErrorScreen.tsx`)
- Shown for error. A cancelled download actually falls back to the list (see App.tsx), so its branch is mostly unreachable.
- Heading "Download failed", or "Download cancelled".
- Description: `describeError(error)`, or "The download stopped unexpectedly. Try again."
- Primary action, one of:
  - "Fix link" (expired link)
  - "Retry" / "Retrying…" (resumable with bytes greater than 0)
  - "Download again" (removes the download, goes to the list, opens New download with the URL)
- A file capsule showing "X of Y (NN%)" or "No data downloaded".
- The URL, truncated, with a "Copy link" button that shows "Copied" (in green) for 2 s.

#### 5.7 Error translation (`src/shared/errors.ts`, `describeError`)
- Strips the Electron IPC prefix and maps raw errors to plain text:

| Raw error | Message shown |
|---|---|
| ENOSPC / EDQUOT | not enough space |
| EACCES / EPERM | permission |
| ENOENT | missing file or folder |
| EROFS | read-only drive |
| EIO | couldn't read or write |
| invalid torrent or magnet | can't read this torrent |
| 401/403/404/410 on a range request | link no longer works |
| "Download is incomplete" | couldn't download every part |
| size mismatch | unexpected size |
| ENOTFOUND / EAI_AGAIN | couldn't find the server |
| ECONNREFUSED | server refused |
| ECONNRESET | connection interrupted |
| ETIMEDOUT | timed out |
| CERT / SSL / TLS | certificate couldn't be verified |
| invalid URL | not a valid link |
| status 401 / 403 / 404 / 4xx / 5xx | matching text |

- Anything else is shown raw.

---

### 6. Limits: speed, data, slow mode (`src/renderer/src/components/LimitsDialog.tsx`, `src/main/network/limits.ts`)

**Dialog layout and saving**
- Title "Speed & data limits". 680px wide, height `min(520px, 100%-2rem)`. Pointer dismissal is off.
- Opened from the NetworksMenu, either on a network's page or the General page.
- **Edits a draft**: nothing applies until **Save**. Cancel or Escape discards.
  - Only changed fields are written.
  - "Reset data usage" is the exception: it acts immediately once confirmed.
  - **The README says "Changes apply as you make them", which is wrong.**
- Left nav (190px):
  - "General → All downloads", with the line "Slow mode on" / "Limit X" / "No limit"
  - "Networks": one item per interface, with lines such as "Limit X" and "11.7 of 50 GB", or "Data limit reached" in red, or "No limit"

**General page ("All downloads")**
- Header line: "N downloading right now", or "No active downloads".
- **Total speed**: a radio pair, "No limit" or "Limit to [input] MB/s|Mbps". The fallback value when first enabled is **20 MB/s**. The last value is kept while the dialog is open.
- **Slow mode**: a speed input plus an "Enabled" switch.
  - default speed `DEFAULT_SLOW_MODE_SPEED` = 2 MiB/s
  - text: "Replaces the total speed limit while enabled. Useful during calls or streaming."
- **Downloads at once**: a −/+ stepper from 1 to 8 with `<output>`. Text: "Other downloads wait in the queue and start automatically."
- **"Reset to defaults…"**: disabled when everything is already at defaults. Confirm text: "This resets total speed, slow mode and downloads at once. Network limits stay unchanged." Defaults are: no speed limit, slow mode off, slow speed 2 MiB/s, 2 at once.

**Network page**
- Header shows the name and "X used by Plexo today / this week / this month".
- **Speed**: "No limit" or "Limit to …", fallback **10 MB/s**. Text: "Limits the combined download speed on NAME."
- **Data limit**:
  - "No limit" or "Limit to [n] GB", fallback **5 GB**
  - "per" Day / Week / Month toggle (default month), disabled when there is no limit
  - text: "Stops downloads on NAME at the limit. Uploads and other apps don't count."
  - a UsageBar, "X of Y this month", that turns red once reached
  - reset text: "Resets tomorrow at midnight" or "on D Month", plus "Weeks start Monday." and "Raise the limit or reset data usage to keep using this network sooner."
- **"Reset data usage…"**: disabled when usage is 0. Sets only the current period to 0 and confirms first.
- **"Remove limits…"**: clears speed and data limits, keeps usage, and confirms first.

**Number input rules (`NumberInput`)**
- `inputMode="decimal"`. Accepts only values that become a positive safe-integer number of bytes; invalid text is ignored.
- Displayed to 2 decimals when ≥ 1, otherwise 3 significant digits.
- Speed is typed in the **footer's unit**: MB/s means 1024² bytes, Mbps means 10⁶/8 bytes.
- **The README says "KB/s or MB/s", which does not match the code.**

**Engine**
- A token bucket holding at most 1 s of tokens.
- The total limit and the per-network limit both apply; the larger wait wins.
- Slow mode replaces the total limit while on; per-network limits still apply.
- Usage is counted on every received byte, for all three periods at once.
- Calendar periods use local time, weeks start Monday (`src/shared/dataLimits.ts`). Period keys are `YYYY-MM` for a month and `YYYY-MM-DD` for a day or week.
- Usage is saved every 10 s and on quit.
- Crossing the limit fires `onLimitReached` → reconcile, and the network status becomes `limit`.

---

### 7. Networks UI

#### 7.1 Detection (`src/main/network/interfaces.ts`)
- Polled every **1 s**, and pushed to the renderer with `networksChanged` on any change.
- Excluded: loopback/internal addresses, 169.254.x, and fe80:: addresses. A network needs at least one usable IPv4 or IPv6 address.
- Display names:
  - macOS: the `networksetup -listallhardwareports` name (e.g. "Wi-Fi", "iPhone USB")
  - Windows: the adapter alias, with kind refined by NDIS media type
  - Linux: the device name
- Kind classification: wifi, usb (rndis/tether/iPhone/`enx`/`usb`), ethernet, bridge or other.
- `id` is the OS device name.

#### 7.2 NetworkRow (`src/renderer/src/components/NetworkRow.tsx`), per download
- Status dot:
  - glows (`plexo-glow` 1.8 s) when active
  - red on failure
  - 0.65 opacity when idle
- A **checkbox** to enable or disable the network for this download (`setDownloadNetwork`). It is filled in the network's colour. `aria-label="Use NAME"`.
- The name (truncated), a pencil button that opens `NetworkEditPopover`, and an "N streams/peers ▼" expander.
- A status word when not on, with the error in a tooltip:
  - Off
  - Not connected
  - Can't reach server, or Can't reach peers
  - Failed
  - Data limit reached
- Columns:
  - progress bar (this network's bytes over the total)
  - Share %, shown as "<1%" for a sliver
  - Speed: "X / limit" with a quieter limit and tooltip "Limited to X" when a per-network limit exists; torrents always show both ↓ and ↑
  - Downloaded, or ↓/↑ for a torrent
- Expanded HTTP stream rows:
  - "Stream #n"
  - a "Chunk #n" chip
  - a "BACKUP" badge (tooltip "Racing another stream for this chunk, which was running slowly") or the state Done / Paused / Retrying… / Idle
  - a per-chunk progress bar, share, speed and bytes
- Expanded peer rows: as in §2.
- Grid columns: `30px minmax(190px,max-content) 1fr 48px minmax(104px,max-content) 160px`.

#### 7.3 NetworksMenu (`src/renderer/src/components/NetworksMenu.tsx`), a popover in the list header
- Trigger:
  - up to 4 coloured dots
  - "N networks", or a red "No network"
  - the combined live speed across running downloads
- One row per interface:
  - dot, custom name, pencil (edit), and an OS device-name chip (max 110px, truncated with a tooltip)
  - status on the right: "Paused" (red, limit reached), the live speed, or "Idle"
  - "Limit X" line, and a data usage line with a mini bar
  - the whole row (via a chevron button stretched over it) opens that network's limits page
- Footer link "Speed & data limits…" opens the General page.
- Usage is polled every 2 s while the menu is open (`useNetworkUsage`).

#### 7.4 NetworkEditPopover (`src/renderer/src/components/NetworkEditPopover.tsx`)
- Name: max **40** characters, placeholder is the OS name, autofocused, text selected on focus. Enter saves.
- Saving: trimmed. Equal to the OS name or empty clears the custom name.
- Colour: 8 swatches (Teal, Amber, Steel, Rose, Violet, Lime, Cyan, Coral), each a 24px hit target with a ring when selected.
- Saves on Done, Enter, or clicking away. **Escape discards.**
- Re-picking the auto-assigned colour does not pin it.

#### 7.5 Colour assignment (`src/renderer/src/theme.ts`)
- Order of precedence:
  1. a user-pinned `colorId` always wins
  2. otherwise the kind's own colour if still free (wifi → teal, usb → amber, ethernet → steel)
  3. otherwise the first unused swatch
- Bridge and other kinds always take a free swatch.
- Assignment is ordered by id, across detected networks and all networks seen in downloads and history.
- The kind's own colour uses a hand-tuned palette. Any other swatch is tinted with `color-mix` in oklch (16% bg, 42% border, 62% text).
- Kind labels: WIFI, USB, ETH, NET.

#### 7.6 NetworkBindingDialog (`src/renderer/src/components/NetworkBindingDialog.tsx`)
- Shown when device binding is unsupported (Linux kernel older than 5.7, no `SO_BINDTODEVICE`) and there is more than one network.
- Title "Only your default network can be used", with an explanation. Single button "Got it".
- Dismissal lasts for the session only and is not persisted.

---

### 8. OS integration

- **Single instance**: `requestSingleInstanceLock`, otherwise exit. On `second-instance`, an accepted link in argv is offered; otherwise the window is restored and focused.
- **Links handed over** (`src/main/openLinks.ts`):
  - accepted: `magnet:?…`, or an absolute path to an existing `.torrent` file
  - sources: macOS `open-url` and `open-file` events (may arrive before ready); Windows/Linux argv at launch and on second-instance
  - kept as `pending` until the renderer calls `takePendingLink`
  - the window is restored, shown and focused, and `linkReceived` is sent
  - the renderer opens New download with the link prefilled; **downloads never auto-start**
- **File associations and protocols** (`electron-builder.yml`):
  - macOS: `CFBundleURLTypes` magnet, and `CFBundleDocumentTypes` Torrent (role Viewer, `LSHandlerRank: Alternate`, ext `torrent`, UTI `org.bittorrent.torrent`), so Plexo never becomes the default handler
  - also `NSDownloadsFolderUsageDescription`
  - Linux: `mimeTypes: [x-scheme-handler/magnet, application/x-bittorrent]`
  - **Windows: intentionally none** (no protocols or fileAssociations); "Open with" still works
- **Packaging**:
  - Windows: NSIS assisted installer (`oneClick: false`, install directory can be changed), one exe for x64 and ARM64 named `plexo-<ver>-setup.exe`, desktop shortcut always created
  - `build/installer.nsh` adds a custom finish page with "Run Plexo" and a "Create a desktop shortcut" checkbox (checked by default; unchecking deletes the shortcut; skipped on silent installs)
  - macOS: dmg for x64 and arm64, not notarized
  - Linux: AppImage, deb and rpm for x64 and arm64
  - `publish` is a placeholder generic URL (`https://example.com/auto-updates`); there is no auto-update
- **Sleep prevention**: `powerSaveBlocker.start('prevent-app-suspension')` while any download is `downloading`, stopped otherwise (`keepAwake`, checked on every push). The display can still sleep.
- **Notifications** (Electron `Notification`, when supported; disabled in tests):
  - "Download complete" — "NAME has finished downloading."
  - "Download failed" — "NAME: <described error>"
  - Clicking a notification restores and focuses the window.
- **Trash**: `shell.trashItem` (§4).
- **Reveal**: `shell.showItemInFolder`.
- **Dock/taskbar progress or badge: NOT implemented.** No `setProgressBar` and no badge. The only progress shown outside the page is `document.title`.
- **Menu**: none custom. No tray.
- **Wake from sleep**: `powerMonitor` `resume` triggers `systemResumed` and a network refresh.
- **Quit**: graceful suspend, with a 3 s forced exit. `window-all-closed` quits on non-mac.
- **Clipboard**: read via main (`readClipboardText`). Copy-link on ErrorScreen uses `navigator.clipboard`.
- **Drag and drop**: anywhere in the window, a `.torrent` file (path via `webUtils`) or a dropped text link. Default navigation is prevented.

---

### 9. Update check

- `src/main/updateCheck.ts`: `net.fetch('https://api.github.com/repos/anmolkapil/plexo/releases?per_page=1')` reads the newest release, prereleases included.
  - The version is compared numerically by segment after splitting on `.` and `-` (not full semver: rc.N is a 4th segment).
  - It runs **once at startup** in `registerIpcHandlers`, and the result is cached.
  - Any failure, rate limit or offline state → null, silently.
  - The URL shown to the user is always `https://getplexo.app/`.
- The `checkForUpdate` IPC returns `{version, url, dismissed}`. `dismissed` is `version === settings.dismissedUpdateVersion`, read on every call.
- UI:
  - `UpdateDialog` (`src/renderer/src/components/UpdateDialog.tsx`): AlertDialog "Plexo X is available" / "A new version is ready to download." Buttons "Not now" and "Download" (an `<a target=_blank>` routed to the OS browser; it has initial focus). Closing dismisses and persists `dismissedUpdateVersion`.
  - Once dismissed, `UpdateIndicator` (`src/renderer/src/components/UpdateIndicator.tsx`): a 26px CircleArrowUp link in the footer, tooltip "New update available", aria-label "Update available: X".
- Nothing downloads or installs itself.

---

### 10. Theming

- `ThemeSource` is `'light' | 'dark'` only. 'system' was removed; a saved 'system' is dropped by the sanitizer.
- **First run follows the OS** (`nativeTheme.shouldUseDarkColors`). After the first toggle the choice is fixed and persisted.
- The main process sets `nativeTheme.themeSource` before creating the window and again on every `updateSettings` patch. This is applied before saving, so a failed write still switches the theme.
- The CSS uses `@media (prefers-color-scheme: dark)` tokens in `src/renderer/src/assets/main.css`, driven by `nativeTheme`.
- The HeroBand has its own `--hero-bg` gradient.
- `ThemeToggle` (`src/renderer/src/components/ThemeToggle.tsx`): a 26px sun/moon button in the footer. Tooltip and aria-label: "Switch to dark theme" / "Switch to light theme".
- The window background and overlay colours follow the theme to avoid a white flash on resize.
- Fonts: Manrope (sans) and Roboto Mono (mono), bundled.

---

### 11. Keyboard shortcuts and input

- **⌘N** (mac) / **Ctrl+N**: opens New download (ignored when it is already open).
- **⌘V / Ctrl+V** outside an input, textarea or contenteditable, with a valid link: opens New download prefilled.
- **Enter**: submits New download and Fix link (they are forms); saves in the network name field.
- **Escape**: closes dialogs (Base UI defaults); discards the NetworkEditPopover draft; closes the Limits dialog without saving.
- Production: Electron-toolkit blocks Ctrl/Cmd+R. Dev: F12 opens devtools.
- **No other shortcuts**: none for pause, select-all or delete.

---

### 12. Accessibility

- `prefers-reduced-motion` turns off all animations and transitions globally (main.css), and the BlockGrid scroll respects it.
- 24px minimum hit targets (WCAG 2.5.8) for stream toggles and colour swatches.
- ARIA in use:
  - `role="table/row/cell/columnheader"` in the network table
  - `role="progressbar"` with `aria-valuenow`
  - `role="toolbar"` with `aria-busy`
  - `role="alert"` for errors
  - `role="status"` for completion and empty filters
  - `aria-pressed` on network chips and swatches; `aria-disabled` (with a tooltip) for networks at their data limit
  - `aria-expanded` on expanders
  - `aria-current="page"` in the limits nav
- `sr-only` text: "Saving to", ", limited to X", direction labels ("Downloading at", "Uploading at", "Received", "Sent").
- `TruncatedText` is focusable and shows a tooltip only when the text is actually cut off.
- The chart has an `aria-label` summary.
- Group select-all checkboxes have descriptive labels.
- The update dialog's initial focus is on the primary action.

---

### 13. Formatting (`src/renderer/src/utils/format.ts`)

- `formatBytes`: binary units B/KB/MB/GB/TB (1024-based). 0 decimals for B, otherwise 1. Promotes the unit at 1024.
- `formatSpeed(bytes|bits)`:
  - bytes: `formatBytes` + "/s"
  - bits: bps/Kbps/Mbps/Gbps/Tbps, **decimal 1000-based**, whole numbers from 100 up
- `formatSpeedLimit`: a round figure, whole from 10 up, no trailing ".0".
- `formatDataUsage`: "11.7 of 50 GB". It rounds **down**, and states the unit once when both values share it.
- `formatSpeedOfLimit`: ["8.0", "10 MB/s"], or "—" when the speed is 0.
- `formatEta`: "Ns", "Mm Ss", "Hh Mm", "—".
- `formatDuration`: `m:ss` or `h:mm:ss`.
- `formatPercent`: integer, capped at 100.
- `formatWhen`: see §4.
- `toDisplayPath`: `~` for home, case-insensitive on Windows.
- `fileExtensionBadge`: as in §1.1.
- `describeFileCount`: "N files" or "X of N files".
- `sourceOf`: the URL host.
- **Speed unit**: the footer toggle "MB/s" / "Mbps" applies everywhere, including limit inputs. Sizes always stay in bytes.

---

### 14. Status bar / footer (`src/renderer/src/components/StatusBar.tsx`)

- Left: "X free" (free space on the destination drive via `statfs`, polled every **30 s** and again when the destination changes), then "N waiting" (queued count).
- Right, in order:
  - `UpdateIndicator` (when dismissed)
  - "Slow mode" switch, tooltip "Limit downloads to X"
  - speed-unit toggle (always one unit pressed), tooltip "Speed units"
  - ThemeToggle
- Speeds and limits are deliberately not repeated here.
- `TitleBar` (`src/renderer/src/components/TitleBar.tsx`): a centred "Plexo" drag strip, 32px on mac and 44px elsewhere. Side padding keeps clear of the OS controls.

---

### 15. Settings: every `AppSettings` field (`app-settings.json`)

| Field | Type | Default | Validation (`sanitizeSettings` in `src/main/settings.ts`) |
|---|---|---|---|
| `themeSource` | `'light' \| 'dark'` | unset: follows OS at first run | other values dropped |
| `dismissedUpdateVersion` | string | unset | string only |
| `destinationDir` | string | unset: OS Downloads | must be absolute; at launch must exist as a dir (300 ms cap) |
| `networkPreferences` | `Record<id, NetworkPreference>` | `{}` | per-entry, per-field checks; empty entries dropped |
| `downloadsAtOnce` | integer | 2 | 1–8 |
| `speedLimit` | bytes/s | unset (no limit) | positive safe integer |
| `slowMode` | boolean | false | only `true` is stored |
| `slowModeSpeed` | bytes/s | 2 MiB/s (2097152) | positive safe integer |
| `speedUnit` | `'bytes' \| 'bits'` | `'bytes'` | enum |

`NetworkPreference` fields:

| Field | Meaning | Default and validation |
|---|---|---|
| `customName` | string | max 40 characters, enforced in the UI only |
| `colorId` | one of 8 swatch ids | the renderer ignores invalid ids |
| `off` | `true` | the last pick left this network out |
| `speedLimit` | bytes/s | positive safe integer |
| `dataLimit` | bytes | positive safe integer |
| `dataLimitPeriod` | `day \| week \| month` | default month |

How saving works:
- The renderer saves optimistically: the store is updated first, then `updateSettings(patch)` is called and errors are swallowed.
- Main merges the patch over the saved file. A field set to `undefined` in the patch is cleared.
- Main then reads the file back and calls `manager.applySettings`.
- `InitialState` sent to the renderer: `homeDir`, `downloadsDir`, `themeSource`, `networkPreferences`, `downloadsAtOnce`, `speedLimit`, `slowMode`, `slowModeSpeed`, `speedUnit`, `destinationDir`.
- **Not persisted**: the stream count choice, the list filter, the NetworkBindingDialog dismissal, and the window size or position (no window-state persistence).

---

### 16. IPC channels (`src/shared/ipc-channels.ts`, `src/shared/ipc-contract.ts`)

Renderer → Main, request/response (`invoke`):

| Key | Channel | Args | Result |
|---|---|---|---|
| listInterfaces | `network:list-interfaces` | – | `NetworkInterfaceInfo[]` (forces a refresh) |
| deviceBindingSupported | `network:device-binding-supported` | – | boolean |
| openNetworkSettings | `network:open-settings` | – | void |
| probeUrl | `download:probe` | url | `ProbeResult` (http \| torrent with `TorrentInfo`) |
| updateSettings | `app:update-settings` | `AppSettings` patch | void |
| chooseDestinationFolder | `dialog:choose-destination-folder` | defaultPath | string \| null |
| chooseTorrentFile | `dialog:choose-torrent-file` | – | string \| null |
| readClipboardText | `clipboard:read-text` | – | string |
| revealDownload | `shell:reveal-download` | id | boolean |
| startDownload | `download:start` | `StartHttpDownloadRequest` \| `StartTorrentDownloadRequest` | id |
| listDownloads | `download:list` | – | `DownloadUpdate[]` snapshots |
| listHistory | `history:list` | – | `FinishedDownload[]` |
| clearHistory | `history:clear` | – | void |
| networkUsage | `network:usage` | – | `Record<id, bytes>` (current period) |
| resetNetworkUsage | `network:reset-usage` | id | void |
| freeSpace | `disk:free-space` | dir | number \| null |
| torrentFiles | `download:torrent-files` | id | `TorrentFileEntry[]` |
| chooseTorrentFiles | `download:choose-torrent-files` | id, selected[] | void |
| pauseDownload | `download:pause` | id | void |
| resumeDownload | `download:resume` | id | void |
| relinkDownload | `download:relink` | id, url | void |
| setDownloadNetwork | `download:set-network` | id, networkId, enabled | void |
| cancelDownload | `download:cancel` | id | void (**exposed but unused by the renderer**; cancelling goes through removeDownload) |
| removeDownload | `download:remove` | id, `{trashFile?}` | void |
| checkForUpdate | `update:check` | – | `UpdateInfo` \| null |
| takePendingLink | `app:take-pending-link` | – | string \| null |

Renderer → Main, synchronous: `app:get-initial-state` (`sendSync`) → `InitialState`. It always replies, with fallback defaults if reading fails.

Main → Renderer, pushes:
- `download:updated`: a `DownloadUpdate` `{seq, state, blocks|pieces (changed only)}`, throttled at 200 ms per download. The renderer merges it with `applyDownloadUpdate` and ignores stale `seq` values.
- `network:changed`: `NetworkInterfaceInfo[]`.
- `history:changed`: no payload; the renderer re-lists after 150 ms.
- `app:link-received`: no payload; the renderer then calls `takePendingLink`.

`StartDownloadRequest` common fields: `url` (finalUrl after redirects), `destinationDir`, `suggestedFileName`, `totalBytes` (0 = unknown), `supportsRanges`, `interfaceIds`, `etag`, `lastModified`.
- HTTP adds `streamsPerNetwork?`.
- Torrent adds `infoHash` and `selectedFiles?`.

---

### 17. Persisted files in `userData`

| File | Format | Notes |
|---|---|---|
| `app-settings.json` | JSON, pretty-printed (2 spaces) | `AppSettings`. Atomic write: `<path>.<pid>.tmp` then rename, with per-file write queueing and Windows rename retry (EPERM/EACCES/EBUSY, 5 attempts). A corrupt file or ENOENT reads as defaults. |
| `network-preferences.json` | legacy, up to rc.8 | Merged into app-settings once at startup, then deleted. |
| `history.json` | JSON array of `FinishedDownload` | Newest first, max 500. `missing` is not stored; `downloadedFiles` is kept for torrents. |
| `network-usage.json` | `{version:2, periods:{day,week,month:{key, bytes:{id:n}}}}` | Older `{month, bytes}` files are migrated. Periods with a stale key are discarded on load. Saved every 10 s and on quit. |
| `downloads/<uuid>/manifest.json` | JSON, `version: 7` | `{savedAt, state (minus blocks/streams or pieces/peers), progress:{networkId:[bytes per block]}, complete, partialPath, publicationPath?, publicationIdentity?, requestPayload}`. Checkpointed every 15 s, plus on pause and publication, with an fsync of the partial file. Written via `.tmp` + rename. Version-2 manifests from rc.1–rc.9 are recognised for cleanup. |
| `downloads/<uuid>/metadata.torrent` | raw `.torrent` | Torrent downloads only. |
| alongside the destination | `<final>.plexo` | HTTP staging file, sparse on Windows. Renamed to the final name when complete. |
| temp dir | `plexo-magnet/` | Magnet metadata probe store, destroyed afterwards. |

---

### 18. Landing site (`docs/`)

- **`docs/index.html`**: a static page at getplexo.app (`docs/CNAME`). Title "Plexo — Free download manager that combines your networks". Includes JSON-LD, OG image, sitemap and robots, and the Geist, Geist Mono and Archivo fonts.
  - Nav: Features, Downloads, FAQ, Support. There is a mobile menu toggle (Escape closes it) and a "Pre-release" pill.
  - Hero: "Combine your networks. Download faster." with an **interactive demo**: Wi-Fi/USB toggles, live total speed, a throughput chart, share bars, and cycling demo files (Project.zip, ubuntu ISO, Footage .mov). A play/pause control respects `prefers-reduced-motion`, and the demo pauses when the tab is hidden. The demo is scaled to the viewport from a 1040px design width.
  - Features cells: Multiple connections, Files and torrents (a queue mock), Speed and data limits (a limits mock).
  - FAQ (6 items): how it combines; which connections; why the OS warns; whole-device speed (no, not a VPN); stability (pre-release); Android tethering on Mac via TetherKit.
  - Support section: star, share, sponsor. A **Share button** uses `navigator.share` or falls back to clipboard ("Link copied"). "Made by Anmol Kapil". Back to top.
- **`docs/downloads.js`** (shared ES5; also used by `scripts/release-notes.mjs`):
  - `describe(name)` classifies release assets by extension and arch: dmg arm64/x64; exe with no arch meaning universal x64+ARM64; AppImage, deb and rpm for x64 and arm64.
  - `detectEnvironment(navigator, hint)` detects mobile (including iPadOS posing as Mac with touch points), ChromeOS as "other", and win, mac or linux. Arch comes from `userAgentData.getHighEntropyValues(['architecture','bitness'])` on Chromium only, else Linux UA hints. The arch is null when unknown.
  - `choose()` defaults Mac to Apple silicon with an Intel alternate, picks the universal Windows installer, and picks Linux AppImage with a .deb alternate.
  - `build()` groups assets per OS with first-launch notes. `markdown()` builds the release-notes downloads table.
- **`docs/landing.js`**:
  - fetches `api.github.com/repos/anmolkapil/plexo/releases?per_page=10` with a 9 s timeout, and picks the first non-draft release that has recognised assets
  - asset URLs are whitelisted to `github.com/anmolkapil/plexo/releases/`
  - renders OS tabs (arrow, Home and End keys) with arch cards, a "Detected" badge, and "We couldn't identify your processor…"
  - hero primary button: "Download for <OS>" plus build meta, or arch choices when the arch is unknown (it never silently defaults to one)
  - release summary: "Latest pre-release/release", version, Release notes link, Previous releases
  - fallbacks: "Couldn't load the latest builds…" or placeholder panels
  - per-OS "First time opening Plexo?" install guides (`<details>`) with copyable commands (Copy → Copied, with an sr-only status); mac uses `xattr -cr`
  - clicking a release download opens the `install-dialog`, "Thanks for downloading Plexo", with try-again and other-builds links; after it closes, a reopen toast "Plexo is downloading / Need help opening it?" appears
  - scroll-spy sets `aria-current` on nav links; reveal-on-scroll uses IntersectionObserver
- **`docs/changelog.js`** (`PlexoChangelog`, entries rc.9–rc.12, kinds new / faster / improved / fixed): **nothing references it**. It is not loaded by `index.html` and no script uses it. It is also stale: the latest entry is rc.12 while the app is rc.14.
- `docs/404.html`, `docs/favicon.svg`, `docs/og.png` and `docs/screenshots/*` are present.

---

### 19. Notable discrepancies and gaps

1. The README says limits "apply as you make them"; the code uses a draft with Save/Cancel.
2. The README says the per-network speed is set "in KB/s or MB/s"; the code uses MB/s or Mbps following the footer unit.
3. The README lists AppImage and .deb only; builder and landing also ship .rpm.
4. Install guidance differs: the README and `downloads.js` use `xattr -dr com.apple.quarantine`, while `landing.js` uses `xattr -cr`.
5. `cancelDownload` IPC is unused by the renderer.
6. Missing features: no dock/taskbar progress, no tray, no custom menu, no seeding after completion, no queue reordering, no window-state persistence, no "system" theme option after the first run, and no auto-update install.
7. `changelog.js` is orphaned (item 18).
8. The ErrorScreen's "cancelled" branch is effectively unreachable, because App.tsx sends a cancelled download back to the list.


---

# Part 2: Bug and lesson catalogue from git history (263 commits)

**Repo:** `/Users/arshramgarhia/Documents/sample/plexo`. Nothing was modified.

**Scope:** 263 commits from 2026-09-13 to 2026-10-07, plus tags `v1.0.0-rc.1` through `rc.14`. I read every commit message and body, and inspected about 50 diffs with `git show` (stat or full). Those were mainly the fixes whose messages explain nothing: b0c0bc6, e56fbcf, d86fc36, 9cc4db5, 4e8f7af, 0819813, 1d277a4, 81791d9, 98ad2d7, e01cf95, 13adb6d, 871dcfd, e1e2f4e, 32c1a69, 81ebb6e, 765964f, 821059e, the fc6e675 merge, 32f5e09, 6888ffa, 1eb9883, 22de4b1, 0774aa6, d7dc405, a131436, 98e5896, ca6782b, 0d98722, 6be78d5, 7493296, 5136608, 8cc15e4, 4995e05, 873cef9, 3272d6b, f892510, 6bc1537, 1861af2, 3946d64, e8b84b6, 34541f1, 6eddb0e and 5a2358c.

**A caveat on PRs #64, #84, #89 and #90:** these were squash-merged, so their individual sub-commits have no hashes of their own. I cite the squash hash with the sub-item's title.

**Who wrote it:** most bodies were written with Claude Code (Co-Authored-By trailers). They are unusually detailed, so root causes below are often quoted from them.

---

### 1. HTTP engine (range requests, connections, scheduling, retries)

| Hash | Symptom | Root cause | Fix | Rule for us |
|---|---|---|---|---|
| **b0d14a2** | Speed and ETA jumped around constantly. | Speed was worked out from the gap between raw socket `data` events, which arrive a few ms apart and irregularly. | Average bytes over a rolling 3 s window per chunk. | Never derive a rate from event-to-event deltas. Use a time window. |
| **6893072** | A silent socket hung a chunk forever. One dropped connection failed the whole download. | No idle timeout and no per-chunk retry. | 20 s socket idle timeout; up to 5 retries with exponential backoff; a `retrying` status. | Every network read needs a timeout, and every unit of work needs its own retry budget, separate from the job's. |
| **c2e9281** | A chunk failed with "Unexpected status" when a signed CDN URL rotated mid-download. | Only the probe followed redirects. | Chunk requests follow 3xx, keeping the same Range header and interface binding. Abort and stall wiring follows whichever hop is in flight. | Every request path must follow redirects the way the probe does. Carry the Range header and the interface binding across each hop. |
| **547169d / b0c0bc6** | Only one connection per network. Later, the split was uneven. | Connections were tied 1:1 to interfaces. Ranges were split naively. | A connections-per-link setting. b0c0bc6 splits the file per network first, then per connection. | Treat connection count as its own setting, separate from how many networks there are. |
| **2fdb33b** | Connections that finished early sat idle while a slow sibling crawled through a large range. Reassembly was also scrambled after a split. | Byte ranges were fixed at start. Parts were concatenated in array order, so a split chunk appended at the end landed in the wrong place. | Work-stealing: split the largest remaining range, with a 1 MB floor. Read the AbortController fresh on each loop pass. Sort parts by `rangeStart` before joining. | Any dynamic re-chunking must order output by offset, never by creation order. |
| **196ce9b** | On a 100 GB file, blocks were about 1.6 GB, so a straggler left the other workers idle. Progress accounting got 12–64× more expensive. | Block size was tied to how many squares the progress grid could draw. | Fixed 8 MB blocks; the grid groups blocks for display. Progress deltas are added to a running total instead of re-summing every block. | Keep the work unit apart from how it is displayed. On the per-byte hot path, never re-sum a collection. |
| **9918671** | A server that capped ranges at 1 MB turned a 40 MB download into a 5 MB file marked complete, with the part files deleted. | Any 206 response was trusted. | Check that Content-Range starts at the requested offset and doesn't run past the end, and that the body length matches. Cut an overlong body off at the range boundary. Backpressure is handled by hand (pause/resume) instead of `pipe()`. | Check that a 206 covers exactly the range you asked for. A status code alone proves nothing. |
| **4e8f7af** | Range support was detected wrongly. | The code trusted the `Accept-Ranges` header. | Ranges count as supported only if a `bytes=0-0` probe returns 206. | Decide whether a server supports a feature by how it behaves, not by what its headers claim. |
| **68b9939** (7 fixes) | Probe hung forever. 0-byte files failed. A malformed `%`-escape threw. Retries got 200 instead of 206 on servers without range support. | No probe timeout. A 416 with `bytes */0` wasn't handled. Retries sent `bytes=N-` to non-range servers. | Probe timeout. Treat 416 `*/0` as an empty file. Fall back to the raw path when an escape is malformed. Restart non-range downloads from byte 0. | Test the edge cases explicitly: 0-byte files, non-range servers, malformed URLs, servers that never answer. |
| **0819813, 4995e05** | Wrong file names from `Content-Disposition`. A Latin-1 `%A3` (£) came through as raw escapes. | One naive regex; `filename*` charset ignored; `decodeURIComponent` only reads UTF-8. | A proper RFC 6266/5987 parser: `filename*` wins, quoted strings keep their semicolons, and ISO-8859-1 is decoded byte by byte. | Header parsing follows the RFC. Unit-test both charsets RFC 5987 requires. |
| **1d277a4** | Headers arrived, then the body froze. | The timeout covered the connection, not body progress. | A body watchdog reset on each `data` event and on `drain`. | Time out on lack of progress, not only on connecting. |
| **6eddb0e** | Downloads hung at 99%. | A connection trickling a few kbps never went silent long enough to trip the stall watchdog. | Reconnect a connection running under 10% of its network's median speed for 10 s, at most twice per block. Networks are never compared with each other. | Detect slow connections relative to their own network's peers. Cap how many times you refresh. |
| **8f33a22** | Throughput and ETA were inflated during retry backoff. Two speed tickers ran at once. | A retrying chunk kept its pre-failure speed. The ticker's lifetime followed global status instead of its own run. | Zero the speed on retry. Tie the ticker to `runChunksToCompletion`'s lifetime. | A timer's lifetime belongs to the scope that created it. |
| **871dcfd** | The peak speed shown was implausibly high. | Two samples a few ms apart made one socket burst look like a sustained speed. | Measurement window floor of 1 s. | Put a minimum duration on any rate measurement. |
| **3f3129b** | With 7 blocks and 16 streams, the second network got no work while the UI said "2 networks combined". | Streams were started network by network and each grabbed a block immediately. | `shared/plan.ts`: interleave streams across networks, cap each network at its share of blocks (minimum one), and size blocks to roughly 2 per stream (1–8 MB). | Interleave start order across resources. Property-test the planner (the e2e that would have caught this failed before the fix). |
| **be75817 + aed305b** | A crawling connection or network held up the tail. Idle streams showed "Chunk #1, Done, 100%". | Reconnecting just swaps one slow connection for another. Streams kept their last block. | A pure scheduler, tested with property tests for deadlock freedom. Hedged attempts: a second attempt races a block that has run at least 5 s and needs at least 5 s more, and the first to finish wins. A connection silent for 5 s while others are receiving is reconnected. A stream holds a block only while downloading it. `PLEXO_DEBUG=1` logs time to first byte and whether the connection was reused. | Make scheduling a pure function of a snapshot so it can be property-tested. Race slow tail work instead of only retrying it. |
| **5a2358c** (perf) | Each 8 MiB block paid for DNS, TCP, TLS and slow start again (#31). | A custom `createConnection` with no agent makes Node skip pooling and send `Connection: close`. | One keep-alive `StreamConnection` per stream; the agent's `createConnection` handles routing and binding. TLS goes through the agent, so certificates are verified and sessions resumed. A reused socket the server closed gets one immediate retry. | A custom `createConnection` with no agent silently turns off keep-alive. Always go through an Agent. |
| **bdfa59f** (perf) | Hedges held whole blocks in memory. | Left over from the per-block part-file design. | With a single staging file written at fixed offsets, hedges write identical, version-checked bytes straight to the file. | Write at offsets, idempotently. It removes the need for buffering and merging. |
| **a323149 → f9d1d72 → d57778d → 8dae93f** (design churn, see §13) | The "measured trials" stream controller failed at random on noisy mobile links and froze a network's growth. Some servers accept extra connections and never answer them. | A noisy experimental controller. Silent refusals were never classed as refusals. | Simple rule: start at 8, double while every stream is receiving, up to 32. Lower the limit only by the streams refused while others are served. Recover one stream per minute without a refusal. A stream silent while its network's others are served counts as refused. | Prefer simple, predictable control rules over clever measured ones on noisy links. Count "accepted but never answered" as a refusal. |
| **bb6a711** | Two runs overlapped after a quick pause and resume. A block another attempt had finished was reset. | Resume didn't wait for the paused run to wind down. Not every stop path handed the block back. | Resume waits for wind-down. Every stop goes through `letGo`. A request's abort cancels its connect. | Serialize the lifecycle: a new run waits until the old one has fully stopped. One exit path hands work back. |
| **34541f1** | A slow destination drive caused "connection error" and marked every network unreachable. 503/429 used up retries quickly. Wake from sleep left broken sockets. | The stall watchdog kept running while a response was paused on disk backpressure. 5xx/429 were treated as fatal after 5 strikes. | Pause the watchdog during backpressure. Wait out 408/429/5xx for up to 5 min and honour Retry-After (capped at 2 min) for the whole network. ±20% jitter on backoff, reset once an attempt makes progress. Refresh connections on wake and keep the computer awake while downloading. 10 s connect timeout. Race a network's addresses Happy-Eyeballs style (RFC 8305, 250 ms apart), last-good first. The probe's 20 s timeout covers the whole redirect chain. | Tell disk stalls apart from network stalls. Use curl's transient status set {408, 429, 500, 502, 503, 504} plus Retry-After. Handle sleep and wake explicitly. |
| **806a261** | A network that dropped was never used again. If every stream stopped, the download failed and deleted its progress. | Networks were fixed at start, and connect failures counted against server-error retries. | A NetworkMonitor pushes changes. A per-download network list with switches, run through `reconcile()`. Connect failures are blamed on the network and retried while it exists. With no network the download waits instead of failing, and an error keeps the partial file. | Blame failures on the right layer (network vs server). "No network" means wait, not fail. |
| **5404d92** ("Read speeds on the download's clock") | A 7 MB/s limit displayed as 6.9. | Speeds were sampled as bytes arrived, and bytes come in bursts, so the windows were biased. | Connections only count bytes into meters. The manager reads every meter on a clock tick. | Sample rates on a clock tick, not when data arrives. |
| **5404d92** ("Smooth time left") | Time left swung by minutes. | It was remaining bytes ÷ current speed, worked out separately in two places. | One value worked out in main, Firefox style: counts down between estimates, moves 30%/s toward a lower one and 10%/s toward a higher one. | Work out derived displays once, in one place, with asymmetric smoothing. |
| **5404d92** ("Fix run-loop handler leak") | Leak. | One wake handler was added per tick per stream. | One handler per stream for its lifetime. | Register listeners once per object, never inside a tick. |

### 2. Resume, integrity and saved progress

| Hash | Symptom | Root cause | Fix | Rule for us |
|---|---|---|---|---|
| **6893072** | Corruption on resume. | Appending trusted in-memory byte counts. | Before each restart, reconcile the part file's real size and truncate to the smaller of disk and memory. | What is on disk is the truth. Reconcile with it before appending. |
| **029387e → 5874f1b → 194433a** | Old and new versions of a file were stitched together. Later, healthy downloads failed behind load balancers. | No validators were checked. Then every ETag change was treated as fatal. | 029387e checks ETag and Last-Modified on resume. 5874f1b checks every chunk response against the probe. 194433a: a size change is fatal; ETags are normalized (strip `W/`, quotes, `-gzip`); any other label change is settled by re-fetching up to 8 samples of bytes already on disk. | Server validators are a hint, not proof. Normalize them, and confirm a change by comparing bytes. |
| **a5f552f** | Truncated files were labelled complete and the parts deleted. | Assembly checked nothing. | Check every block is complete, every part has its length, and the total matches. Delete the partial output on failure. Attach an error listener on open, because an unhandled stream `error` takes down the main process. | Put guards in front of the most expensive failure. A half-written file is worse than none. |
| **7911897** | One leaked file descriptor per abort (heading for EMFILE). Late buffered writes landed after the file had been reconciled. | `fail()` destroyed the request but not the write stream, and `pipe()` doesn't close its destination. | Close or destroy the part-file stream on failure. | Every abort path must close its file handles. Measure descriptors per abort. |
| **515202b, d7dc405** | Two downloads with the same name overwrote each other. | An `existsSync` check doesn't reserve the name. | Claim the name with an `O_EXCL` create (later `<name>.plexo` with `wx+`) and release it on cancel or error. | Reserve names by exclusive create, not by checking. |
| **e56fbcf, d86fc36** | Downloads were lost on relaunch. Stale manifests were written during shutdown. | No persistence; then the persistence timer kept firing while suspending. | Manifest JSON written via temp file + rename. A `suspending` flag stops scheduled writes. Remove stale `.tmp` files on restore. | Write via temp + rename, and freeze persistence while shutting down. |
| **036749d, 358d00c, 67d2e6e** | Simulated downloads lost their interfaces on resume. Orphaned restored downloads blocked new starts invisibly. Resume looked dead. | Resume re-listed the real interfaces. Restore brought back everything. A paused download's error was swallowed. | Keep synthetic interfaces. Keep only the newest restored download. Show "Resuming…" and surface the paused error. | State restored at startup has to pass the same checks as live state. Never swallow an error on a paused item. |
| **68b9939** | Resume failed at assembly. | Part files were missing or short. | Re-fetch those blocks on resume. | Check on-disk data when resuming, not only at the end. |
| **5851ec0** | 3 ms per update push at 4096 blocks. | The whole state was `structuredClone`d every 200 ms. | Send only the blocks that changed, with sequence numbers. Manifest v5 stores per-network bytes per block. | Diff before IPC. Version your persisted formats. |
| **bb6a711** | Unknown-size files couldn't be resumed reliably. | Completion wasn't recorded. | `savedProgress.ts` records completion and validates what it reads back. | Validate persisted state when reading it back. |
| **5404d92** ("Clear what can't be restored") | Leftovers from rc.1–rc.9 lingered forever. | Old manifest versions were kept and ignored. | Read only v6 (later v7). Clear any download that can't be restored, but delete partial data only when it is unmistakably Plexo's. | Clean up unreadable state actively, but delete user-visible files only when you can prove they are yours. |
| **a669eb3, fd411f2, 7184397, 615150e** (settings) | Successive saves dropped each other. A failed read wiped the file. The theme toggle and window disagreed. | No write queue. Any read error was treated as an empty file. | Per-file write queue with temp + rename. Reads wait for queued writes. Per-process temp names. Only a missing or corrupt file counts as empty; other errors abort the save. Apply the theme before saving. | "Missing" and "unreadable" are different. Never overwrite a file you couldn't read. |
| **93da97a** | A dismissed update dialog came back after Cmd+R. | Dismissal was cached at startup. | Read it on each call. | Cached startup state goes stale after a renderer reload. Test reloads. |

### 3. Disk I/O (sparse files, staging, free space, backpressure)

| Hash | Lesson |
|---|---|
| **029387e** | Check free space up front, so the user doesn't hit ENOSPC partway through. |
| **31fb254** | Assembly hung forever on a disk-full error, because the code waited for `drain`, which an errored stream never emits. Use `stream.pipeline`. The space check also has to count parts plus the assembled copy (2× the file). **Rule:** don't wait on `drain` by hand; use `pipeline`. |
| **aa3c256** | `MaxListenersExceededWarning` appeared at 5 or more blocks, because one `pipeline` ran per part with `end:false`, which leaves listeners behind on the destination. Replaced with a single pipeline over a generator. Every e2e now fails on a leak warning. **Rule:** treat Node warnings as test failures. |
| **0774aa6 → d7dc405 → a131436 → 98e5896** (pivot) | Downloads are written straight to a staging file at fixed offsets beside the destination. That removes the second full-size copy, so roughly 1× disk space is enough. The hidden `.plexo-<id>.part` name became the visible `<name>.plexo`. Publishing by hard link (atomic, no overwrite) failed on exFAT, so it became an `lstat` check followed by `rename` (accepting a small race). Names are truncated to the 255-byte component limit. **Rule:** stage on the same volume and rename. Don't depend on hard links. Budget the component length for suffixes. |
| **34541f1** | Disk backpressure was misread as a network stall. Pause the watchdog while writes are held back. |
| **e8b84b6** (#89) | On a hard drive, many streams writing scattered offsets make the disk the bottleneck, and held readers look like a slow network. Changes: judge disk pressure per download, not per network (per-network judging re-created the "fast network stuck low" shape from #50); 512 KiB writer buffers so `writev` coalesces; a DiskWatch that judges by the clock, not by ticks; no hedging or crawl judgements while the disk is behind; a stopped attempt flushes what it holds instead of discarding up to 512 KiB; a queued checkpoint is no longer dropped; a "slow drive" warning with hysteresis (on at 1 s, off after 10 s). Separately, NTFS zero-fills the gap before a write past the last byte written, so most of the file could be written twice; the fix is `FSCTL_SET_SPARSE` via kernel32 through koffi (refused harmlessly on exFAT/FAT), checked in Windows CI with `fsutil`. **Rule:** the disk is a resource the controller must see. Make staging files sparse on NTFS. |
| **5404d92** | Torrent checkpoints fsync only files written since the last sync. A slow-mode limit that was never saved fell back to nothing; it now falls back to the default the UI shows. **Rule:** a default shown in the UI must also be the default the code applies. |
| **765964f** | A saved destination on a dropped network share blocked launch (sync IPC plus `stat`). Capped at 300 ms. **Rule:** cap any filesystem call on the startup path. |
| **8dae93f** | A test volume was too small because Chromium's GPU caches took several MB of userData. **Rule:** disk-limit tests need headroom for the runtime's own files. |

### 4. Network detection per OS

- **9cc4db5 (Windows):** PowerShell `Get-NetAdapter` (with a 5 s timeout, UTF-8 output, BOM stripped, single object vs array handled). Uses `NdisPhysicalMedium` (1/9 = Wi-Fi, 14 = Ethernet). Link-local `169.254.*` addresses are filtered out. Classification gained regexes for rndis/tether. **Rule:** shelling out to OS tools needs a timeout, encoding handling and a graceful empty fallback.
- **81791d9:** Latency probe falls back from `1.1.1.1` to `8.8.8.8`. Linux network settings open via `gnome-control-center || nm-connection-editor`. Ping was later removed entirely (21ad8c0). **Rule:** remove a measurement when nothing uses it, along with its polling and IPC.
- **0819813:** Windows display name falls back to the adapter description.
- **32f5e09:** Windows routes by interface metric, so two adapters on the same subnet or router add nothing. The fix detects same-subnet conflicts (CIDR from the netmask) and warns. The README also covers the Windows policy that disconnects Wi-Fi when Ethernet is plugged in. **Rule:** detect the configurations where bonding can't work and tell the user.
- **6888ffa / 1eb9883 / 22de4b1 (IPv6):** Interfaces carry multiple addresses. `routes.ts` matches local and remote address families and preserves DNS order. Each route gets a short connect window (≤3 s, half the remaining time) while the last gets everything left. DNS is bounded by the request deadline. Brackets are stripped from IPv6 literals. The probe's `req.setTimeout` (an idle timeout) was replaced with a total timer. **Rules:** DNS is part of the request deadline; a stale IPv6 address must not use up the whole budget; strip `[]` before passing hosts to sockets.
- **806a261 / 34541f1:** Push network changes from main instead of polling the renderer. Look up the network's address on every connect. When an address changes, wake streams immediately and drop that network's sockets.
- **7d30ea6:** With no networks, the renderer looped about 4000 scans/s and froze. Every scan set status to "loading", which swapped screens, and the new screen scanned on mount. **Rule:** a re-scan must keep the last result on screen. Watch for mount effects that change state which unmounts the component.
- **21ad8c0:** `networksetup` output was parsed by its English labels, so it broke on non-English macOS. It is now read by position. Windows uses its short alias ("Wi-Fi", "Ethernet 2"). Linux infers kind from device names (wlp*, enp*, enx*, *u1). **Rule:** never parse localized CLI labels.

### 5. Linux binding

- **32c1a69 (rc.7):** Linux picks the outgoing route by destination, so binding to an IP (`localAddress`) still left via the default route, where the other network dropped the packets. The second network was useless. Fix: create the socket with libc `socket()` plus `setsockopt(SO_BINDTODEVICE)` via koffi, then hand Node the fd (`new Socket({fd, manualStart:true})`, with no `localAddress`, because a second bind fails). Support is checked once by binding to `lo`. Kernels older than 5.7 refuse unprivileged use; those fall back to the old behaviour and a dialog explains it. A custom `createConnection` must set `port` and `defaultPort`, or Node writes the port into the Host header. Only Linux koffi prebuilds were packaged. **Rule:** on Linux, binding to a source IP doesn't pin the interface. Feature-detect `SO_BINDTODEVICE` and degrade visibly.
- **81ebb6e:** Loopback downloads hung forever, because a socket pinned to eth0 can't reach 127.0.0.1 and the connect had no timeout. Loopback is now never pinned. **Rule:** exempt loopback from interface pinning, and always put a timeout on connect.
- **3b0721e:** The snap target was dropped. It had never been released, and its sandbox would likely restrict per-interface binding.
- **8daba37:** The download page tells users that multiple networks need kernel 5.7 and that AppImage needs FUSE 2 on recent Ubuntu.

### 6. Windows and NTFS

- **9cc4db5:** Wait for the write stream's `close`, not `end`, before resolving, because Windows can't reopen or remove a file whose handle is still open. Use `finished(output)`. Sanitize server-supplied names: `<>:"|?*`, trailing dots and spaces, reserved names (CON, PRN, COM1–9 including superscript digits, LPT), and NTFS alternate data streams.
- **fc6e675 (PR #17):** `mkdir` on a drive root throws EPERM, so `ensureDirectory` treats an existing directory as success. `before-quit` force-exits after 3 s if suspending hangs.
- **fd411f2:** Retry rename on EPERM/EACCES/EBUSY, since antivirus, indexers and sync clients briefly hold files open.
- **97fdcc0 / 6bc1537:** Assisted NSIS installer with a directory page and a desktop-shortcut checkbox. The build broke because `$mui.FinishPage.ShowReadme` is only declared by `MUI_PAGE_FINISH`; functions using it must come after it (warning 6000 under `-WX`). **Rule:** in NSIS, declaration order matters and warnings are errors.
- **98ad2d7 → 8cc15e4:** The Windows build CI job was removed three days after it was added (9cc4db5). rc.12 then shipped torrents that crashed on Windows startup: the cross-platform build bundled the macOS host's `node-datachannel` WebRTC native binary, and `simple-peer` imported it eagerly. Fix (rc.13): patch `simple-peer` to use globals, set `tracker.wrtc:false`, exclude `node-datachannel` and `webrtc-polyfill` from the package, add an `afterPack` script that fails the build if they are in `app.asar`, and add a `windows-torrent` CI job that runs with `--ignore-scripts` and with `process.dlopen` throwing. **Rule:** don't drop platform CI. Cross-building from one OS ships that OS's native addons. Assert the package's contents in `afterPack`.
- **e8b84b6:** Sparse staging files on NTFS. The koffi exclusion glob had to add `win32_x64|win32_arm64`. **Rule:** when a native dependency gains a new platform use, update the packaging excludes and test the packaged app.
- **3272d6b:** Killing Electron's main process on Windows doesn't kill Chromium's children, which keep the profile lock. The test harness uses `taskkill /T /F`.
- **5404d92:** Torrent paths that Windows can't save are refused. Windows doesn't register magnet/.torrent handlers, because the installer would take them from the user's torrent client. Windows and Linux draw their window controls over Plexo's own strip.

### 7. Torrents / WebTorrent (all in 5404d92 unless noted)

- **Pivot:** the plan was originally to write a BitTorrent engine on `bittorrent-protocol`. Research found every download manager with torrents embeds a mature engine, so it switched to webtorrent 3.0.21 (pinned exactly) with a two-line `patch-package` patch adding a `connect` hook, used to pin each peer to a network.
- **Turned off:** uTP, web seeds, UPnP/NAT-PMP and LSD, because they bypass the TCP hook. WebRTC was added to that list in 8cc15e4.
- **Pause and resume:** pause destroys the client. Resume re-adds the torrent with the done pieces as its bitfield, which webtorrent spot-checks by hash.
- **Incoming peers:** first dropped, because their network was unknown. Then kept on the network whose address matches the socket's local address. **Rule:** a socket's local address tells you its network.
- **Upload over USB:** first banned, then allowed, because tit-for-tat rewards uploading on each network.
- **Layout:** first a `<name>.plexo/` staging folder, then "download in place under the claimed name". Manifest moved to v7, and cleanup must never remove the download now that the published path and the download are the same path.
- **Flaky test:** seeders shared `/tmp/webtorrent`, so two tests seeding "Album" overwrote each other and served pieces that failed verification (about 1 run in 12). Each seeder now gets its own folder. **Rule:** isolate per-test temp directories, including a library's defaults.
- **Peer row:** a per-peer progress bar sat at 100% for every seeder, so it became a dash. A "reduce repeated peer row details" change was reverted inside the PR.
- **File selection:** `choosePieces` is recomputed on every start, restore and change. A file that is fully downloaded stays ticked.

### 8. Packaging and installers

- **f892510 (rc.5):** electron-builder defaults to the build machine's architecture, so building on Apple Silicon produced arm64-only dmg, deb, AppImage and even the NSIS exe. Fix: explicit `arch: [x64, arm64]` and `${arch}` in artifact names. **Rule:** set the architecture explicitly for every target.
- **3b0721e:** Dropped snap. **e5271f4:** Added rpm.
- **d2970aa:** The deb build needs `homepage` in package.json; otherwise electron-builder reads `.git/config`, which a worktree doesn't have.
- **d2d4bcf → 1861af2:** Electron 44 has no postinstall and downloads its binary lazily. electron-vite 5 reads `path.txt` directly, so a failed install broke dev with "Electron uninstall". The fix uses Electron's own `install-electron` in postinstall, predev and prestart, replacing a custom script. **Rule:** use the tool's official idempotent installer, not a homemade one.
- **4de3788:** CI installs with `--ignore-scripts` for lint and typecheck.
- **32c1a69:** Ship only the koffi prebuilds needed.
- **8cc15e4:** `afterPack` checks the asar contents.
- **5404d92:** Info.plist `LSHandlerRank Alternate` and Linux `.desktop` MIME types offer Plexo for magnet links without making it the default.

### 9. macOS quirks

- **38a954d:** Dark Mode showed a white content area under a dark titlebar, so colours became tokens and the BrowserWindow `backgroundColor` follows `nativeTheme` (no white flash on resize).
- **375a53a:** The dock icon swapped to a light variant, mismatching Finder and Launchpad, so it is always dark.
- **e585ef2:** The System theme option was dropped.
- **a203d66 (landing page):** macOS overlay scrollbars hide until you scroll. A styled `::-webkit-scrollbar` keeps them visible, and `scrollbar-width` is kept to Firefox because Chrome prefers it when set.
- **21ad8c0:** Localized `networksetup` output.
- **5404d92:** `open-url` and `open-file` must be registered before `ready`, behind a single-instance lock. Each link is kept until the window takes it, so a cold start doesn't lose it.
- **8daba37 / d334fc9:** The app is unsigned, so the page shows first-launch steps and the exact Gatekeeper command with Copy. Safari and Firefox can't report a Mac's chip, so the default is Apple silicon with a one-click Intel link.
- **b2e06a4 / caf3c0b:** Android USB tethering on macOS needs TetherKit.

### 10. UI and renderer

- **Descender clipping happened 3 times:** e1e2f4e, 6be78d5, and in 5404d92 ("Stop clipping the line under a download's name"). Each time `truncate` (overflow:hidden) was combined with `leading-none`. **Rule:** truncated text needs `leading-normal`. Lint or review for `truncate` + `leading-none`.
- **Subgrid saga (a63b53c → dbec011 → 4457892 → 7ee32c3 → 82c6d04 → 50701eb → 83f7e22 → 6b10ff6):** the status dot overlapped the name. Gutters on a subgridded axis come from the parent grid, not from `gap` on the items. Chromium clips outer tracks by a subgrid item's own padding. A margin hack (50701eb) was added, then removed once the real cause was found. **Rule:** build a standalone repro (82c6d04 did) and don't stack stopgaps.
- **97b001c:** An unlayered `button { font: inherit }` beat every Tailwind utility. **Rule:** global resets go in `@layer base`.
- **43dfcc7:** Elements that appear and disappear changed row heights. `tailwind-merge` dropped Badge's base `text-xs`.
- **694d030 / a5e6cbc:** base-ui decides on the first render whether `open` is controlled, so `open={false}` before measuring left TruncatedText's tooltip permanently shut. Use `disabled` instead.
- **e091cfb:** Natively disabled buttons can't be hovered or focused, which defeated their explanatory tooltips; use `focusableWhenDisabled`.
- **c9ff927 / c786ca8 / 1737f6c / 8141207:** Vocabulary consistency ("combine" for networks vs "assemble" for the file; streams vs chunks). Don't show two contradictory percentages or repeat "PAUSED" five times.
- **df45254:** Raw internal errors reached the user mid-download; route every error through one translator. **38a954d:** Strip Electron's "Error invoking remote method" prefix.
- **715e257:** One typed `IpcContract` for preload and main (drift used to compile cleanly), and `ipcRenderer` is no longer exposed. **b1a39eb:** Removed `sandbox:false`; only http(s) goes to `openExternal`; persisted prefs are sanitized field by field. **f7ca676:** Exhaustive switch with `assertNever`.
- **ca6782b / 7493296 / 5136608 / 4484085:** Min width 620→720 and min height 420→520. A further bump to 600/640 was reverted the same day.
- **3272d6b:** A progress bar at 0% had zero width, so Playwright judged it invisible. A full-width track with an inner fill fixed it.
- **5404d92:** Peak was set by the startup burst, so it became the best 5-second average. Peak then dropped when the stand-in value was replaced, so it shows a dash until a speed is held. In 21ad8c0, PEAK could read below AVG (40 of 20,000 simulated runs); it is now computed from the same bytes as AVG, with pauses cut out. "Show in Finder" used a stale exists flag; main now checks again by id. Network colours had no light-mode pairs.
- **21ad8c0:** Data usage rounds down (4.96 GB of 5 GB must read "4.9 of 5", not "5.0").

### 11. Tests and CI flakiness

- **4de3788 + cfb303c:** When adding a gate, make main pass it in the same change, so it doesn't land red.
- **15311a6:** E2E against a local origin that can drop, stall, corrupt or hold at an exact byte. Every completed file is checked byte for byte. Known bugs are marked `test.fail()`. Test knobs (`PLEXO_E2E_*`) are ignored in packaged builds. **e01cf95:** Hold a request before snapshotting the content, otherwise there is a race with `setContent`.
- **fc6e675:** Retry Electron launch on ETXTBSY.
- **62fca99:** "Falls back to defaults" assumed `…Downloads`, which isn't true on Ubuntu CI. Compare against a fresh install instead.
- **aa3c256:** Fail every e2e on a listener-leak warning. **aed305b:** Check the stream/block invariants on every event.
- **4a23c10 / 2e4efd0 / 19f871b / 5404d92 ("Drop tests that pin UI details"):** The suite went from 209 to 167 tests (about 4 min down to 2) and later from 238 to 228. A flaky controller e2e near its 60 s timeout was replaced by unit simulations. **Rule:** tests guard risks, not constants or styling.
- **3272d6b:** Xvfb with `-noreset` (resetting when the last client exits raced the next parallel launch). Fixed screen size. Windows `taskkill /T`. Invariant scans switched from `expect` to `node:assert` (synchronous, no report step per block). Fixed a network-discovery click race. Nightly runs race-prone specs with `--repeat-each=10`.
- **8dae93f:** Disk test headroom for Chromium caches. **5404d92:** Shared seeder temp folder.

### 12. Release process, versioning and landing site

- **Versioning:** a straight `chore: bump version to 1.0.0-rc.N` commit, then a tag (rc.1–rc.14). Every release ships `--prerelease`.
- **3946d64 (rc.4):** The update check never fired, because GitHub's `/releases/latest` ignores pre-releases and returns 404. Uses `releases?per_page=1` instead. **Rule:** know which releases your update API ignores.
- **873cef9:** Artifact selection used `name.includes(version)`, so rc.1 also matched rc.10–rc.14 files. Fixed with an exact-version regex that also handles RPM's `.x86_64.rpm` naming. **Rule:** match versions exactly.
- **8daba37:** `docs/downloads.js` is the single source of what each file is. The primary button never offers the wrong architecture. Mobile visitors are told it's a desktop app. Metadata, blockmaps and stray files are hidden. `scripts/release-notes.mjs` builds the release body. A hostile file name can't inject markup.
- **efd3b3b:** A real 404 page (Cloudflare Pages otherwise served home with a 200). **6334b6a:** The footer wordmark never appeared on phones, because the IntersectionObserver's −8% root margin meant it could never intersect; the mobile menu pushed content down. **456164f:** Named grid areas for the download rows. Several poll and What's-new iterations (14b0514 through ed339ab) show churn from page-placement A/B decisions.
- **821059e:** Default focus goes to "Download", not "Not now".

---

### Timeline of milestones

- **09-13** 64231da: macOS multi-interface range downloader. Same day: reliability (6893072, c2e9281, 029387e) and work-stealing (2fdb33b).
- **09-14** 323ffae: v2 UI, grouped by physical network, custom titlebar.
- **09-15** MIT/README (49dba9f). Integrity PR #1 (9918671, a5f552f, 7911897, 515202b). CI gates (4de3788). Persistence (e56fbcf). Windows support (9cc4db5). Brand icon and theme (23d0340).
- **09-16/17** Assembling phase and simulator dev tool (669d780). Tailwind v4 and shadcn/Base UI migration (aeb3b6c…3dfdfdf).
- **09-18** E2E suite (15311a6). Seven-bug sweep (68b9939). Linux (81791d9). **rc.1** (1d0e3d3). Landing page (d914ecd). Update notifier (f03aedd). **rc.2**.
- **09-19** rc.3–rc.7 in one day: speed fixes, pre-release update check, multi-arch, crawl reconnect, `SO_BINDTODEVICE`.
- **09-22** Planner (3f3129b). Hedged scheduler (be75817, aed305b). Labelled downloads (8daba37). **rc.8**.
- **09-23** Settings persistence (7184397, 615150e). Same-subnet warning. **rc.9**.
- **09-25/26** IPv6 (6888ffa). NSIS options (97fdcc0). Direct-to-destination staging (0774aa6). Keep-alive (5a2358c). Automatic stream count (a323149). Delta IPC (5851ec0). Live networks (806a261). Retry timing (34541f1). Simulator removed (e20c5e9). **rc.10**.
- **09-26/27** Simpler stream count (f9d1d72). Per-download picker (d57778d). Silent refusals (8dae93f). **rc.11**.
- **10-04** Landing redesign and getplexo.app (9717f44, efd3b3b). **rc.12** (5404d92): torrents, queue, limits, history, list-as-home. **rc.13** (8cc15e4): Windows torrent hotfix. rpm (e5271f4).
- **10-07** Speed units and chart (21ad8c0). Disk-aware Auto and sparse NTFS (e8b84b6). Nightly fixes (3272d6b). **rc.14** (21ca1ef).

### Notable refactors and design pivots

1. **Fixed ranges → shared block queue → planned interleaving → hedging scheduler** (2fdb33b, 196ce9b, 3f3129b, be75817/aed305b). Each step came from a measured idle-worker problem.
2. **Block size tied to the grid → fixed 8 MB → file-sized 1–8 MB, with MAX_BLOCKS removed** (196ce9b, 3f3129b, 5851ec0). Display and work units were kept apart.
3. **Part files plus an assembly copy → a single staging file at offsets → in-place torrents** (0774aa6, 98e5896, 5404d92). Halves disk use, removes assembly, and lets hedges write directly (bdfa59f).
4. **localAddress → SO_BINDTODEVICE fd sockets → a routes layer (IPv4/6, Happy Eyeballs) → keep-alive agents** (32c1a69, 6888ffa, 34541f1, 5a2358c).
5. **Stream count:** user setting → measured trial controller (a323149, bb6a711) → simple doubling with refusal-based back-off (f9d1d72) → user picker restored (d57778d, and again in the 5404d92 dialog after it went missing) → disk-aware (e8b84b6). The lesson: the clever controller lost to a simple rule on noisy links.
6. **Fixed networks → live NetworkMonitor with reconcile()** (806a261). Failures are blamed on the network, and the download waits rather than failing.
7. **DownloadManager split** into the Transfer seam plus HttpTransfer (2,168 → 936 lines) before adding TorrentTransfer (5404d92). Refactoring first, with no test changes, made the torrent work additive.
8. **Own BitTorrent engine → patched webtorrent** (5404d92). Research overturned the plan.
9. **The simulated-download dev tool was added (669d780) and then deleted (e20c5e9)** once real-server e2e covered it. It had also caused bugs of its own (43099f5, 036749d).
10. **Settings consolidation:** one file, one channel, sanitized in one place, with network-prefs migrated (615150e).
11. **Inline styles → Tailwind/shadcn** (aeb3b6c–3dfdfdf). **Typed IPC contract** (715e257). **Single-download screens → a queue list as home with New download as a dialog** (5404d92).
12. **Manifest versions** 1→2 (IPv6) → 4/5 (compact per-network bytes) → 6 → 7. Old ones are cleared rather than migrated, which is acceptable only because the app is unsigned and reinstalled by hand.

### Top cross-cutting rules for us

1. Verify the server's answer (exact Content-Range, length, validators by sampling bytes), not just the status code.
2. Disk is the truth: reconcile before resuming, guard assembly, and delete partial output on failure.
3. Every network step needs its own timeout (DNS, connect, first byte, body progress), and all of them sit within a total deadline.
4. Tell apart where a failure came from: server refusal, network loss, disk backpressure, or sleep.
5. Keep-alive needs an Agent. A custom `createConnection` without one turns pooling off.
6. Per-OS routing: Linux needs `SO_BINDTODEVICE`, Windows routes by metric (warn on same subnet), and loopback is never pinned.
7. Windows: wait for handle `close`, retry rename on EPERM/EBUSY, sanitize reserved names, use sparse staging files, and kill process trees.
8. Packaging: set the architecture explicitly, assert package contents in `afterPack`, keep a CI job per platform, and never let a cross-build ship the host's native addons.
9. Releases: match exact versions, and remember `/releases/latest` ignores pre-releases.
10. UI: `truncate` needs `leading-normal`, global resets go in `@layer base`, base-ui uses `disabled` rather than `open={false}`, and errors go through one translator.
11. Tests: check invariants on every event, fail on Node warnings, isolate temp directories, use Xvfb `-noreset`, and delete tests that pin constants or styling.


---

# Parts 3–5: Error taxonomy, edge cases, patches and algorithms

Root: `/Users/arshramgarhia/Documents/sample/plexo`. Nothing was modified. Every file in scope was read in full. I also read `src/main/testKnobs.ts`, because most timing constants get their production defaults there, and a few renderer and IPC files to see how errors reach the screen. Citations are `file:line`. Paths are relative to `src/` unless they say otherwise.

**Production defaults from `main/testKnobs.ts:14-36`.** A packaged build ignores the env overrides (`:7`).

| Knob | Default | Line |
|---|---|---|
| blockBytes (max block) | 8 MiB | :18 |
| retryBaseDelayMs | 1000 | :19 |
| stallTimeoutMs | 20 000 | :20 |
| connectTimeoutMs | 10 000 | :21 |
| serverBusyForMs | 300 000 (5 min) | :24 |
| slowWarmupMs | 5 000 | :25 |
| slowForMs | 10 000 | :26 |
| silentAfterMs | 5 000 | :27 |
| hedgeAfterMs | 2 000 | :28 |
| magnetTimeoutMs | 180 000 | :30 |
| torrentDht | true | :32 |

---

### A) Error taxonomy

#### A.1 Error classes and kinds

| Class / kind | Where | Meaning | Classification |
|---|---|---|---|
| `HttpStatusError(status, retryAfterMs)`, message `Unexpected status N for range request` | `main/download/chunkDownloader.ts:46-60` | A chunk response that is neither 206 nor a valid 200-from-byte-0 (`:239-244`). | `transient` getter `:57-59` covers **408, 429, 500, 502, 503, 504** (curl `--retry` set): these are waited out. Everything else (401/403/404/410…) is a "strike". |
| `RemoteChangedError(check, seen)` | `chunkDownloader.ts:34-43` | A response's version (size / ETag / Last-Modified) differs from the accepted versions. Nothing from that response is written. | Size change means proof, and the download fails with its data discarded. A validator-only change goes to a byte-sample check (see A.3). |
| `ConnectionError(cause)` | `main/network/routes.ts:75-79`; wrapped by `asConnectionError` `:83-87` | Connect failed, dropped mid-response, or a stall watchdog fired. "Says something about the network, not the server." | **Never gives up**: retried forever with backoff, and no strike is counted (`httpTransfer.ts:908`). |
| `NoCompatibleRouteError(host)` | `routes.ts:66-70` | The selected interface has no address in the remote's IP family (for example, a redirect to an IPv6-only host). | Fatal for that **network**: `failNetwork` right away, no retry (`httpTransfer.ts:861-867`). Thrown at start if no selected network fits (`downloadManager.ts:815`). |
| `UnsafeTorrentError` | `main/download/torrent/paths.ts:14` | Torrent paths that escape the folder, collide, are too long, or are invalid on Windows. | Fatal at probe time. |
| AbortError (`DOMException 'AbortError'`) | `chunkDownloader.ts:143,199`; `routes.ts:119` | Pause, cancel, retire, or a refresh / lost abort. | Not an error. Mapped to outcome `stopped` or `aborted` (`httpTransfer.ts:686-691`). |
| Abort reasons `'refresh' \| 'lost'` | `httpTransfer.ts:33` | `refresh`: a stuck connection is swapped (no retry counted, no backoff). `lost`: a hedge race was decided by another attempt. | Not failures. |
| Attempt outcomes `completed / stopped / aborted / failed` | `httpTransfer.ts:69-75` | — | — |
| Plain `Error` protocol failures | `chunkDownloader.ts:227` "Too many redirects for range request"; `:261` 206 without usable Content-Range; `:267` wrong range start; `:276` overrun past rangeEnd; `:352` "more data than the requested range"; `:361` short body "Server returned X bytes for a Y-byte range" | Server answered wrongly. | Strikes, the same as non-transient status errors. |
| Write / disk errors (`fileStream.on('error', fail)`) | `chunkDownloader.ts:291,333` | ENOSPC, EIO and similar during a write. | Not `ConnectionError`, so they count as **strikes**. After more than 5 in a row the stream retires. When a network's last stream retires, the network fails. When every enabled network has failed, the download fails with that message. `describeError` then shows the ENOSPC text. |
| Publish errors | `downloadFile.ts:56` "Download file size does not match the expected size"; `downloadManager.ts:1414` "Download is incomplete — refusing to publish the file"; `downloadFile.ts:66,105` name exhaustion / extension too long | Consistency checks at publish. | Status becomes `error` with `resumable` left undefined, so it is treated as resumable (`downloadManager.ts:1446-1452`). |
| Start-time errors (thrown to IPC, shown in New Download dialog) | `downloadManager.ts:757` "Select at least one network"; `:764` "Add the torrent again to start downloading."; `:766` "Invalid torrent metadata"; `:297` "Not enough disk space: this download needs X GB but only Y GB is free"; `routes.ts:107` "Could not resolve the download host in time" | — | User-facing and fatal for that start attempt. |
| Probe errors | `probe.ts:20` NO_RESPONSE "The server did not respond — check the link and try again"; `:181` "Server responded with status N"; `torrent/metadata.ts:12-14` TOO_LARGE / V2_ONLY / NO_PEERS; `:37` "This isn't a torrent Plexo can read"; `:95` "This magnet link isn't valid"; `:121` "Replaced by a newer link" | — | Shown in NewDownloadDialog (`renderer/src/components/NewDownloadDialog.tsx:130`). |
| Relink errors | `downloadManager.ts:916` "Only web downloads support replacing a link."; `:919` "Pause the download before replacing its link."; `:921` "This download can't resume. Start it again."; `:923` "That isn't a link to a file"; `:926` "That link is to a different file: X GB, not Y GB"; `:930` "This server doesn't support resuming downloads…" | — | Shown in FixLinkDialog (`renderer/src/components/FixLinkDialog.tsx:42`). |
| Restore / resume errors | `downloadManager.ts:697` "The partial download file is missing. Remove this download and start again." (`resumable=false`); `:963` "The partial download file is unavailable. Reconnect the destination drive and try again." (status unchanged, not resumed) | — | — |
| Torrent file-choice errors | `torrent/files.ts:14,16`; `downloadManager.ts:1163,1174,1180` "A file that's already downloaded stays" | — | — |
| Trash / remove errors | `trashDownload.ts:17-19`; `torrent/ownedFiles.ts:29,35,43`; `torrentDestination.ts:117` | Refuse rather than delete something unsafe. | — |
| Cancel cleanup failure | `downloadManager.ts:1243-1251` | — | Status `error`, `resumable=false` ("retry cancellation rather than resuming"), then rethrown. |

#### A.2 `describeError`: translating errors for the user

`shared/errors.ts:97-106` strips Electron's `Error invoking remote method 'x': ` prefix (`:1`) and a nested `Error: ` (`:2`). It then tries regex hints in order, first match wins (`:7-90`):

1. `ENOSPC|EDQUOT` → "There isn't enough space to save this download. Free up space or choose another folder."
2. `EACCES|EPERM` → permission message
3. `ENOENT` → "The file or folder is missing…"
4. `EROFS` → read-only drive
5. `EIO` → "couldn't read or write the download. Check the drive"
6. Invalid torrent / infoHash / magnet → "couldn't read this torrent"
7. **`LINK_REFUSED = /status (401|403|404|410) for range request/`** (`:5`) → "This link no longer works. Paste a new link to continue." (the signed-link-expired case)
8. `Download is incomplete` → "couldn't download every part"
9. `Download file size does not match` → unexpected size
10. `ENOTFOUND|EAI_AGAIN` → can't find server
11. `ECONNREFUSED`
12. `ECONNRESET|socket hang up` → interrupted
13. `ETIMEDOUT|ESOCKETTIMEDOUT` → timed out
14. `/CERT|SSL|TLS/i` → certificate
15. `Invalid URL|ERR_INVALID_URL`
16. Probe-time `Server responded with status 401` → needs auth; then 403; 404; `4\d\d`; `5\d\d`

If nothing matches, the stripped raw message is returned.

`renderer/src/utils/format.ts:257-266` repeats `LINK_REFUSED` as `linkExpired()`. It is true for an HTTP download in `error` status whose message matches, and the UI then offers **"Fix link"** in place of Retry.

#### A.3 Retry and backoff policy (HTTP)

Constants in `main/download/httpTransfer.ts`:
- `MAX_CHUNK_RETRIES = 5` (`:121`)
- `RETRY_BASE_DELAY_MS = 1000` (`:122`)
- `RETRY_MAX_DELAY_MS = 15_000` (`:123`)
- `UNREACHABLE_RETRY_MS = 5_000` (`:126`)
- `SERVER_BUSY_FOR_MS = 300_000` (`:130`)
- `RETRY_AFTER_MAX_MS = 120_000` (`:131`)

**Backoff** (`:135-138`): `min(1000·2^(n−1), 15000) × (0.8 + 0.4·rand)`, so ±20% jitter "as gRPC's backoff has". The sequence is about 1, 2, 4, 8, 15, 15… seconds.

**Per-stream counters** (`ChunkRuntime`, `:98-105`):
- `failures` drives backoff. It resets when the attempt delivered any network bytes (`:886`), on block completion (`:784`), and on `wake()` (`:387`).
- `strikes` counts server-wrong answers. It resets when the attempt wrote any bytes (`:887-890`) or a block completed (`:785`).
- `busySince` is the start of the busy window. It resets with strikes.

**`finishFailed` decision tree** (`httpTransfer.ts:823-939`):
1. **RemoteChangedError** (`:833-859`).
   - A size check gives `'different'`. Anything else runs `confirmSameBytes` (`:1138-1185`). It reads up to `MAX_SAMPLES = 8` blocks that have data, spread evenly, up to `SAMPLE_BYTES = 16 KiB` each from disk (`:213-214`). It re-fetches each sample with up to `SAMPLE_TRIES = 4` tries (`:217`), skipping replies still showing an old label (useful behind load balancers, `:1175`).
   - `same`: push the new version into `acceptedVersions` and fetch again at once, with no retry counted (`:842-850`).
   - `different`: `failDownload(message, discard=true)`. This sets `resumable=false`, and the run then deletes the staging file (`downloadManager.ts:1406`).
   - `unknown`: fall through to an ordinary retry.
2. **NoCompatibleRouteError**: hand the block back and `failNetwork` (`:861-867`).
3. **ConnectionError** while the network has received nothing for `SILENT_AFTER_MS` (5 s): mark the network `unreachable` and reconcile (`:872-874`, `markUnreachable` `:503-508`).
4. **Hedge** attempts are never retried (`:877-880`).
5. `network.retries += 1` only if the status is `on` (`:883`). This is the per-network "retries" number shown in the UI.
6. A 403, 429 or 503 sets `self.refused = true`, which feeds connection-limit detection (`:902-904`).
7. A transient status starts `busySince`. It is "waiting out" while `now − busySince < 5 min` (`:905-907`).
8. **Give up**: when the error is not a `ConnectionError`, `++strikes > 5`, and the stream is not waiting out a busy server, the stream retires. If the network has no live streams left, `failNetwork(network, message)` runs (`:908-914`).
9. Otherwise wait `retryDelayMs(failures)`, capped at 5 s if the network is `unreachable` (`:916-917`).
10. **Retry-After** (`:918-924`). Parsed by `retryAfterMs` in RFC 9110 seconds or HTTP-date form (`chunkDownloader.ts:63-69`). It applies only to transient statuses. The wait is `max(backoff, min(RetryAfter, 120 s))`, and `holdUntil[network]` is set so that **no stream on that network** sends a new request until then. The hold is per network (per source address), not per request. Each held stream waits `holdFor + rand·min(1000, holdFor/10)` (`:1067-1074`).
11. Stream status becomes `retrying` during the wait. The wait is interruptible through `nudge` (`wake`) or stop (`:397-400`).

**Stale keep-alive socket**: one free retry on a fresh socket if `req.reusedSocket` was set and it was not a timeout or abort (`routes.ts:406-410`).

#### A.4 Per-chunk, per-network and per-download handling

- **Per chunk/attempt.** A failure hands the block back to the queue (`letGo`, `httpTransfer.ts:946-958`). The written prefix is kept. If the attempt delivered nothing, its network goes into `avoidNetworkByBlock`, so another network gets that block first (`:956`, `scheduler.ts:64-79`).
- **Per stream.** Strikes lead to retirement (`:909`). Stuck-connection refresh is not a failure (`:460-499`).
- **Per network.** `failNetwork` sets status `failed` and stores `error` (`downloadManager.ts:1479-1483`). Reconcile then drops its streams to 0 (`httpTransfer.ts:276-279,297`). Switching it off and on, reconnecting, or resuming retries it: `begin()` resets `failed` and `unreachable` to `on` (`downloadManager.ts:1007-1013`). `unreachable` keeps exactly **one** probing stream (`httpTransfer.ts:299-300`) and flips back to `on` at the first byte received (`:702-706`).
- **Per download.** It fails when every enabled network is `failed`, using the first one's error (`downloadManager.ts:1550-1554`). It also fails on RemoteChanged/different (discard), on torrent client or torrent errors (`torrentTransfer.ts:170-173,178,230`), and on publish failure. A notification "Download failed: <describeError>" is shown (`downloadManager.ts:1473`). `failDownload` is a no-op unless the status is `downloading` (`:1469`).
- **No network at all** (all off, offline or at their limit) is **not** an error. The download stays `downloading` with zero streams, and the UI explains why (see A.6).

#### A.5 State machines

**DownloadStatus** = `queued | downloading | paused | completed | error | cancelled` (`shared/types.ts:68-69`).

| From | Event | To | Code |
|---|---|---|---|
| (new) | `start()` with room (`running < downloadsAtOnce`) | downloading, then `launch` | `downloadManager.ts:858-864` |
| (new) | `start()` with no room | queued (at the back; `queuedAt = max(now, others+1)`) | `:860`, `enqueue` `:373-383` |
| queued | `pump()` finds room (FIFO by `queuedAt`) | downloading (`begin`) | `:361-370,989` |
| queued | pause | paused | `:876-883` |
| queued / paused / error / downloading | cancel | paused (transient), then cancelled after files and manifest are removed | `:1223-1242` |
| downloading | pause / quit (`suspendAll`) / last enabled network switched off (`pausedForNoNetwork`) | paused | `:886-908,1319-1329,1063-1068` |
| downloading | `failDownload` / all networks failed / publish error | error (`resumable = !discard`) | `:1468-1475,1551,1446` |
| downloading | all units done, then publish succeeds | completed, then moved to history (removed from runtimes) | `:1411-1457,548-579` |
| paused | resume with room | downloading | `:940-986` |
| paused | resume with no room | queued at the **front** (`min(now, others)−1`) | `:976-983` |
| paused (no network) | a network switched back on | resumes (`resumeAfterVerifying(byNetwork=true)`) | `:1073-1075` |
| error (`resumable !== false`) | resume / relink | downloading or queued | `:944,914-938` |
| error (`resumable === false`) | — | only cancel, remove, or "Download again" | `ErrorScreen.tsx:35` |
| cancel cleanup throws | — | error, `resumable=false` | `:1243-1251` |
| relaunch | downloading/queued in manifest | paused (`pausedAt = savedAt`) | `:661-665` |
| relaunch | paused, all units done, published file has same dev/ino and expected size | completed, then history | `:688-694,540` |
| relaunch | partial file missing | error, `resumable=false` | `:695-698` |

Rules within these transitions:
- Queue time counts as paused time (`pausedAt ??= now` in `enqueue`; added to `totalPausedMs` in `begin`).
- Pause is refused while `publishing` (`:884`).
- Cancel is refused while publishing (`:1207`).
- Lowering `downloadsAtOnce` leaves running downloads to finish (`:326-333`).

**NetworkStatus** = `on | off | offline | unreachable | failed | limit` (`types.ts:148-159`). Reconcile derives it (`downloadManager.ts:1534-1546`):
- not enabled → `off`
- else, not present and the monitor has looked → `offline`
- else `limits.limitReached` → `limit`
- else if the previous status was off, offline or limit → `on`
- else keep the previous one (`unreachable` or `failed` stick)

Other transitions: `on → unreachable` (`markUnreachable`, or the torrent rule); `unreachable → on` on first byte or wire; `→ failed` through `failNetwork`; `failed/unreachable → on` on `begin()`. A network that appears mid-download is added as `off` (`:1520-1525`). A never-used vanished network is dropped from the list (`:1503-1509`).

**HttpStreamStatus** = `pending | downloading | retrying | paused | completed | cancelled` (`types.ts:75-76`). `pending` means idle with no block. `completed` means no work left (`httpTransfer.ts:1105`). A retired stream leaves the list.

**Block status**: `pending | downloading | completed`, plus `skipped` for torrent pieces (`types.ts:113-115`).

#### A.6 How errors and states reach the user

- **Downloads list row** (`renderer/src/screens/DownloadsScreen.tsx:633-647`): `describeError(error ?? 'Something went wrong')` in danger color. The action is **Fix link** if `linkExpired`, **Retry** if `resumable !== false`, otherwise **Download again**. The progress bar turns red on error (`:650-657`). A bulk "Resume (n)" excludes expired links (`:164-169`).
- **ErrorScreen** (`renderer/src/screens/ErrorScreen.tsx:25-90`): heading "Download failed" or "Download cancelled". The body is `describeError(error)` or "The download stopped unexpectedly. Try again." Buttons: Fix link, Retry (only if resumable and bytes > 0), or Download again.
- **DownloadingScreen** `waitingFor` (`renderer/src/screens/DownloadingScreen.tsx:87-99`):
  - "Can't reach the server. Retrying…" (torrent: "Can't reach peers. Retrying…")
  - "Every network in use has reached its data limit. Raise one in Speed & data limits."
  - "Waiting for a network. Reconnect one or switch one on."
  - It also shows a `diskLimited` badge with tooltip (`:266`), and `describeError(download.error)` (`:303`).
- **Network row status text** (`renderer/src/components/NetworkRow.tsx:24-30`): Off / Not connected / Can't reach server (torrent: Can't reach peers) / Failed / Data limit reached. The tooltip shows `describeError(network.error)` (`:450`).
- **OS notifications**: "Download complete — X has finished downloading." and "Download failed — X: <friendly>" (`downloadManager.ts:1445,1449-1452,1473`). Suppressed under tests.
- **Dialogs**: NewDownloadDialog (probe and start errors), FixLinkDialog (relink), LimitsDialog, TorrentFiles, DetailHeader all use `describeError`.

---

### B) Edge cases from code comments, and the patches

#### B.1 Topic by topic

**Content-Disposition** (`main/download/probe.ts:65-122`)
- `filename*` takes precedence (RFC 6266 / RFC 5987), regex `filename\*=charset'lang'value`.
- **ISO-8859-1** is decoded byte by byte with `String.fromCharCode`, because `decodeURIComponent` only reads UTF-8: "%A3 (£) would throw" (`:70-76`). Other charsets use `decodeURIComponent`, falling back to the raw value.
- Next comes a quoted `filename="…"`, which keeps semicolons inside the quotes and undoes backslash escapes (`:84-93`). Then an unquoted token (`:95-103`). Both are `%`-decoded best-effort.
- Fallback: the last segment of the URL path, `%`-decoded (malformed escapes keep the raw path), else `download` (`:108-122`).
- The name is then sanitized (B.1 collisions).

**Probe: 206 vs 200, servers ignoring Range** (`probe.ts:34-63,159-217`)
- The probe sends `GET` with `Range: bytes=0-0`, not HEAD: "unlike HEAD it also tells us … whether range requests actually work."
- `supportsRanges = status === 206` only. A 200 means Range was ignored "even if its headers statically claim `Accept-Ranges: bytes`" (`:189-193`).
- The total comes from `Content-Range /N`. `Content-Length` is trusted only on a 200 ("on a 206 it describes the single byte", `:200-205`).
- **416 with `bytes */0`** means a valid empty file (`:161-178`). Status 0 or ≥ 400 throws `Server responded with status N`.
- Torrent detection: `Content-Type: application/x-bittorrent` or a path ending in `.torrent` (`:146-149`).
- The probe uses plain `http(s).request`, so the **default route and default agent, not bound to any chosen interface** (`:36-63`).

**Chunk 206 vs 200** (`chunkDownloader.ts:202-283`)
- No `Range` header when fetching the whole file from 0. An empty file would answer `bytes=0-` with 416 (`:202-208`).
- A 200 is accepted only when `rangeStart === 0`. Otherwise the server ignored Range "and we'd silently write the wrong bytes" (`:234-244`). For a bounded range, the end-of-body length check confirms the server didn't send the whole file.
- A 206 must have a parseable `Content-Range: bytes s-e/total|*` (`:88-98`), with `start === rangeStart` and `end ≤ rangeEnd`.
- The body is written by hand, not piped, so an overlong body is cut at the range boundary ("could overwrite the next block", `:314-354`).
- A short body is rejected at `end` (`:356-366`).

**ETag / Last-Modified / If-Range**
- **No `If-Range` header is ever sent.** Each response's validators are checked after the fact by `compareVersion` (`main/download/fileVersion.ts:31-49`), against `acceptedVersions`, which starts as the probe's etag, lastModified and totalBytes (`httpTransfer.ts:207-209,259`).
- A size mismatch is proof of a change. An ETag mismatch is only a hint, settled by byte sampling. A Last-Modified mismatch is used only if no ETag was available.
- `normalizeEtag` strips `W/`, quotes, and `-gzip|br|deflate|zstd` or `;gzip` suffixes (Apache, `fileVersion.ts:21-27`).
- Comment: "a file republished mid-download would otherwise be stitched together from two versions and still pass every length check" (`chunkDownloader.ts:246-255`).
- `versionOf`: a 206 takes totalBytes from Content-Range; a 200 takes it from Content-Length (`:105-114`).
- Pause/resume: the first chunk after resume carries the check ("can tell a real change from a relabelled server", `downloadManager.ts:949-953`). `acceptedVersions` is not persisted; confirmations are redone after a restart (`httpTransfer.ts:230-232`).

**Redirects**
- Probe: up to `MAX_REDIRECTS = 5` (`probe.ts:14`), with relative Location resolved against the current URL. The whole chain shares one 20 s deadline, "so a chain of slow hops can't stretch it" (`:16-19,124-143`).
- The download uses `probe.finalUrl` (`types.ts:26-27`).
- Chunks follow up to 5 redirects per request ("if a CDN reissues a redirect mid-download (e.g. a signed URL rotates)", `chunkDownloader.ts:76-79,224-232`). The redirect target is **not** remembered, so each new request starts again from the stored URL. A 3xx without Location is treated as "Too many redirects".
- **Per-network redirect**: the redirect is followed on the same `StreamConnection`, which is bound to that network. If the new host has no compatible IP family on that interface, `NoCompatibleRouteError` causes `failNetwork` ("A redirect can move this worker to a host its network cannot reach", `httpTransfer.ts:861-867`).
- `fetchRange` sample requests also follow up to 5 redirects (`chunkDownloader.ts:399-403`).

**Expired / signed links**
- 401, 403, 404 or 410 on a range request (`errors.ts:4-5`) end, after 5 strikes per stream across all networks, in a download error. The UI then offers **Fix link**.
- `relink()` (`downloadManager.ts:911-938`): requires status paused or error and resumable, re-probes, requires the **same totalBytes**, requires range support if the download was splittable, replaces the URL with `finalUrl`, persists, and resumes. Byte identity is then checked by the normal version check.

**403 / 429 / 503 and Retry-After.** See A.3. In addition, 403, 429, 503 and "left unanswered (silent)" count as **refusals**, which signal a per-IP connection limit to the concurrency controller (`concurrency.ts:9-20`, `httpTransfer.ts:485,902-904`). A refusal while sending nothing "isn't limiting connections: it is busy, or the link has expired" (`concurrency.ts:18-20`).

**Silent / stalled sockets and timeouts**
- Connect (DNS + TCP + TLS) is bounded by `min(connectTimeoutMs = 10 s, timeoutMs = 20 s)` (`routes.ts:332`); error "Connection stalled: could not connect" (`:188-191`).
- Response headers: a 20 s timer per request (`routes.ts:395-398`), "Connection stalled: no response from server".
- Body: a stall watchdog of 20 s between `data` events (`chunkDownloader.ts:71-74,162-167,295,318`). It is **cleared while reading is held** for disk backpressure or throttling (`:297-312`).
- Probe: 20 s total (`probe.ts:19`). Sample fetch: `res.setTimeout(20 s)` (`chunkDownloader.ts:411`).
- **Silent attempt** (`httpTransfer.ts:510-517`): no network bytes for 5 s since start, while some other block has bytes ("Until something has arrived, a quiet origin is just a slow one"). The attempt is refreshed and counted as a refusal. A whole network silent for 5 s goes to `unreachable` (`:476-482`).
- **Crawling** (`:519-538`): after a 5 s warm-up, speed below `SLOW_RATIO = 0.1` (`:144`) × the median of the same network's reference speeds (other warm streams' current speed plus every stream's last finished-block speed, its own included) for 10 s. Then refresh. Networks are never compared with each other ("cellular is expected to be slower than Wi-Fi", `:449-455`).
- `MAX_REFRESHES_PER_BLOCK = 2` (`:153`). Hedges are dropped, never counted (`:489-491`). No refresh at all without range support, "a reconnect restarts the whole file from byte 0" (`:461-462`). An attempt waiting on writes is exempt ("the disk set its pace", `:470-475`).
- After a refresh, the stream waits `REFRESH_HANDOFF_MS = IDLE_POLL_MS (250) + 50 = 300 ms` "so that a connection already proven fast… gets to the block" (`:184-187,812-819`).

**Sleep / wake**
- `powerMonitor.on('resume')` calls `manager.systemResumed()` and `networks.refresh()` (`main/ipc/handlers.ts:83-86`).
- HTTP (`httpTransfer.ts:366-376`): `traffic.aliveAt = now`, `warmSince = now`, `slowSince = null`, then `wake(all, reconnectAll)`: aborts attempts as `refresh`, destroys agent sockets, and nudges backoffs. Comment: "Its sockets are likely dead… every judgement made by the clock spans the sleep" (`downloadManager.ts:1110-1119`).
- Torrent: `reach.answeredAt = now` (`torrentTransfer.ts:137-140`).
- Prevent sleep: `powerSaveBlocker.start('prevent-app-suspension')` while any download is running (display may still sleep), in a try/catch (`downloadManager.ts:1121-1137`).

**IP changes / interface disappearing** (`downloadManager.ts:1078-1108`)
- Each poll compares each interface's address set. A *changed* set wakes that network's streams: `failures = 0` and the backoff is cut short. A *moved* set (an address was lost) also aborts in-flight attempts as refresh and calls `connection.reconnect()`, "Chrome does the same… (ERR_NETWORK_CHANGED)".
- Every new socket re-reads the interface's current addresses (`routes.ts:297-299,324-326`). A missing interface gives "The network is not connected".
- Torrent: `wake` destroys wires on networks that moved (`torrentTransfer.ts:132-135`). A network switched off or gone has its wires destroyed in `reconcile` (`:81-84`).
- Linux: if the interface vanished between listing and `socket()`, the error surfaces as a socket error on the next tick (`deviceBinding.ts:77-81`).

**Filename collisions, staging, partial naming** (`main/download/paths.ts`)
- `sanitizeFileName` (`:28-36`): `/` and `\` become `_`, Unicode control characters become `_`. On Windows also `<>:"|?*` become `_`, trailing dots and spaces are stripped, and reserved `CON|PRN|AUX|NUL|COM1-9¹²³|LPT1-9¹²³` get a `_` prefix. An empty or all-dot name becomes `download`.
- `claimName` (`:56-85`): tries `name`, `name (1)`, … `MAX_NAME_ATTEMPTS = 10_000` (`:16`). The base is truncated by **UTF-8 bytes** (code-point safe) so that `base + " (9999)" + suffix + ext ≤ 255` bytes. An EEXIST from the claim means "try the next name".
- HTTP staging: `reserveDestinationPath` creates `<final>.plexo` exclusively (`open 'wx+'`). The claim is kept only if `<final>` itself doesn't exist; otherwise the `.plexo` is removed and the next name is tried. It is then marked sparse (`:92-108`). "The final name itself does not appear until the file is complete."
- Publish (`downloadFile.ts:50-106`): verifies size, then fsyncs. It then loops candidates again: skips one whose sibling `.plexo` belongs to another download, persists publication intent (`beforeAttempt`), re-checks the target doesn't exist, and `rename()`s (same directory, no copy). The acknowledged race: "Node has no portable no-replace rename" (`:84-87`).
- Torrents are written **in place** under a claimed final name: a folder via `mkdir`, or a file via `open 'wx'` (`paths.ts:115-126`). There is no `.plexo` for torrents.
- Old versions (rc.1–rc.9, manifest v2) claimed the final name as an empty file. Cleanup removes it only while it is still empty and unfinished (`downloadManager.ts:253-284`).

**Free space** (`downloadManager.ts:290-300`)
- `statfs(destDir)`: `bavail × bsize` against required bytes, checked once at start (HTTP total, `:823`; torrent total minus skipped, `:772`) and when adding torrent files (`:1189`).
- Skipped for unknown size. Not re-checked during the download (ENOSPC is then mapped by `describeError`).
- No double space is needed, because staging sits beside the final file and becomes it by rename (`downloadFile.ts:9-10`).

**Sparse files**
- NTFS (`main/download/sparseFile.ts:3-9`): writing past the end zero-fills the gap synchronously, so scattered streams would write most of the file twice. Plexo calls `FSCTL_SET_SPARSE (0x900c4)` through koffi → `kernel32 CreateFileW(GENERIC_WRITE, SHARE_ALL=0x7, OPEN_EXISTING)` + `DeviceIoControl`. It is Windows-only, never throws, and FAT32/exFAT refusal is ignored.
- APFS and ext4 get no explicit handling: positional `r+` writes into the empty `wx+`-created file make holes naturally. No preallocation or `ftruncate` is done.
- Torrent "files are sparse"; `size()` returns `totalBytes` once the path exists (`torrentDestination.ts:69-74`).

**Disk can't keep up (write backpressure)**
- Each writer is its own `createWriteStream(path, {flags:'r+', start, highWaterMark: 512 KiB})` (`downloadFile.ts:5-23`). "32 streams hold at most 16 MiB." Writes batch into one writev. Append mode is never used.
- When `write()` returns false, the response is paused (`res.pause`), `onWriteWait(true)` is reported, and reading resumes on `drain` (`chunkDownloader.ts:329-343`). End-of-body flush counts as waiting, but "closing the file isn't the disk falling behind" (`:367-371`).
- Two progress counters exist: `networkReceived` (network health) and `received`, which counts only bytes the writer accepted and is the only "safe to resume from" figure (`httpTransfer.ts:51-54`).
- `DiskWatch` and its cap are covered in C.4. `diskLimited` is shown to the user. Hedges are suppressed while the disk is behind (`scheduler.ts:36-37,90`).
- When a write fails, buffered data is still flushed (`stream.end()`) before rejecting, unless the stream itself errored. This prevents fd leaks and "buffered writes landing after a retry has started" (`chunkDownloader.ts:177-197`). It waits for `close` before resolving, because "Windows cannot reliably reopen/remove a file until its handle closes" (`:372-373`).

**IPv6**
- Link-local `fe80:` and `169.254.*` addresses are excluded (`interfaces.ts:143-149`).
- URL brackets are stripped (`routes.ts:27-30`, `probe.ts:42`).
- Routes pair only matching families (`routes.ts:43-57`).
- Happy Eyeballs (RFC 8305): `ATTEMPT_DELAY_MS = 250` stagger, families alternate, the last good route is tried first per `iface host:port`, and a failure hands over immediately (`:121-235`).
- The Linux binding sets both device and `localAddress`, "an interface may have several IPv6 addresses" (`deviceBinding.ts:83-90`).
- Torrent: an incoming IPv4-mapped `::ffff:` address is normalised (`torrentTransfer.ts:305-306`). Peer addresses are bracketed for v6 (`:28-29`). Hostname peers are rejected because they have no family (`:248-249`).

**DNS** (`routes.ts:32-40,89-117`)
- `dns.lookup(all:true, order:'verbatim')` keeps OS order. "DNS is part of opening a connection, so it must not outlive the connection's deadline": `resolveTargetWithin` races a timer and is abortable.
- At start, a DNS lookup with a 20 s limit decides which selected networks are compatible (`downloadManager.ts:811-815`).
- Resolution is **not per interface**: it uses the system resolver and only filters routes by family per interface. `ENOTFOUND` and `EAI_AGAIN` are wrapped as ConnectionError, so they are retried forever.

**tmpfs**: nothing specific. Only the magnet probe writes to `os.tmpdir()/plexo-magnet` with `deselect:true` and `destroyStore:true` (`torrent/metadata.ts:107,114`).

**Torrent path sanitisation** (`torrent/paths.ts:16-67`)
- Mirrors fs-chunk-store exactly: names are stripped of `[<>:"/\\|?*\u0000-\u001F]` (`STRIPPED_FROM_NAMES` `:6`), and the path is resolved under a fake `/staging` base.
- Rejected: an empty name, climbing out (`..`), or an absolute path. Any segment over 255 bytes is rejected.
- On Windows also rejected: reserved names *with* an extension (`CON.txt`) and invalid characters or a trailing dot or space.
- Case-insensitive duplicate files are rejected, as is a file whose path is also a folder prefix.
- Removal (`torrent/ownedFiles.ts`) validates every path before touching any, refuses symlinked parent folders, and refuses a target that became a folder. Trash only moves owned files and removes empty folders.

**Restart / persistence integrity** (`downloadManager.ts:476-722`)
- Stale `manifest.json.tmp` is removed.
- A SyntaxError or ENOENT manifest is discarded. Other read errors (drive not mounted, permission) are left for the next launch.
- A manifest of another version is discarded along with its unmistakable `.plexo` leftovers.
- Two manifests sharing a partial path: the newest is kept (`:512-525`).
- Progress beyond the file size, or a "completed" block without its full length, resets that block (`:700-711`).
- Torrent: if the `.torrent` is missing the download can't be restored.
- Publication crash safety: before rename, `publicationPath` and the partial's `{dev, ino}` are persisted (with `required=true`, so the error propagates, `:1420-1425`). On restore, a file at the published path with the same dev/ino means the download is completed (`:672-694`).

**Other**
- Unknown size: a single open-ended block, one stream on one network (`plan.ts:75-76,114`; `httpTransfer.ts:160-164`). Saved `complete` is needed because the length is unknown (`savedProgress.ts:10-12`).
- A non-splittable download switches networks rather than adding them (`downloadManager.ts:829-832,1057-1060`).
- Every block is claimed once before any network gets a second (interleave), so "a small file shouldn't all go to whichever network came first" (`httpTransfer.ts:318-320`).
- Windows `jsonFile` rename retries for antivirus or indexer locks (`jsonFile.ts:25-37`).
- macOS `networksetup` output is parsed **by position**, because labels are localised ("Matériel", "WLAN") (`interfaces.ts:29-31`).
- Windows PowerShell output has a UTF-8 BOM stripped, and a single object is not wrapped in an array (`:85-86`).
- Quit runs `suspendAll`, which pauses and persists everything, with a 3 s forced `app.exit` (`main/index.ts:147-161`).
- A removed download that is still winding down must not push updates (`downloadManager.ts:1588-1590`).
- Data limit: "Told once the caller is done with its bytes: what it does may stop the very connection they came in on" (setImmediate, `limits.ts:121-125`).
- Torrent: "webtorrent's pipe resumes the socket when its wire drains, which can cut a wait short; the debt… only lengthens the next one" (`torrentTransfer.ts:293-296`).
- Torrent edge pieces hold neighbours' bytes; unwanted files are removed at publish (`files.ts:36-40`, `torrentDestination.ts:54-67`).
- A torrent piece found missing on disk by webtorrent, though marked done, is re-downloaded (`torrentTransfer.ts:413-433`).

#### B.2 The patches (applied by `patch-package` in `postinstall`)

`package.json:21`: `"postinstall": "patch-package && install-electron && electron-builder install-app-deps"`. `patch-package` is pinned at `8.0.1` (`:61`), `webtorrent` exactly at `3.0.21` (`:40`). `fs-chunk-store 5.0.1`, `parse-torrent 11.0.24` and `bittorrent-peerid 2.0.2` are also exact. `koffi ^2.16.3` (`:37`) does the native FFI for SO_BINDTODEVICE and FSCTL_SET_SPARSE. `allowScripts` still lists `node-datachannel@0.32.3` (`:75`).

1. **`patches/webtorrent+3.0.21.patch`**
   - `node_modules/webtorrent/index.js` constructor: adds `this._connect = typeof opts.connect === 'function' ? opts.connect : null`, next to `maxConns`.
   - `node_modules/webtorrent/lib/torrent.js` around line 2122 (outgoing TCP peer dial): `peer.conn = net.connect(opts)` becomes `peer.conn = this.client._connect ? this.client._connect(opts) : net.connect(opts)`. The uTP branch just above is untouched.
   - **Why**: it gives Plexo a hook to pick the network per peer and build the socket itself through `connectRoute` (localAddress, plus SO_BINDTODEVICE on Linux), the same mechanism HTTP uses. Plexo passes `connect: (o) => this.connect(o)` (`torrentTransfer.ts:151-154`). uTP isn't hooked, so it is disabled (`engine.ts:60,71`). The types are declared in `torrent/modules.d.ts` (`connect?: (options:{host;port}) => Socket`).

2. **`patches/@thaunknown+simple-peer+10.1.2.patch`**
   - `lite.js`: `import { RTCPeerConnection, RTCSessionDescription, RTCIceCandidate } from 'webrtc-polyfill'` becomes `const { RTCPeerConnection, RTCSessionDescription, RTCIceCandidate } = globalThis`, with the comment "Plexo binds TCP peers to selected networks; WebRTC peers bypass that hook. Avoid loading the unused native polyfill when importing the torrent engine."
   - **Why**: importing webtorrent no longer pulls the native `node-datachannel` addon through `webrtc-polyfill`. In Electron's main process the globals are undefined, so WebRTC simply isn't available. It is turned off anyway (`tracker: { wrtc: false }`, `engine.ts:62-63,73`).

#### B.3 Edge cases our engine must handle (with citations)

1. Probe with `GET Range: bytes=0-0`, not HEAD, to learn real range support. `probe.ts:34-46`
2. Treat range support as only `status === 206`; ignore `Accept-Ranges`. `probe.ts:190-193`
3. An empty file answers 416 `bytes */0`: a valid 0-byte download. `probe.ts:161-178`
4. Trust `Content-Length` only on a 200, never on a 206. `probe.ts:200-205`
5. Total size from `Content-Range .../N`; handle `*`. `probe.ts:196-199`, `chunkDownloader.ts:88-98`
6. One time budget across the whole redirect chain. `probe.ts:16-19,129`
7. Cap redirects at 5; resolve relative `Location`. `probe.ts:14,131-140`
8. A 3xx without Location ends redirect following. `probe.ts:135`, `chunkDownloader.ts:226-228`
9. Detect torrents by MIME type or `.torrent` extension. `probe.ts:146-149`
10. Content-Disposition: `filename*` takes precedence over `filename`. `probe.ts:66-82`
11. RFC 5987 ISO-8859-1 decoding, done separately from UTF-8. `probe.ts:70-76`
12. Quoted filename with `;` inside and backslash escapes. `probe.ts:84-93`
13. Malformed `%` escapes fall back to the raw value. `probe.ts:77-81,115-119`
14. Name from the URL path when no header, else `download`. `probe.ts:114-121`
15. Sanitize server names: separators, control characters, Windows reserved names and characters, trailing dots. `paths.ts:28-36`
16. 255-**byte** component limit, truncating whole code points, with room for ` (9999).plexo`. `paths.ts:65-69`
17. Collision suffixes `name (N).ext` up to 10 000 attempts. `paths.ts:16,71-84`
18. Folder names get no extension split. `paths.ts:120-121`
19. Exclusive create of the staging `.plexo` (`wx+`) as the reservation. `paths.ts:98`
20. A staging claim must also check the final name is free. `paths.ts:100-105`
21. Re-check collisions at publish and skip names another download's `.plexo` holds. `downloadFile.ts:73-82`
22. Persist publish intent (path plus dev/ino) before the rename, for crash recovery. `downloadManager.ts:1420-1425`, `downloadFile.ts:83-94`
23. No portable no-replace rename: check just before renaming. `downloadFile.ts:84-87`
24. Same-directory rename, so publishing needs no copy and no extra space. `downloadFile.ts:96-99`
25. Verify staging size equals expected size before publishing. `downloadFile.ts:55-57`
26. Refuse to publish if any unit is incomplete. `downloadManager.ts:1413-1415`
27. Free-space check with `statfs` (`bavail*bsize`) before starting; skip when size is unknown. `downloadManager.ts:291-300`
28. NTFS: mark staging sparse, or scattered writes zero-fill twice. `sparseFile.ts:3-9,57-94`
29. Positional writers with their own fd and `r+`, never append. `downloadFile.ts:14-23`
30. Bounded write buffer of 512 KiB per stream to detect a slow disk. `downloadFile.ts:5-7`
31. Pause the socket on `write()` false and resume on drain (backpressure). `chunkDownloader.ts:332-343`
32. Clear the stall watchdog while held for disk or throttle. `chunkDownloader.ts:297-312`
33. Track network bytes separately from bytes the writer accepted; resume only from accepted bytes. `httpTransfer.ts:51-54`, `chunkDownloader.ts:16-19`
34. On failure, flush or close the writer before rejecting (fd leak, late writes). `chunkDownloader.ts:177-197`
35. Windows: resolve only after the file handle is closed. `chunkDownloader.ts:372-373`
36. No Range header for a full fetch from byte 0 (avoids 416 on empty files). `chunkDownloader.ts:202-208`
37. A 200 on a ranged request with start > 0 means Range was ignored: reject. `chunkDownloader.ts:234-244`
38. A 206 must carry a parseable Content-Range. `chunkDownloader.ts:259-263`
39. A 206 starting at the wrong offset: reject. `chunkDownloader.ts:265-272`
40. A 206 running past rangeEnd: reject. `chunkDownloader.ts:274-281`
41. A body longer than the range: truncate at the boundary and fail. `chunkDownloader.ts:314-354`
42. A body shorter than expected: fail. `chunkDownloader.ts:359-366`
43. Version check on every response (size, ETag, Last-Modified), so the file isn't stitched from two versions. `chunkDownloader.ts:246-255`, `fileVersion.ts:31-49`
44. Normalize ETags (`W/`, quotes, `-gzip`/`-br` suffixes). `fileVersion.ts:21-27`
45. Load balancers labelling the same bytes differently: settle by sampling 8×16 KiB, 4 tries each. `httpTransfer.ts:211-217,1138-1185`
46. A size change is proof of a new file: fail and discard. `fileVersion.ts:33-35`, `httpTransfer.ts:834-855`
47. Follow mid-download redirects per chunk, e.g. a rotating signed URL. `chunkDownloader.ts:76-79,224-232`
48. A redirect to a host the bound interface can't reach: fail only that network. `httpTransfer.ts:861-867`
49. Expired signed link (401/403/404/410): an actionable "Fix link" that keeps the bytes. `errors.ts:4-5,33-35`, `downloadManager.ts:911-938`
50. Relink requires an identical size and range support. `downloadManager.ts:924-933`
51. Transient statuses 408/429/5xx are waited out for 5 min, not 5 retries. `chunkDownloader.ts:55-59`, `httpTransfer.ts:905-908`
52. Retry-After as seconds or HTTP-date, capped at 120 s, held per network. `chunkDownloader.ts:62-69`, `httpTransfer.ts:918-924,1065-1074`
53. Jittered exponential backoff (±20%) so streams don't stampede. `httpTransfer.ts:133-138`
54. Connection errors never exhaust retries while the network exists. `httpTransfer.ts:895-898,908`, `routes.ts:72-74`
55. A network that is wholly silent becomes `unreachable`, keeping one probe stream and 5 s retries. `httpTransfer.ts:124-126,299-300,503-508,917`
56. A silent connection while others are served: refresh, and count it as a refusal. `httpTransfer.ts:476-486,510-517`
57. A crawling TCP stream (<10% of the same network's median for 10 s): reconnect. `httpTransfer.ts:140-146,519-538`
58. Cap refreshes at 2 per block. `httpTransfer.ts:151-153,487-496`
59. No refresh when the server lacks ranges (it would restart from 0). `httpTransfer.ts:461-462`
60. A stale keep-alive socket closed by the server: one silent retry on a fresh socket. `routes.ts:406-410`
61. Bound connect time (DNS+TCP+TLS 10 s), header time (20 s) and body stall (20 s). `routes.ts:316-332,395-398`, `chunkDownloader.ts:71-74`
62. Abort an in-progress handshake when its request is abandoned. `routes.ts:333-337`
63. DNS must respect the connect deadline and abort signal. `routes.ts:89-117`
64. Happy Eyeballs: 250 ms stagger, alternating families, sticky last-good route. `routes.ts:121-235`
65. Exclude link-local IPv4 (169.254) and IPv6 (fe80) source addresses. `interfaces.ts:143-149`
66. Strip IPv6 URL brackets before socket or isIP use. `routes.ts:27-30`, `probe.ts:42`
67. A route requires matching IP families between local and remote. `routes.ts:43-57`
68. Linux: localAddress alone still leaves by the default route, so use SO_BINDTODEVICE. `deviceBinding.ts:4-8`
69. SO_BINDTODEVICE is unprivileged only from Linux 5.7: probe on `lo`, fall back with a warning. `deviceBinding.ts:41-64`
70. Don't device-bind loopback destinations. `deviceBinding.ts:69-72`
71. An fd-wrapped socket needs `manualStart`. `deviceBinding.ts:83-85`
72. An interface vanishing between list and connect becomes a socket error, not a throw. `deviceBinding.ts:77-82`
73. Poll interfaces every 1 s (no OS event); diff with JSON. `interfaces.ts:185-223`
74. Re-run costly label lookups (networksetup, PowerShell) only when the device set changes. `interfaces.ts:118-137`
75. Localised macOS `networksetup` output: parse by position. `interfaces.ts:29-38`
76. Windows PowerShell: BOM, single object vs array, restricted policy. `interfaces.ts:71-96`
77. Linux interface kind from predictable names (wl*, en*u*, enx*, usb*). `interfaces.ts:46-56`
78. An address change wakes streams; a lost address drops their sockets (ERR_NETWORK_CHANGED). `downloadManager.ts:1078-1108`
79. A network appearing mid-download starts off. `downloadManager.ts:1491-1492,1520-1525`
80. A vanished network goes `offline` and is reused when it returns. `downloadManager.ts:1534-1542`
81. Sleep/wake: reset clocks, reconnect everything, refresh interfaces. `downloadManager.ts:1110-1119`, `handlers.ts:83-86`
82. Prevent system sleep while downloading. `downloadManager.ts:1121-1137`
83. Switching off the last network pauses; switching one on resumes. `downloadManager.ts:1041-1076`
84. A non-splittable download runs on exactly one network; enabling another switches over. `downloadManager.ts:829-832,1057-1060`
85. Unknown size: one open-ended block, one stream. `plan.ts:75-76,114`, `httpTransfer.ts:160-164`
86. Every network gets a first block before any gets a second (interleave). `httpTransfer.ts:318-320`, `plan.ts:48-65`
87. A block that delivered nothing on network X goes to another network while one is free. `scheduler.ts:64-79`
88. Hedge only after the queue is empty and the disk is keeping up. `scheduler.ts:89-90`
89. Hedges write identical bytes to the same offsets after the version check; the first to finish wins. `httpTransfer.ts:40-43,787`
90. A primary attempt retracts its block to what is truly secured (the max of its own start and the others' progress). `httpTransfer.ts:634-642`, `blockProgress.ts:49-58`
91. Progress events can lag the final write: square the block up at completion. `httpTransfer.ts:774-783`
92. Per-network byte attribution of a block split across networks. `types.ts:121-131`, `blockProgress.ts:3-35`
93. A server refusing some connections (403/429/503 or silence) while serving others: lower that network's ceiling. `concurrency.ts:9-17,150-154`
94. Disk falling behind: cut streams to half, recover by doubling. `concurrency.ts:21-32,186-201`
95. A disk that stopped entirely (no landing for 500 ms) reads as unknown and is not "fixed" by fewer streams. `concurrency.ts:81-86,103-110`
96. A data limit is crossed mid-chunk: notify asynchronously. `limits.ts:121-125`
97. Calendar periods in local time, weeks starting Monday, with migration from the monthly-only format. `dataLimits.ts:9-23`, `limits.ts:57-64`
98. A token bucket holds at most 1 s, so no burst after idle. `limits.ts:12-27`
99. Atomic JSON: tmp plus rename, per-pid tmp name, per-path write queue, Windows lock retries. `jsonFile.ts:25-54`
100. A corrupt or missing JSON file reads as undefined; other read errors throw so nothing is overwritten. `jsonFile.ts:6-22`
101. fsync the staging file before writing the manifest. `downloadManager.ts:1693-1695`, `downloadFile.ts:36-44`
102. A restored manifest is untrusted input: validate the plan, and drop progress beyond the file size. `savedProgress.ts:28-63`, `downloadManager.ts:700-711`
103. Two manifests claiming one partial: keep the newest. `downloadManager.ts:512-525`
104. A partial on an unmounted drive: don't discard; report "Reconnect the destination drive". `downloadManager.ts:492-497,961-966`
105. Downloads restored after relaunch never auto-start. `downloadManager.ts:660-665`
106. Pause and cancel must wait for the previous run to wind down before a new one starts. `downloadManager.ts:955-957`
107. Cancel deletes files before removing the item; a failed cleanup is not resumable. `downloadManager.ts:1223-1256`
108. Quit: pause and persist everything, with a 3 s hard deadline. `index.ts:147-161`
109. Torrent: sanitise paths exactly as the store will write them. `torrent/paths.ts:16-67`
110. Torrent: case-insensitive collisions, and a file that is also a folder prefix. `torrent/paths.ts:52-65`
111. Torrent: v2-only torrents unsupported; `.torrent` capped at 10 MB. `metadata.ts:10-13,36,59,79`
112. Magnet metadata timeout of 3 min; only the latest lookup is kept. `metadata.ts:88-123`
113. Torrent removal: validate every path, refuse symlinks and folder swaps. `ownedFiles.ts:4-47`
114. Torrent edge pieces write into unchosen files: delete those at publish. `files.ts:36-40`, `torrentDestination.ts:54-67`
115. A completed torrent file can't be deselected. `downloadManager.ts:1176-1181`
116. Incoming peers attributed by local address; unknown ones dropped. `torrentTransfer.ts:302-321`
117. Hostname peers (`x.pe`) have no family, so they are rejected. `torrentTransfer.ts:248-266`
118. uTP, web seeds, WebRTC, UPnP, NAT-PMP and LSD off, because they bypass the binding. `engine.ts:58-79`
119. Trust a resume bitfield, but reset pieces webtorrent finds missing. `torrentTransfer.ts:218-219,413-433`
120. Throttle torrent bytes at `socket.push` without forcing flowing mode. `torrentTransfer.ts:285-300`

---

### C) Algorithms and constants

#### C.1 Block and chunk sizing (`main/download/plan.ts`)

| Constant | Value | Line |
|---|---|---|
| `DEFAULT_MAX_BLOCK_BYTES` | 8 MiB | :13 |
| `MIN_BLOCK_BYTES` | 1 MiB | :17 |
| `START_STREAMS_PER_NETWORK` | 8 (IDM/XDM) | :21 |
| `MAX_STREAMS_PER_NETWORK` | 32 (IDM) | :25 |
| `BLOCKS_PER_STREAM` | 2 | :30 |

- `planDownload` (`:70-87`): if not splittable or size ≤ 0, the result is **one block** of `totalBytes`. Otherwise `targetBlocks = networkCount × 32 × 2` and `blockSize = clamp(ceil(total/targetBlocks), min(1 MiB, maxBlock), maxBlock)`.
  - Examples with 2 networks (128 target blocks): a 1 GiB file gives 8 MiB × 128 blocks; 100 MiB gives the 1 MiB floor (100 blocks); 10 GiB is capped at 8 MiB (1280 blocks).
  - `networkCount` is the networks **compatible at start** (`downloadManager.ts:816-821`). Block size is **fixed for the download's life**; networks joining later don't change it.
  - Rationale: "a racing second attempt re-fetches a block rather than splitting it… so the last block's size is how long the slowest connection can hold the download up" (`:10-12`).
- `planBlocks` (`:104-119`): `[i·bs, min((i+1)·bs, total)−1]`. An unknown size gives one block `[0, null]`.
- `planPieces` (`:123-138`): torrent pieces of `pieceLength`.
- `startingStreams(waiting, joiningNetworks, requested=8)` (`:93-100`): `max(1, min(clamp(floor(requested), 1, 32), ceil(waiting/joining)))`. Never 0, "so a network that joins can still race a slow block".
- `interleave` (`:50-65`): round-robin by group, used to start streams across networks fairly.

There is **no dynamic range splitting or true work-stealing**: blocks are fixed, and idle streams pull the next pending block. The endgame uses hedging (duplicate attempts), not splitting.

#### C.2 Scheduler: the pull queue (`main/download/scheduler.ts`)

`pickWork` (`:147-157`) returns primary work first, then a hedge. It is called synchronously so that "nothing between choosing the work and registering it can yield" (`httpTransfer.ts:1076`).

- **Primary** (`nextWaitingBlock` `:69-79`): the lowest-index `pending` block, except one whose `avoid[block] === myNetwork` while any stream on another network is `pending` (idle). If no other network is free, it takes it anyway: "never stranded". The `avoid` entry is set when an attempt delivered 0 bytes (`httpTransfer.ts:956`) and cleared when a primary is assigned (`:598`).
- Idle streams poll every `IDLE_POLL_MS = 250` while any block is pending or downloading (`httpTransfer.ts:184,1094-1104`). Otherwise the stream is `completed`.
- **Hedging** (`nextHedgeTarget` `:83-145`) uses policy `{hedgeAfterMs: 2000, maxHedgesPerBlock: 2, startupMs: 1000}` (`httpTransfer.ts:154-158`). Conditions:
  - No pending blocks, and the disk is not `behind`.
  - The block is `downloading`, has a known end, and has at least one primary attempt in flight. The requester isn't already on it.
  - Hedges used < 2, and the requester's network isn't avoided for that block.
  - **Every** attempt on it has run ≥ 2000 ms, and none is `writeWaiting`.
  - `eta = remaining / (fastest holder's stream speed)`; a silent holder has ETA ∞.
  - Worth it if the requester's own last-block speed is known and `eta ≥ 2 × (1000 + remaining/mySpeed·1000)` ("finish in under half that time"). If unknown, `eta ≥ 2000 ms`.
  - Prefer another network: if the requester's network already holds the block and a free stream exists on a different, non-avoided network, skip.
  - Choose the block with the **largest** ETA.
- **Hedge mechanics** (`httpTransfer.ts:574-613,643-645,758-791`):
  - A hedge starts at the block's current frontier `bytesDownloaded`, writes to the same offsets, and doesn't change block status or owner.
  - The first `completed` attempt marks the block done and aborts rivals with reason `lost`.
  - A losing primary's later completion is "surplus" (`:765-769`).
  - Failed hedges aren't retried. Up to 2 hedges per block; a stuck hedge can itself be raced ("a block whose hedge is stuck too can be raced again", `scheduler.ts:13-14`).
- **Progress frontier**: `advanceBlock` only moves forward (max of the attempts' positions) and credits the gain to that network. `retractBlock` trims the tail, blaming the last writer first (`blockProgress.ts`). A new primary retracts to `max(startOffset, others' position)` (`httpTransfer.ts:637-642`).

#### C.3 Stream lifecycle and reconcile (`httpTransfer.ts:263-321`)

- Usable networks are those with status `on` or `unreachable`. A non-splittable download uses only the first.
- A "joining" network has just become `on`, or has no live streams. Its target is `max(live, min(startingStreams, concurrency.limit(net)))`.
- `unreachable` gets 1 stream. Off, offline, failed and limit get 0.
- Excess streams retire in this order: refused first, then those with failures, then the newest (`:1005-1025`). Retiring costs nothing because the written prefix stays.
- One `StreamConnection` per stream, with keep-alive agents, reused block to block, "pays for DNS, TCP and TLS handshakes and TCP slow start once" (`routes.ts:290-299`).
- The HTTPS agent wraps the routed socket with its own TLS, so certificates are verified against the URL host and **TLS sessions are cached** (`routes.ts:263-281`).
- Debug logging with `PLEXO_DEBUG=1` reports TTFB, reusedSocket, and bytes per attempt (`httpTransfer.ts:115-119,734-744`).

#### C.4 Stream-count policy (`main/download/concurrency.ts`)

Design: "Deliberately simple, after Gopeed and aria2: no timers and no speed comparisons" (`:1-2`). It is a pure function of a snapshot taken each tick (`httpTransfer.ts:962-987`).

| Constant | Value | Line |
|---|---|---|
| `RECOVER_MS` | 60 000 | :40 |
| `DISK_PATIENCE_MS` | 500 | :42 |
| `DISK_RECOVER_MS` | 10 000 | :46 |
| `DISK_STALLED_MS` | 500 | :49 |

- Controller: Auto is `ConcurrencyController(32, grows=true)`. A user pick (1..32) is `ConcurrencyController(picked, grows=false)`. It is null for non-splittable downloads or when a test fixes the count (`httpTransfer.ts:168-182`). The controller is kept across pause and resume (`:249-251`).
- **Growth** (`:166-171`): if no refusals, the disk isn't holding, streams > 0, and **all** streams have answered (received ≥ 1 byte ever), add `min(streams (so it doubles), ceiling−streams, spareWork)`. `spareWork` is the count of pending blocks, shared across networks in this tick. Growth path: 8 → 16 → 32.
- **Refusals** (`:150-158`): `refusedNow = refused > 0 && served > 0` (refused while other streams got data this tick). Then `ceiling = max(1, min(streams − refused, previousCeiling))`. After 60 s with no refusal it rises by 1; at max the ceiling is deleted. "A server that meant it costs one refused request a minute" (Surge-style).
- **Disk** (`DiskWatch` `:90-125`, `judgeDisk` `:186-201`):
  - Reading per tick: `writing == 0` → unknown; `held·2 ≤ writing` → keeping-up; else if a write landed within 500 ms → behind; else unknown (stalled).
  - Behind for ≥ 500 ms with max streams > 1: `cap = ceil(maxStreamsAnyNetwork / 2)`.
  - Keeping up for 10 s while at the cap: cap × 2, or null once ≥ 32. Keeping up while below the cap doesn't count.
  - The cap applies to every network (decided for the whole download, rationale `:26-30`). Each network runs `min(32, serverCeiling, diskCap)` (`limit()` `:177-183`).
  - The disk is judged only in Auto (`grows`), but `diskLimited` is shown regardless (`httpTransfer.ts:336-350`). The UI flag rises after 500 ms behind and clears after 10 s keeping up.
- The tick resets each stream's `refused` and `served` after reading them (`httpTransfer.ts:329-333`).

#### C.5 Download queue and manager loop (`downloadManager.ts`)

- `downloadsAtOnce`: default 2, min 1, max 8 (`types.ts:342`). `pump()` is FIFO by `queuedAt` and runs after every run ends, every cancel, and every settings change (`:361-370,871,1254`).
- A user resume goes to the front of the queue (`:976-979`).
- `TICK_MS = 500` (`:130`): on each tick, `updateSpeeds`, `updateTimeLeft`, a 1 s sample (speed history and peak), then `transfer.tick` (stuck refresh, then reconcile, then concurrency). The loop also wakes when any worker settles (`:1336-1392`).
- `UI_UPDATE_MS = 200` (`:128`) throttles IPC pushes. Updates are deltas: state without blocks, plus only blocks whose status, interfaceId, bytesDownloaded or provisionalBytes changed (`:1600-1636`; `types.ts:269-285`).
- Speed: `Meter` window `SPEED_WINDOW_MS = 3000` with a minimum 1 s denominator. It is read only on the tick clock (`transfer.ts:82-111`), with per-connection and per-network meters.
- ETA smoothing: per second, 0.3 when falling and 0.1 when rising (Firefox style). A new estimate ≤ half the expected value is accepted as is (`transfer.ts:180-214`).
- Peak speed: best `PEAK_SECONDS = 5` stretch, never below the average (`downloadManager.ts:135-176`).
- Speed history: `SPEED_HISTORY_SECONDS = 60` samples, one per second per network (`types.ts:345`, `downloadManager.ts:142-152`).

#### C.6 Speed limits and slow mode (`main/network/limits.ts`)

- **Token bucket** per rate. Capacity is 1 s of rate, it starts at 0 tokens, and it can go into debt; the debt equals the wait: `wait = −tokens/rate·1000 ms` (`:12-28`). Changing the rate adjusts the bucket in place, so the debt carries over (`:190-197`).
- **Global** bucket (all downloads, all networks) plus a **per-network** bucket (`NetworkPreference.speedLimit`, "Bytes a second all downloads together may take over it", `types.ts:300-301`). `take()` returns `max(globalWait, networkWait)` (`:126-129`).
- **No per-download limit.**
- **Slow mode** replaces the global limit with `slowModeSpeed ?? DEFAULT_SLOW_MODE_SPEED = 2 MiB/s` (`types.ts:346`, `limits.ts:89-95`).
- Enforcement:
  - HTTP: `throttle(bytes)` per data chunk, then `res.pause()` for `wait` ms, release, and resume. The watchdog is cleared while held (`chunkDownloader.ts:344-348`).
  - Torrent: a `socket.push` override pauses the socket (`torrentTransfer.ts:288-300`).
  - "Stopping reading is what slows the sender: TCP's window fills" (`limits.ts:7-10`).
  - Only received bytes are limited and counted. Torrent uploads are not.

#### C.7 Data-usage accounting

- Periods are `day | week | month`, default month (`types.ts:290,302-305`; `limits.ts:105`).
- Keys: `YYYY-MM` for month; `YYYY-MM-DD` for day and for week (the Monday's date). Local time zone, weeks start Monday (`dataLimits.ts:9-15`). `nextDataReset` gives local midnight of the next day, next Monday, or the first of next month (`:17-23`). There is **no configurable reset day**.
- Every byte is added to **all three** period counters at once (`limits.ts:116-118`). The limit is compared against the chosen period's counter (`:132-140`).
- `rollPeriods` resets a counter whose key changed (`:176-187`).
- Crossing the limit triggers `setImmediate(onLimitReached)`, then `limitsChanged()` reconciles all downloads, sets the network to `limit`, and its streams retire (`downloadManager.ts:313,342-349`).
- `resetUsage(id)` zeroes only that network's current chosen period and saves at once, rolling back on failure (`limits.ts:151-165`).
- Saved to `userData/network-usage.json` as `{version: 2, periods: {day|week|month: {key, bytes: {networkId: n}}}}`. Saved at most every `USAGE_SAVE_MS = 10_000` after activity (`:40,119`) and on quit (`suspendAll` → `limits.save()`). On load, only entries whose key matches the current period are kept. The legacy `{month, bytes}` format is migrated to month (`:54-82`).

#### C.8 Progress persistence

- **Layout**: `userData/downloads/<uuid>/manifest.json`, plus `metadata.torrent` for torrents (`downloadManager.ts:385-400`). Staging is `<dest>.plexo` beside the destination (HTTP) or in place (torrent).
- **Manifest shape** (`version: 7`, `:112-126,1683-1692`):

  ```
  { version: 7, savedAt, state: <DownloadState minus blocks/streams|pieces/peers>,
    progress: { [networkId]: number[] /* bytes per block index */ }, complete: boolean,
    partialPath, publicationPath?, publicationIdentity?: {dev, ino}, requestPayload }
  ```

  Block ranges are not stored; they are re-derived from `totalBytes` and `blockSizeBytes`. `restoreBlocks` validates `ceil(total/bs) === totalBlocks`, and that each column has the right length and only safe non-negative integers. Any mismatch restarts the blocks from fresh (`savedProgress.ts:4-63`). Torrent pieces store only verified bytes; provisional bytes are never persisted (`:4-5`).
- **Frequency**:
  - A routine checkpoint `CHECKPOINT_INTERVAL_MS = 15_000` after any push (`:131-133,1638-1648`). "Syncing a growing file can briefly monopolize a slow destination drive." At most one running and one queued.
  - Immediate `persistNow` on start, pause, relink, restore, quit, cancel failure, before and after publish, and at completion.
- **Atomic write**: the persistence chain is serialized per download. If the status is downloading or paused, it first **fsyncs the staging file** (`file.sync()`), then writes `manifest.json.tmp` and `rename`s it over `manifest.json` (`:1661-1704`). Routine checkpoint errors are swallowed; publication intent (`required=true`) propagates.
- **`jsonFile.ts`** (settings, history, usage):
  - `readJson` waits for queued writes first; a SyntaxError or ENOENT gives undefined, other errors throw (`:6-22`).
  - `updateJson` does a read-modify-write queued per path in `writeChains`, writes `${path}.${process.pid}.tmp` (pretty, 2 spaces), then `renameWithRetry`: up to 5 attempts, waiting 50·(n+1) ms on EPERM, EACCES or EBUSY (`:25-54`).
- **History**: `userData/history.json`, newest first, `MAX_ENTRIES = 500`. Entries are validated on read and marked `missing` by `stat` (`history.ts:11-85`). Completed downloads move to history and their manifest folder is deleted (`downloadManager.ts:548-579`).
- **Settings**: `userData/app-settings.json`, every field sanitised (`settings.ts:13-108`). Legacy `network-preferences.json` is migrated (`:116-130`).

#### C.9 Network binding per OS

- **macOS and Windows**: `net.connect({host: remoteIP, port, localAddress, family})`. These OSes "already route by source address" (`deviceBinding.ts:8,70-72`). No route-table changes, no metrics.
- **Linux**: through koffi, `libc.so.6` `socket(AF_INET|AF_INET6, SOCK_STREAM|SOCK_CLOEXEC, 0)` and `setsockopt(fd, SOL_SOCKET=1, SO_BINDTODEVICE=25, device, len+1)`. The fd is wrapped as `new net.Socket({fd, manualStart:true}).connect({host, port, localAddress, family})` (`deviceBinding.ts:10-15,28-39,83-90`).
  - Support is probed once by binding `lo` (EPERM before kernel 5.7). On failure it falls back to localAddress only, with a console warning (`:41-64`). The result is exposed to the UI as `deviceBindingSupported` (`handlers.ts:89-90`).
- **"routes.ts"** is not OS routing. A `NetworkRoute` is the tuple `{device, localAddress, remoteAddress, family}`, built from DNS results × interface addresses of the same family, in DNS order (`routes.ts:15-20,43-57`). The routes are raced with Happy Eyeballs. `lastGoodRoute` is keyed by `ifaceId host:port`.
- The probe and `.torrent` URL fetch (`fetch`) are **not bound** and use the default route. Magnet metadata uses an unbound shared client (`metadata.ts:85-87`).

#### C.10 Interface detection

- `os.networkInterfaces()` is polled every `POLL_MS = 1000`, serialized. A change is detected with `JSON.stringify` and fans out to `manager.networksChanged()` and the renderer (`interfaces.ts:185-223`; `handlers.ts:76-80`).
- Filter: drop `internal`, keep IPv4/IPv6, drop `169.254.*` and `fe80:*`, and drop devices with no usable addresses (`:142-159`). The id is the OS device name (`types.ts:15`).
- Labels and kinds: macOS `networksetup -listallhardwareports`; Windows PowerShell `Get-NetAdapter` (Name, InterfaceDescription, NdisPhysicalMedium: 1 or 9 means Wi-Fi, 14 means Ethernet). Both have a 5 s timeout (`DISCOVERY_TIMEOUT_MS`, `:8`) and are cached until the device set changes (`:118-137`). Linux uses name patterns (`:46-56`). Regex classification for Wi-Fi, USB/RNDIS/iPhone, bridge and Ethernet is at `:58-66`.
- An IPv4 CIDR subnet is computed for the UI's same-subnet warning (`:98-116`; `types.ts:8-9`).

#### C.11 Torrent engine configuration

- webtorrent 3.0.21 is loaded lazily through `import()` (ESM in CJS main, `engine.ts:6-13`).
- **One client per download run**. Pause destroys it with files kept; resume creates a new one seeded with a bitfield of completed pieces (`torrentTransfer.ts:36-44,214-231`). A separate app-lifetime probe client handles magnets "so its DHT stays warm" (`metadata.ts:85-87`).
- Options (`engine.ts:66-79`): `utp:false`, `webSeeds:false`, `tracker:{wrtc:false}`, `natUpnp:false`, `natPmp:false`, `lsd:false`, `dht: true` (off in tests), plus `connect` hook and `maxConns`. Trackers and DHT are on.
- Per network: `PEERS_PER_NETWORK = 30` gives `maxConns = 30 × usableNetworks`, updated on reconcile (`torrentTransfer.ts:17-18,80,153`).
- The listening port isn't configured, so it is webtorrent's default. Incoming TCP connections are tagged by `socket.localAddress` (`:174,302-314`).
- A new outgoing peer goes to the usable network with the **fewest** peers (connected + dialling), first on a tie (`peers.ts:11-21`). An unreachable network gets one dial at a time.
- Unreachable rule: `UNREACHABLE_AFTER_ATTEMPTS = 5` dials with no answer **and** `UNREACHABLE_AFTER_MS = 60_000` since the last answer **and** no peers on it **and** another network has peers (`torrentTransfer.ts:19-22,87-101`). A wire arriving resets it to `on` (`:326-329`).
- Credit: bytes per piece per network are held as unverified. On `verified`, the piece length is split proportionally with whole bytes, the remainder going to the largest share; with no data it goes to the fallback network (`peers.ts:23-47`, `torrentTransfer.ts:383-411`). `provisionalBytes` is shown live.
- Store: an fs-chunk-store subclass maps the torrent's top folder to the claimed name ("Name (1)") and records written files, so `sync()` fsyncs only those (`engine.ts:30-56`, `torrentDestination.ts:76-103`).
- File selection: deselect all pieces, then `file.select()` for chosen files (`torrentTransfer.ts:193-201`). Unchosen-only pieces are `skipped`.
- Metadata: a recent cache of `RECENT_LIMIT = 8` .torrent files by infoHash (`metadata.ts:18-29`), `MAX_TORRENT_FILE_BYTES = 10 MiB`, magnet timeout 3 min.

#### C.12 Things absent (gaps worth knowing)

- No `If-Range`. Validators are checked after the response.
- No checksum or hash verification for HTTP.
- No Accept-Encoding or Content-Encoding handling, no cookies, auth headers, Referer or proxy support. User-Agent is `Plexo/1.0`.
- Probe and DNS are not per-interface.
- No mid-download free-space check, and no preallocation or fallocate.
- No tmpfs or exFAT special-casing beyond the sparse attempt.
- No per-download speed limit, and no configurable usage reset day.
- Blocks are never split dynamically; a block can only be raced by hedges.
- Write errors such as ENOSPC are retried as strikes (up to 5 per stream, then the network fails), not failed immediately.


---

# Part 6: Test catalogue and CI

Nothing in the repo was modified. There are 35 spec files and about 230 `test(` call sites; loops expand that to more cases. All tests run under Playwright (`@playwright/test` ^1.63) and use `fast-check` ^4.10 for property-based tests. There is no Jest/Vitest. The "unit" tests are Playwright specs that import `src/main/...` modules directly.

**Tagging works by title.** A tag is written into a `test.describe(...)` or `test(...)` title. `--grep @smoke` matches the full title path, so a tag on a describe covers every test inside it. Three tags exist: `@smoke`, `@disk` and `@chaos`. Nothing is marked `test.fail`/`fixme`/`only` right now. `CONTRIBUTING.md` says known bugs should be written as `test.fail(...)`.

---

### 1. Per-spec catalogue

Notation: "fixture" means the full Electron app plus the automatic `checks` fixture (described in §2). "Pure" means the spec imports a module and runs no Electron.

#### `e2e/download.spec.ts`: happy paths, names, edge cases
**`happy paths @smoke`**
- IPv6-only origin (`host:'::1'`, `PLEXO_E2E_INTERFACES=a=::1`) completes. Asserts no network retries and that every chunk request came from `::1`.
- IPv6 origin with an IPv4-only selection: `start` rejects with `/No selected network/` and no download is created.
- Ranged download of 1 byte, and of 37.5 blocks (4 connections). Asserts `bytesDownloaded===size` and `totalBlocks===ceil(size/BLOCK)`.
- Each stream keeps its connection across blocks: 4 streams, 4 distinct TCP connection ids at the origin, more than 4 requests.
- Two networks share the work (skipped without a LAN address). Both a and b carried bytes, and per-block `bytesByInterface` equals what the server sent per source address.
- Delta updates: the first update carries all 64 blocks; later ones carry fewer than 16 changed blocks; more than 5 updates arrive.
- Server without range support: one stream, whole file.
- Unknown size (no Content-Length, no ranges): completes with `totalBytes===0`.
- Redirect during the probe (`/start` to `/files/test.bin`), and redirects on chunk requests (the first 3 ranged requests to `/files/moved.bin`). Both complete.

**`file names @smoke`**
- Content-Disposition with `../../evil.sh` becomes `.._.._evil.sh` inside the destination folder.
- Control character `%0A` becomes `_`.
- ISO-8859-1 `filename*` decodes to `£ rates.txt`.
- The same name twice gives `test (1).bin`, and the first file is unchanged (sha check).

**`edge cases`** (describe is untagged; individual tests carry tags)
- A 0-byte file completes within 5 s `@smoke`.
- A server that never answers the probe (`stallHeaders`): `probeUrl` rejects with `/did not respond/` in under 10 s `@smoke`.
- An expired link (403 mid-download) gives an error that keeps its bytes. `relinkDownload` to a different file rejects with `/different file/`. Relinking to a fresh identical URL completes and fetches only what was missing, with slack of 4 blocks. *(untagged, so it runs nightly only)*
- Queue limit of 2: a third download is `queued`. Pausing one lets the queued one start. Resuming the paused one puts it back in `queued`, first in line. Everything completes `@smoke`.

#### `e2e/integrity.spec.ts`: misbehaving server or network (everything here is `@smoke`)
- **Transient faults are retried to a correct file.** The fault hits the first 3 ranged requests with start>0. Cases: `wrongStart`, `overlong`, `noContentRange`, `ignoreRange` (200 for a mid-file range), 500, `{endAfter:1000}`, `{cutAfter:5000}`, `stallBody`, `stallHeaders`. Asserts the fault was injected 3 or more times and the retry total is above 0.
- **A connection stuck at a crawl** (`SLOW_WARMUP_MS=500`, `SLOW_FOR_MS=1500`):
  - A crawling 2 KB/s block is reconnected. Asserts the slow request was cut short, a later request resumed inside that block's range, and there were 0 retries.
  - Equally slow connections (256 KB/s everywhere) are left alone: no request was cut off.
  - A block is refreshed at most twice: exactly 3 requests for the always-crawling third block.
- **No range support:** a dropped connection restarts from 0. Exactly 2 full transfers.
- **One network dies for good** (b returns 503 after 2 blocks): network a finishes. Asserts 503s really happened over b.
- **Busy server** (`SERVER_BUSY_MS=60000`): 14+ `503 Retry-After: 1` answers, more than the retry budget, are waited out. No request lands within roughly 1 s of a 503.
- **Permanent faults end in a clean error:** every chunk returns 500, or every chunk redirects to itself. Status is `error` with a message.
- **File changes mid-download** (hold at 3 blocks+100, then mutate): same size with a new ETag, or a different size with no validators. Ends in `error /changed during the download/`, never a spliced file.
- **Servers that label the same file differently:**
  - A load balancer with two ETags over identical bytes completes.
  - `W/"v1"`, `"v1-gzip"` and `"v1"` are treated as the same file and complete.
  - A half-rolled-out new version (alternate requests serve new bytes and a new ETag) ends in `error`, never a mix.

#### `e2e/lifecycle.spec.ts`: pause, resume, cancel, remove
- `pause and resume @smoke`:
  - Pause partway through a block (5 blocks+1234), resume, complete. Speed is 0 while paused.
  - The same, paused exactly on a block boundary (8 blocks).
  - Rapid pause/resume 20 times while the origin is held, then complete.
  - Resuming while a paused stream is still winding down. A relabelled ETag triggers a sample request that hangs (`stallHeaders`). Resume must wait for the old run rather than run beside it, and the download completes.
- `pause during a retry backoff` (`RETRY_BASE_MS=5000`): pausing does not wait out the backoff. Paused in under 2 s `@smoke`.
- `resume safety checks @smoke`:
  - New ETag plus new bytes while paused gives `error /changed/`.
  - Same bytes under a new ETag (server migrated) resumes. Completed blocks are not refetched; only small samples of 16 KB or less verify them.
  - No validators at all resumes.
  - Resume against a server without range support completes.
- `cancel and remove @smoke`:
  - Cancel while paused. The `.plexo` staging file exists while paused and is removed after cancel.
  - Remove after completion keeps the file (sha matches) and removes `userData/downloads/<id>`.
  - Remove while downloading deletes the destination and the manifest folder.

#### `e2e/recovery.spec.ts`: quit, crash, persisted state (all `@smoke`)
- Normal quit mid-download, relaunch: same id, `paused`, bytes above 0. Resume completes.
- SIGKILL with the origin held at byte 100, and at 3 blocks+777 (4 connections). Restored as `paused` with the same id, and resume completes.
- A corrupt `manifest.json` doesn't stop startup. The download is dropped, its folder cleared, and a new download works.
- Two saved manifests for one partial file (a clone dated an hour earlier): only the newest is restored, and the older folder is removed.
- Staging file deleted while the app was closed: `error /partial download file is missing/`.
- Legacy and future manifests are cleared:
  - v2 manifest with a `parts/` folder and an empty placeholder file: removed.
  - v5 manifest with a sibling `.plexo` file: removed.
  - A v2 manifest pointing at a real non-empty user file: the manifest is removed but the user file is kept.
  - The current-version download is still restored.
- A staging file truncated while closed (power cut after the manifest said a block was done): the block is refetched and the sha matches.

#### `e2e/disk.spec.ts`
- `disk space @disk`:
  - A 16 MB file on a 10 MB volume: `start` rejects with `/Not enough disk space/`. Nothing is written to dest or to `userData/downloads`.
  - A 24 MB file on a 40 MB volume, with userData and dest both on it: the file fits once but not twice and completes. Only that one file is in dest, which proves there is no second staging copy.
- `destination folder problems @smoke`:
  - The destination folder is deleted mid-download (held at 10 blocks): status is `error`, and cleanup doesn't hang waiting for a drain.
  - A read-only destination (chmod 555; skipped on Windows): start rejects with `/EACCES|permission/`, and no orphaned folder is left in userData.

#### `e2e/chaos.spec.ts`: `@chaos`
- A single property test: "any sequence of pauses, crashes and faults still ends in the exact file". Details in §3.

#### `e2e/hedge.spec.ts`: racing a slow block (untagged, `HEDGE_MS=400`)
- The holder crawls at 2 KB/s and the hedge finishes it in under 10 s. The slow block gets exactly 2 requests, and the hedge starts partway into the block, not at its start.
- The slow holder can win (40 KB/s against a 3 KB/s hedge). The losing hedge is cut off with fewer than BLOCK bytes sent.
- Paused while racing: no stream has `hedge:true` in the paused state, and resume completes.
- A second network rescues a crawling block (LAN only). At least one block has bytes credited to both a and b.

#### `e2e/liveNetworks.spec.ts`: interfaces appearing and disappearing mid-download
- The only network drops for 1.5 s `@smoke`. Status stays `downloading` with 0 streams and no lost bytes. When the network returns, the download completes.
- `two networks @smoke` (LAN only):
  - b drops out and comes back. b shows `offline` but stays `enabled`, and traffic goes over b again after it returns.
  - A network that appears mid-download is listed as `enabled:false,status:'off'` and gets no traffic until `setDownloadNetwork(b,true)`. Switching it off keeps its credited bytes. Switching the last network off pauses (zero requests). Switching it back on resumes. A resume with no network on re-enables the network switched off last.
  - Rapid off/on of the last network: off+on resumes; off, on, off stays paused; on again completes.
  - Resume with none on, where the last-switched network was unplugged while paused: a is re-enabled, b stays disabled, and the download completes.
- `a network that can't reach the server @smoke` (`SILENT_MS=500`): every b connection is cut at 0 bytes. b goes to `unreachable` with exactly 1 probing stream, and the download continues on a. Once unblocked, b is `on` with 2 streams and is credited bytes.
- `a network that changes address` (`RETRY_BASE_MS=10000`) `@smoke`: b's address is rewritten to `127.0.0.1` (a new DHCP lease). b's streams wake immediately and deliver bytes in under 5 s instead of waiting out the backoff.
- `the computer wakes from sleep` (`STALL_MS` and `SILENT_MS` at 60000) `@smoke`: one block stalls after its headers, then `powerMonitor.emit('resume')` is sent. The block is re-requested and the download completes within 5 s.

#### `e2e/network.spec.ts`: `StreamConnection` (`src/main/network/routes`), all `@smoke`, real local `http` servers, no Electron
- An AAAA-only hostname connects over `::1` and keeps the HTTP `Host: ipv6.example.test:port` header.
- Route fallback: 127.0.0.1 fails, and `::1` still gets time to answer (1.8 s delay, 3 s deadline).
- A DNS lookup that never resolves obeys the deadline: "Could not resolve the download host in time".
- A kept-alive connection the server dropped is replaced transparently. The answers are 1 and 3, over 2 connections.
- Aborting while still connecting rejects with an `AbortError` in under 1 s.
- A network that comes back with a new address: the first request fails with `/not connected/`, then succeeds once the interface exists.
- `reconnect()` drops pooled sockets, so the next request opens a new connection (connection count goes 1, then 2).

#### `e2e/streams.spec.ts`: adaptive stream count against a real server (untagged)
- A server that turns extra connections away with 503 (accepts 4): the peak tried is 8 and it settles at 4. 503s were seen.
- A server that leaves extra connections unanswered (`stallHeaders`, `STALL_MS=20000`): peak 8, settles at 4.
- A server busy for everyone for 1.5 s (`SERVER_BUSY_MS=60000`): not treated as a connection limit. Peak and final counts are above 8.
- A user-picked count of 4 is kept: the peak is exactly 4.

#### `e2e/writeWait.spec.ts` `@smoke`
- Slow disk simulation: the first `fs.WriteStream._write` callback is delayed 3 s. A range held by the disk is not refetched; there are exactly 4 chunk requests for a 4 MB file at 1 MB blocks.

#### `e2e/limits.spec.ts`: speed and data limits (untagged)
- A total speed limit of 256 KB/s holds 1 MB to more than 2.5 s.
- Slow mode at its default 2 MB/s holds 6 MB to more than 1.8 s.
- Per-network data limit, once per period (day, week, month): the network goes to `status:'limit'` while the download stays `downloading`, and `networkUsage().a` is at least the limit. Raising the limit lets it complete.
- A torrent under a 384 KB/s limit still arrives whole and takes more than 1.5 s.
- Usage survives period switches and a restart (a seeded `network-usage.json` v2 gives 1024, 2048 and 3072 per period). With `Date.now` monkeypatched in main to after the next monthly reset, all periods read 0.
- A legacy monthly usage file migrates. Switching to day shows 0; switching back to month shows 12345.
- Calendar maths (pure): local midnight, Monday-start weeks, year rollover.
- Resetting usage for a network in day, week or month: other networks and other periods are kept, and the reset survives a relaunch.

#### `e2e/openLinks.spec.ts`: links handed over by the OS (untagged)
- A `.torrent` path on the command line pre-fills the Link field and shows the Files group. Nothing starts.
- A second instance launched with a magnet passes it to the first window (single-instance lock) and exits with code 0. The first window's Link field equals the magnet.
- macOS `open-url`/`open-file` simulated through `app.emit`. A non-magnet URL and a missing `.torrent` are ignored and the field stays hidden. A magnet and a real `.torrent` fill the field.

#### `e2e/removal.spec.ts`
- `removal during completion @smoke`. The `history.json` rename is held by monkeypatching `fs.rename` in main. Cases: remove, clear history, trash (`shell.trashItem` stubbed to move files into a folder). Each waits for the pending history save, stays removed after a restart, and the file is kept unless it was trashed.
- Trashing a just-finished multi-file torrent waits for its owned-file history record. Only `a.bin` and `b.bin` go to the trash; a user-added `personal.txt` stays.
- Failed torrent cancellation `@smoke`. The destination folder is replaced with a symlink/junction, so `removeDownload` rejects `/folder is now a link/`. Status becomes `error` with `resumable:false`. The row stays visible, there is never a `cancelled` event, and the manifest says `error`. All of this survives a relaunch. After the link is fixed, removal succeeds and `personal.txt` is kept.
- Cleanup failure on an active HTTP download `@smoke`. `fs.rm` of the staging file is patched to hang and then throw EACCES. Both concurrent removes reject `/Permission denied/`. A resume during the cancellation stays `paused`. The result is `error` with `resumable:false`, the staging file is still there, and there is no `cancelled` event. A later remove cleans up.

#### `e2e/selection.spec.ts`: multi-select toolbar UI (untagged)
- Mixed selection (finished, expired/error, paused): Select all gives "3 selected" and the layout doesn't shift. "Resume (1)" is offered, there is no Retry, and "Fix link" and "Cancel downloads… (2)" appear. "Remove from list (1)" keeps the finished file. The cancel confirmation can be backed out and then confirmed, which leaves "No downloads yet".
- A finished file that went missing can be removed from the list. "Move files…" is not offered.
- Trashing a finished torrent through the UI ("Move files to Trash/Recycle Bin") keeps unrelated files in the folder.
- Cancelling an unfinished torrent keeps unrelated files in its folder.

#### `e2e/ui.spec.ts`
- `a torrent through the UI` (untagged, one network): choose files (uncheck `b.bin`), start, open peers. Checks the peer row text and cells ("Peer #n", "Receiving at", "WebTorrent", %, 6 cells, no progressbar or badges), the Networks table columns, and the "N pieces ·" text. At the end it checks "2 of 3 files", "written in N pieces · uploaded".
- `UI journeys @smoke`:
  - At the minimum window size of 720×620, Pause/Resume, the Network column header, the checkbox and "Show in Finder" are fully in the viewport. No horizontal scroll. An empty progressbar has `aria-valuenow=0`.
  - Paste a link, choose Streams 8 (the manifest `requestPayload.streamsPerNetwork===8`), pause, resume, finish, reveal. `shell.showItemInFolder` is stubbed and records the path.
  - Cancel through the confirmation dialog, then download the same link again.
  - A 404 link shows "couldn't be found" and Download stays disabled.
  - A completed download is still listed after a restart and opens to its finished view.
- `settings @smoke`:
  - With `PLEXO_FORCE_UPDATE_VERSION=9.9.9`: dismiss the update dialog, change the theme, the destination and a network's name and colour. Everything is still set after a reload and after a relaunch, with no waiting for saves. The update link is visible and the dialog stays dismissed.
  - A broken `app-settings.json` (truncated JSON, or a destination that doesn't exist) falls back to fresh-install defaults.
- `no networks` (`PLEXO_E2E_INTERFACES=''`) `@smoke`: "No networks connected" shows, Scan again is safe, and when an interface appears "No downloads yet" shows.

#### Torrent specs (local swarm)
- **`e2e/torrent.spec.ts`** (untagged):
  - Single-file torrent: fileName is right and the piece count is right.
  - Multi-file torrent published as its folder ("Album", tree sha).
  - Pause, quit, relaunch, resume with 3 seeders at 300 KB/s. The swarm's upload delta is at most missing+6 pieces, so no refetching.
  - Choosing files: only the chosen ones are written. `skippedBytes>0`, `bytesDownloaded===total−skipped`, and some pieces are `skipped`.
  - The file choice survives quit, relaunch and resume.
  - Changing the choice mid-run (`chooseTorrentFiles`): `files` equals `{chosen:2,total:4,selected:[0,2]}`.
  - Two networks (LAN only): both deliver bytes and peers are seen on a and b. Switching b off moves all peers off b.
  - Uploading over a USB-kind network (`b=<lan>==usb`). A leech sees a wire from the LAN address and receives bytes. `bytesUploaded` per network is at least what was received, totals match, and upload speed is 0 at the end.
- **`e2e/torrentProbe.spec.ts`** (untagged; webtorrent seeders with no tracker; magnet peers come from `x.pe`):
  - HTTP link to a `.torrent`, a local `.torrent` folder, and a magnet. Each checks infoHash, name, size and files.
  - Case-colliding files are refused: `/saved as the same file/`.
  - A v2-only (BEP 52) torrent is refused.
  - A magnet with no peers (`MAGNET_MS=1500`) fails with `/No peers responded/`.
- **`e2e/torrentNative.spec.ts` `@smoke`**: in a fresh Node process with `process.dlopen` throwing, webtorrent imports, `WEBRTC_SUPPORT===false`, and a client is created and destroyed.
- **`e2e/torrentFiles.spec.ts`** (pure, fast-check): `wantedPieces` is true exactly when a chosen non-empty file has a byte in the piece. `chosenFiles` handles all/none/out-of-range/fractional input.
- **`e2e/torrentPaths.spec.ts`** (pure, fast-check, 500 runs × darwin/linux/win32):
  - Paths stay inside the folder. No `..`, `.` or empty segments, each segment is at most 255 bytes, and there are no case-insensitive collisions. Otherwise the result is `UnsafeTorrentError`.
  - Explicit refusals: traversal, A/a collisions, file-vs-dir collisions, `CON.txt` and `a:b` on win32, and an empty list.
  - `STRIPPED_FROM_NAMES` equals the regex from `filename-reserved-regex` (the regex fs-chunk-store uses).
- **`e2e/torrentPeers.spec.ts`** (pure): `pickNetwork` chooses the network with the fewest peers and spreads evenly. `creditPiece` gives integer shares that sum to the piece length (property). Bitfield bit order is checked.

#### Pure algorithm specs (no tags, so they run nightly only)
- **`e2e/plan.spec.ts`** (fast-check): blocks tile the file exactly and stay within MIN/DEFAULT_MAX block size. `startingStreams` bounds hold. A small file is one block. A 512 MiB file has enough blocks for every network to reach its maximum streams. A 12 MiB file gets 8 or more blocks. No ranges or unknown size gives 1 block. `interleave` keeps per-group order (property).
- **`e2e/scheduler.spec.ts`** (`pickWork`): about 25 cases.
  - Primary selection: first pending block; avoid-list handling so a block can't be stranded.
  - No deadlock (property).
  - Hedge rules: raced on finish-time rather than speed; a quiet holder is raced unless `writeWaiting`; startup grace; never hedge while blocks are pending; at most 2 hedges per block; another network gets the first go; never a block of unknown length; never your own block; the longest-to-go block is raced first.
  - A "real work only" property, and `diskBehind` suppresses hedging.
- **`e2e/concurrency.spec.ts`** (`ConcurrencyController`, `DiskWatch`): about 20 cases.
  - Doubling up to the maximum; waits until every stream has answered; capped by spare work.
  - Retires refused streams and remembers the ceiling; 0-served refusals aren't treated as a limit; networks are decided independently; ceiling recovery of +1 per `RECOVER_MS`; a user-picked count is fixed.
  - Disk-behind halving needs `DISK_PATIENCE_MS` and floors at 1 stream; a brief disk stall recovers; an intermittently slow disk is stable; the disk cut doesn't disturb the server ceiling.
  - `DiskWatch` judges by the clock.
- **`e2e/blockProgress.spec.ts`**:
  - `advanceBlock`/`retractBlock` race semantics, plus a property that attribution equals the frontier.
  - `reserveDestinationPath` creates the `.plexo` placeholder and `(1)` names.
  - `DownloadFile` with out-of-order and overlapping hedge writes produces one exact file. `publish` won't overwrite a final file created meanwhile; it publishes as `(1)`.
- **`e2e/savedProgress.spec.ts`**:
  - Save/restore round-trip.
  - An unknown-size completed download stays completed.
  - Restore rejects a bad plan (block size 0, −1, NaN, 1.5, undefined, 1 byte, count mismatch) before allocating.
  - Malformed progress resets the blocks.
  - Torrent restore keeps verified bytes and never provisional ones.
- **`e2e/downloadUpdate.spec.ts`** (`applyDownloadUpdate`): delta merge without mutating the old state; stale sequence numbers ignored; a partial update before any snapshot is dropped; torrent piece merge.
- **`e2e/invariants.spec.ts`**: tests the harness's own `checkEvents`. It catches attribution mismatches, short completed units, and `completed → paused`, with context in the message. It handles a 1000-event × 256-block history and still catches a bad final event.

#### Other specs
- **`e2e/sparseFile.spec.ts`** (Windows only): `markSparse` on NTFS, confirmed with `fsutil sparse queryflag`.
- **`e2e/releaseNotes.spec.ts` `@smoke`**: runs `scripts/release-notes.mjs` against a fake `dist/` for versions `1.0.0-rc.1`, `1.0.0` and `1.0.0-rc.11+build.1`.
  - `--files` lists exactly that version's 9 builds (dmg×2, setup.exe, AppImage×2, deb×2, rpm×2). Other versions (for example rc.1 against rc.11, or 1.0.0 against 11.0.0), blockmaps, `latest.yml` and snaps are excluded.
  - The notes body is the user notes plus `## Downloads`, with each URL appearing once.
  - A missing version exits 1 with "No X builds in dist/".
- **`e2e/downloads.spec.ts`** (untagged): the website download page (`docs/downloads.js`, `docs/index.html`).
  - `describe()` labels for every artifact, with noise files mapping to null.
  - `detectEnvironment` across 9 real user agents, including Apple-silicon Chrome claiming "Intel", iPad desktop mode, Android and ChromeOS.
  - `build()`: primary/alternates/hints, a missing platform, grouping order, exactly one recommended build.
  - `markdown()` output.
  - About 12 page tests in page-host:
    - Tabs and links.
    - An undecided Mac shows both dmg links; a detected architecture links directly; mobile gets no direct download.
    - Install guidance per OS plus the dialog.
    - XSS: a hostile tag name and asset name, and a `javascript:` URL, inject nothing.
    - GitHub API returning 500 falls back to the releases link.
    - Clipboard copy succeeds; clipboard denied or unavailable falls back to manual copy.
    - Keyboard navigation of the tabs; scroll-spy in the navbar.
    - Layouts at 390 px and 768 px, with tap targets of at least 44 px.
    - The interactive demo's network toggles.

---

### 2. How the harness works

#### Launching Electron (`e2e/fixtures.ts`)
- `playwright.config.ts`:
  - `testDir ./e2e`, global setup, 60 s timeout.
  - **`retries: 0` even on CI**, on purpose: the comment says a pass that needs a retry has found a race.
  - `fullyParallel`, 2 workers on CI and 4 locally, `forbidOnly` on CI.
  - HTML reporter on CI.
- `e2e/global-setup.ts`:
  1. Finds a usable second "network" by picking a non-internal, non-169.254 IPv4 address. It proves that address works by making a request to a `0.0.0.0` server with `localAddress` bound to it, then sets `PLEXO_E2E_LAN`; the value is empty if the check fails.
  2. Runs `npx electron-vite build` unless `PLEXO_E2E_SKIP_BUILD=1`. Tests always run the built `out/`.
- `PlexoApp.launch()`:
  - Calls `_electron.launch({ args: [PROJECT_ROOT, ...args, --no-sandbox on Linux] })`, retrying on `ETXTBSY`.
  - Passes this env: `PLEXO_USER_DATA` (a temp dir), `PLEXO_E2E_HIDE_WINDOW=1`, `BLOCK_BYTES=65536` (64 KB blocks, so roughly 1 MB files split into many blocks), `RETRY_BASE_MS=20`, `STALL_MS=1500`, `SERVER_BUSY_MS=1`, `HEDGE_MS=600000` (hedging off unless a test turns it on), `STREAMS=2`, `INTERFACES=a=127.0.0.1[,b=<LAN>]`, `DHT=0`.
  - Tests override any of these with `test.use({ appEnv: {...} })`.
- The driver goes through the same `window.plexo` IPC API as the renderer: `api` is a Proxy over `page.evaluate`. `__plexoRecord` is exposed and subscribed to `onDownloadUpdated`, so every update is recorded, both raw (`updates`) and reassembled (`sessions`, one array per launch).
- `start()` imitates the IdleScreen Start button: `listInterfaces`, then `probeUrl`, then `startDownload` with the HTTP or torrent payload. `pinStreams` rewrites `process.env.PLEXO_E2E_STREAMS` in main before each start.
- `evaluateMain` runs code in the main process. Tests use it to:
  - stub dialogs, `shell.trashItem` and `showItemInFolder`;
  - monkeypatch `fs.rename`, `fs.rm` and `WriteStream._write`;
  - patch `Date.now`;
  - emit `powerMonitor` resume and `app` open-url/open-file events;
  - rewrite the interface list.
- `kill()` is SIGKILL; on Windows it uses `taskkill /T /F` so child processes die too. `quit()` closes normally, which runs before-quit and suspends downloads. `relaunch()` combines them.

#### Automatic end-of-test checks (the auto `checks` fixture)
- **`checkEvents`** runs over every recorded event:
  - `bytesDownloaded ≤ totalBytes`.
  - Units are contiguous, in order, and their count equals `totalBlocks`/`totalPieces`.
  - Per-unit `bytesByInterface` sums to its bytes. A unit's bytes fit its size, and a completed unit is full.
  - Torrent `provisionalBytes` stays in range. Peer ids are unique. Every peer is on a known network with status connected or receiving. Network uploads sum to the total.
  - HTTP while downloading: a stream holds a block exactly when its status is `downloading`. Each block has 1 primary and at most 2 hedges, and a held block is `downloading`.
  - Status transitions follow the `ALLOWED_NEXT` state machine.
- **No `MaxListenersExceededWarning`** appears in the app's stdout or stderr.
- **`checkFinalState`**, only once every tracked download is terminal:
  - An error download is removed first.
  - `completed` means the sha256 (or the `treeSha` of a folder) matches the source. Otherwise no destination file exists.
  - The `.plexo` staging file is gone.
  - The destination folder gained exactly the completed files and nothing else.
  - **`lsof` shows no open handles** under dest or `userData/downloads`. This check is skipped silently if `lsof` is missing, so effectively it never runs on Windows.
- On failure the test attaches `download-events.json`, `app-output.txt` and `server-requests.json`, the origin's request log.

#### Simulated networks (`src/main/testKnobs.ts`)
- `testInterfaces()` parses `PLEXO_E2E_INTERFACES` (`id=address[=subnet[=kind]]`, where kind is `usb`, or `wifi` inferred from the name, otherwise `ethernet`). It **replaces** the result of `listActiveInterfaces()` in `src/main/network/interfaces.ts:129`. It is read on every call, so tests change networks mid-download by rewriting `process.env` in main.
- There are **no loopback aliases or virtual NICs**. Network a is 127.0.0.1. Network b is the host's real LAN IP, used as the source address for requests to a `0.0.0.0`-bound loopback origin. The origin tells the networks apart by `req.socket.remoteAddress`. Tests that need b are skipped when no LAN address works. IPv6 tests use `::1`.
- The other knobs, all ignored when `app.isPackaged`:

  | Variable | Default | Read in |
  |---|---|---|
  | `PLEXO_USER_DATA` | none | `index.ts`; also disables notifications (`downloadManager.ts:1560`) |
  | `PLEXO_E2E_HIDE_WINDOW` | off | keeps the window hidden, turns off backgroundThrottling, hides the dock |
  | `BLOCK_BYTES` | 8 MiB | `downloadManager.ts:820` |
  | `RETRY_BASE_MS` | 1000 | `httpTransfer.ts` |
  | `STALL_MS` | 20000 | probe, chunkDownloader, httpTransfer, DNS resolve in downloadManager |
  | `CONNECT_MS` | 10000 | `httpTransfer.ts` |
  | `SERVER_BUSY_MS` | 5 min | `httpTransfer.ts` |
  | `SLOW_WARMUP_MS` / `SLOW_FOR_MS` | 5 s / 10 s | crawl detection |
  | `SILENT_MS` | 5 s | unreachable-network detection |
  | `HEDGE_MS` | 2 s | `httpTransfer.ts` |
  | `MAGNET_MS` | 3 min | `torrent/metadata.ts` |
  | `DHT` | on | `torrent/engine.ts`; tests set `0` |
  | `PLEXO_FORCE_UPDATE_VERSION` | none | `ipc/handlers.ts:231`, skips the real GitHub check |
  | `PLEXO_E2E_STREAMS` | auto | `testStreamsPerNetwork()`, read per call; turns off auto-sizing |

  `PLEXO_DEBUG=1` (`httpTransfer.ts:117`) logs per-request diagnostics and is not a test knob.

#### The origin server (`e2e/origin.ts`)
One file per server, with deterministic xorshift32 bytes (`seededBytes(size, seed)`) and a request log that records n, path, source address, TCP connection id, range, time, fault, status and bytes sent.

**Construction options:**
- `size`, `seed`.
- `host` (`0.0.0.0` or `::1`).
- `ranges:false`: ignore Range and omit Accept-Ranges.
- `contentLength:false`: chunked transfer, unknown size.
- `etag` (default `"v1"`, or null), `lastModified`, `contentDisposition`.
- `bytesPerSecond`: per-response throttle.
- `sharedBytesPerSecond`: a link-wide throttle across responses. No spec I read uses it.

**Faults per request** (`setRule(req => Fault)`):
- `ignoreRange`: 200 with the whole file.
- `wrongStart`: Content-Range off by one.
- `overlong`: one extra body byte.
- `noContentRange`: 206 with no Content-Range header.
- `stallBody`: headers, then silence.
- `stallHeaders`: never answers.
- `{status, headers}`: any status. Specs use 403, 404, 500, 503 and 503+Retry-After. There is no 429 test, though the knob would support one.
- `{cutAfter, afterMs?}`: socket destroyed after N bytes.
- `{endAfter}`: a short body that ends cleanly.
- `{crawl: B/s}`: 256-byte trickle.
- `{redirect}`: 302.

**Automatic behaviour:** 416 for a start at or past EOF; HEAD supported; the 1-byte probe (`bytes=0-0`) is never held.

**Precise control:**
- `hold(offset)` pauses every response just before a byte offset and resolves when it is reached; `release()` continues. This is how tests pause, kill or mutate at exact bytes instead of using timers.
- `setContent(buf, etag)` publishes a new version.
- `setVersionRule(req => {content, etag})` imitates load-balanced or half-rolled-out backends.
- Each response snapshots its content, so swapping the file mid-response can't splice two versions.
- `chunkRequests()` returns the log without probes.

#### Torrent swarms (`e2e/torrentSwarm.ts`)
- An in-process `bittorrent-tracker` HTTP server on 127.0.0.1 with UDP and WS off.
- webtorrent clients with `QUIET` options: no DHT, LSD, UPnP, NAT-PMP or uTP. The app itself runs with `PLEXO_E2E_DHT=0`.
- `seed(files, {folder, pieceLength, uploadLimit})` uses a private temp folder per seeder, because a shared `/tmp/webtorrent` once corrupted pieces across parallel tests. Calling it repeatedly adds more seeders of the same torrent. Slowness comes from `uploadLimit`.
- `leech()` adds a downloading peer, used to test Plexo's uploads.
- `uploaded()` sums what the swarm sent, used to prove no refetch after resume.
- `torrentFileOnDisk()` writes the `.torrent` file.
- `torrentProbe.spec.ts` uses a tracker-less seeder and magnet `x.pe=127.0.0.1:port` instead.

#### Disk simulation
- **Disk full:** a real small filesystem. macOS uses `hdiutil attach -nomount ram://` plus `diskutil erasevolume HFS+`. Linux uses `sudo -n mount -t tmpfs -o size=Nm` plus chown. Without these tools or passwordless sudo the test skips, which includes Windows always. So ENOSPC comes from the OS, not a mock.
- **Slow disk:** `WriteStream.prototype._write` is monkeypatched in main to delay the first callback by 3 s (`writeWait.spec.ts`). Disk back-pressure policy is otherwise covered by the pure `concurrency.spec.ts` (`DiskWatch`, `diskBehind`) and `scheduler.spec.ts` (`writeWaiting`).
- **Permission and missing-folder errors:** `chmod 0555`, deleting dest mid-download, symlink/junction swaps, and `fs.rm`/`fs.rename` patched to hang or throw EACCES.
- **Power-cut states:** SIGKILL, manifest truncation or corruption, and a truncated staging file.

#### Page-host (`e2e/page-host/main.cjs`, `preload.cjs`)
- This is for the **marketing download page** (`docs/index.html`), not for opening links in the app.
- A bare Electron app reads `PAGE_SCENARIO` (JSON). It intercepts `https` through `protocol.handle`: `api.github.com` returns the scenario's releases or `apiStatus`, and everything else returns 404.
- It loads `docs/index.html` in a hidden window with `contextIsolation:false` and sets the user agent.
- The preload overrides `navigator.platform`, `maxTouchPoints` and `userAgentData.getHighEntropyValues({architecture})`, removing them where the imitated browser (Safari, Firefox) has none.
- Opening links in the app itself (`openLinks.spec.ts`) uses the real app: command-line args, a second spawned instance for the single-instance handoff, and `app.emit('open-url'|'open-file')`.

---

### 3. Chaos testing (`e2e/chaos.spec.ts`)

It is model-based testing with `fc.commands` and `fc.asyncModelRun`.

- **Inputs generated per run:**
  - A file seed (1 to 2³¹−1).
  - Connections: a fixed 1 to 4 streams per network, or `'auto'`. For auto, the app is created with `PLEXO_E2E_STREAMS=''` so auto-sizing stays on across relaunches.
  - A command sequence of up to 12 commands.
- **Real system per run:** fresh temp dirs, an `Origin` of 48 blocks (3 MB) throttled to 512 KB/s, a new `PlexoApp`, and networks `['a','b']` when a LAN address exists. The origin rule takes faults one by one from `real.faults` for each non-probe ranged request.
- **Model:** `phase: running | paused | done`.
- **Commands:**
  - `Wait(20–700 ms)`, then `sync`. `sync` marks the phase done if the download completed, and **throws if the status is error or cancelled**, so the property requires transient faults never to end a download.
  - `Pause`, only valid while running. Waits for paused or completed.
  - `Resume`, only valid while paused. Waits for downloading or completed.
  - `Restart('crash'|'quit')`: SIGKILL or a normal quit, then relaunch. Asserts the same id and a status of paused or completed.
  - `Flaky(1–3 faults)`: **replaces** the fault queue rather than adding to it, so faults never pile up past the retry budget. Faults are drawn from `{cutAfter: 0..BLOCK-1}`, `{status:500}`, `{status:503}`, `wrongStart`, `overlong` and `{endAfter: 0..BLOCK-1}`.
- **After the sequence:** faults are cleared, the download is resumed if paused, and it must complete within 60 s. Then `checkEvents(app.sessions)` runs across all launches, plus `checkFinalState` (sha, no strays, staging file removed, no open handles).
- **Runs and reproduction:**
  - `PLEXO_CHAOS_RUNS` defaults to 5 locally. Nightly uses 50, and `workflow_dispatch` takes a `chaos_runs` input.
  - Test timeout is `RUNS*90s+60s`, with `interruptAfterTimeLimit` set to match.
  - When a run fails, fast-check shrinks the sequence to the smallest one that still fails and prints the seed. `PLEXO_CHAOS_SEED=<seed>` replays it.
- **Not in the chaos alphabet:**
  - Stalls (`stallBody`/`stallHeaders`) and crawls.
  - 429/403/Retry-After.
  - ETag or content changes.
  - Network add, remove or change-address events, and `setDownloadNetwork` toggles.
  - Disk faults, relink, cancel/remove, and multiple simultaneous downloads.
  - Any torrent chaos.
- **Complementing it:**
  - `lifecycle.spec.ts` and `recovery.spec.ts` are run again nightly with `--repeat-each=10` as the "race-prone" set.
  - Property tests in the pure specs: plan, scheduler deadlock-freedom and real-work, blockProgress attribution, torrent paths and files, credit.

---

### 4. CI

There are **two workflows** and no release or packaging workflow.

#### `.github/workflows/ci.yml` (PRs to main and pushes to main; concurrency group `ci-${ref}` with cancel-in-progress)
- **Job `ci`** (ubuntu-latest, Node 22, `cache: npm`):
  - Runs `npm ci --ignore-scripts`, which skips `postinstall` (patch-package, install-electron, electron-builder install-app-deps).
  - Then `npm run lint`, then `npm run typecheck` (node, web and the e2e tsconfig `tsconfig.e2e.json`: noEmit, includes e2e plus a few src `.d.ts` files), then `npm run format:check`.
- **Job `e2e` "E2E (smoke)"** (ubuntu-latest, Node 22, npm cache):
  - Runs a full `npm ci`, so the Electron binary is downloaded.
  - Then `xvfb-run --auto-servernum --server-args="-screen 0 1280x1024x24 -noreset" npm run test:e2e:smoke`, which is `playwright test --grep @smoke`. The global setup builds the app.
  - Uploads `playwright-report` on failure, kept 14 days.
- **Job `windows-torrent`** (windows-latest, Node 22):
  - Runs `npm ci --ignore-scripts` on purpose, so no native binaries are installed, then `npx patch-package`.
  - Runs `e2e/torrentNative.spec.ts` and `e2e/sparseFile.spec.ts` with `PLEXO_E2E_SKIP_BUILD=1`. Neither launches Electron.

#### `.github/workflows/e2e-nightly.yml` (cron `0 3 * * *` plus `workflow_dispatch` with a `chaos_runs` input, default 50)
- Matrix: `os: [ubuntu-latest, windows-latest]`, `fail-fast:false`, 60-minute timeout, Node 22, npm cache.
- Steps:
  1. `npm ci`, then `npx electron-vite build`.
  2. **Full suite**, all tags: `@disk` (tmpfs works on Linux through passwordless sudo and skips on Windows), `@chaos` with `PLEXO_CHAOS_RUNS=50`, and every untagged spec. Linux runs under xvfb with `-noreset`, because an Xvfb reset can race parallel launches.
  3. Race-prone tests repeated: `e2e/lifecycle.spec.ts` and `e2e/recovery.spec.ts` with `--repeat-each=10`.
  4. Upload `playwright-report-<os>` on failure, kept 14 days.

#### Tags by run
- **PR (Linux):** only `@smoke`.
- **PR (Windows):** only `torrentNative` and `sparseFile`.
- **Nightly (Linux and Windows):** everything.
- Specs with no `@smoke` tag therefore **never run on a PR**: all the pure algorithm specs, `hedge`, `streams`, `limits`, `torrent`, `torrentProbe`, `openLinks`, `selection`, `downloads`, and the untagged tests in `download.spec.ts` (the relink test) and `ui.spec.ts` (the torrent UI journey).

#### Caching, packaging, release
- **Caching:** only `actions/setup-node` npm caching. The Electron binary, Playwright and the build output are not cached.
- **Artifacts:** Playwright HTML reports on failure only. No traces or videos are configured; the debugging attachments are the events, app-output and server-request JSON files.
- **Packaging:** none in CI. `electron-builder.yml` sets `afterPack: scripts/check-torrent-package.mjs`. That hook opens `app.asar` with `@electron/asar listPackage` and throws if any `node_modules/node-datachannel` or `webrtc-polyfill` entry is present (it says rc.12 shipped a macOS binary on Windows). The hook only runs during local `build:mac`, `build:win` and `build:linux`. Targets:
  - mac: dmg for x64 and arm64, not notarized.
  - win: NSIS, one installer covering x64 and arm64.
  - linux: AppImage, deb and rpm, each for x64 and arm64.
  - The publish URL is a placeholder (`https://example.com/auto-updates`).
- **Release is manual** (`CONTRIBUTING.md`):
  1. `npm version`.
  2. Run the three build scripts.
  3. `node scripts/release-notes.mjs v<ver> notes.md > body.md`.
  4. `gh release create ... --prerelease $(node scripts/release-notes.mjs v<ver> --files)`.

  The script builds a version regex from the artifact names, filters `dist/` to files `docs/downloads.js` can `describe()`, and renders a markdown table that links to `https://github.com/anmolkapil/plexo/releases/download/<tag>/`. `CONTRIBUTING.md` says "seven files", but the builder config and `releaseNotes.spec.ts` produce nine because of the rpm builds. The docs are stale here.

---

### 5. Gaps and weaknesses

1. **No real multi-NIC testing.** The "second network" is the host's LAN IP sending to loopback, so both networks share one physical path. Nothing tests:
   - real interface binding through `network/deviceBinding.ts` (koffi on Linux);
   - `getMacHardwarePortNames` and `getWindowsAdapters` labelling;
   - the real `listActiveInterfaces` filtering, since `testInterfaces()` short-circuits it entirely;
   - real Wi-Fi or USB-tethering kinds (kind comes from a string).

   Network loss is imitated by rewriting env vars, not by an interface going down. Multi-network tests skip silently when no LAN address works, and nothing asserts that they ran on CI.
2. **No macOS CI at all**, on PR or nightly, even though the app ships dmgs and has mac-only paths: `open-url`/`open-file` (only imitated through `app.emit`), `dock.hide`, the RAM-disk `@disk` path and hardware port names. Windows gets the full suite only nightly, and Windows skips `@disk` and chmod tests. The `lsof` open-handle check never runs on Windows.
3. **Pure and unit specs don't run on PRs.** About 90 fast algorithm tests (scheduler, concurrency, plan, savedProgress, blockProgress, downloadUpdate, torrent paths, files and peers, invariants) aren't tagged `@smoke`, so a regression in them surfaces only the next night. All torrent download specs except `torrentNative` are nightly only too. There is no separate unit-test runner and no coverage measurement.
4. **The packaged app is never tested.** Every knob is disabled when `app.isPackaged`, and e2e runs only the unpackaged `out/` build. CI does no installer smoke test and no packaging step, so the `afterPack` WebRTC check only runs on a developer's machine. Nothing tests code signing or notarization (notarize is false), auto-update (`publish` is a placeholder), or file associations and protocol registration from the installer.
5. **Origin features with no tests:**
  - 429 and `Retry-After` as an HTTP date.
  - The `sharedBytesPerSecond` throttle.
  - HTTPS/TLS: everything is plain `http://`, with no certificate errors, SNI or HTTP/2.
  - Proxies; authentication (401) and cookie-gated URLs.
  - gzip `Content-Encoding` on the body (only ETag suffixes are tested).
  - Multi-range or `multipart/byteranges` replies; `If-Range` semantics.
  - Very large files above 4 GB or 2³²: only the `plan` property reaches 2⁴² bytes.
  - Real DNS: the hostname tests use an injected `resolveHost`.
6. **Torrent gaps:**
  - No DHT, uTP, PEX, LSD, UPnP or NAT-PMP.
  - No hostile or corrupt peers (bad piece data, failed hash checks).
  - No tracker failures, slow-metadata magnets with real trackers, or WebRTC/WebSocket peers.
  - No torrents with disk-full or slow-disk conditions.
  - No chaos for torrents.
7. **Chaos scope is narrow** (see §3): HTTP only, a single download, a fixed 3 MB size, a small fault alphabet, and no network-topology, disk, ETag or stall events. 5 runs locally and 50 nightly is a small sample.
8. **Disk:** ENOSPC that arrives mid-download (rather than the up-front space check) isn't clearly tested; the 40 MB volume test only proves the file fits once. Nothing tests slow fsync, EIO, or removable-drive unmounts during a write. The slow-disk test delays a single write. Disk tests skip without passwordless sudo, and nothing reports that they were skipped.
9. **UI and accessibility:** a few journeys plus a minimum-window check. There is no visual regression or screenshot diffing, no automated a11y audit (axe) of the app (the docs page checks some ARIA by hand), no theming check beyond persistence, no i18n, and no notifications (disabled whenever `PLEXO_USER_DATA` is set).
10. **No performance or throughput benchmarks, no soak or long-run tests**, and no measurement of memory or handle leaks beyond the MaxListeners warning grep and the lsof check.
11. **Security:** filename and torrent-path sanitization are well covered, and so is XSS on the docs page. Nothing covers IPC input validation from a compromised renderer, or SSRF/`file://` handling of arbitrary URLs passed to `probeUrl`.
12. **Process notes:** CI uses only Node 22, with no version matrix. There is no caching of the Electron download or the build. The PR smoke job pays for a full `npm ci` plus the build. Retries are deliberately 0, so flakiness fails the run, which is intended but leaves no quarantine mechanism other than `test.fail`.

#### Key files
- Harness: `e2e/fixtures.ts`, `e2e/origin.ts`, `e2e/torrentSwarm.ts`, `e2e/global-setup.ts`, `e2e/page-host/main.cjs`, `e2e/page-host/preload.cjs`
- Config: `playwright.config.ts`, `tsconfig.e2e.json`, `package.json` (`test:e2e`, `test:e2e:smoke`, `typecheck:e2e`)
- Knobs: `src/main/testKnobs.ts`, read in `src/main/index.ts`, `download/{probe,chunkDownloader,httpTransfer,downloadManager}.ts`, `download/torrent/{engine,metadata}.ts`, `network/interfaces.ts`, `ipc/handlers.ts`
- CI: `.github/workflows/ci.yml`, `.github/workflows/e2e-nightly.yml`
- Packaging and release: `electron-builder.yml`, `scripts/check-torrent-package.mjs`, `scripts/release-notes.mjs`, `CONTRIBUTING.md`


---

# Part 7: Weaknesses, tech debt and risks

These are my own observations from reading the code, cross-checked against the subagent findings in Parts 1, 2 and 6. Severity: **H**igh, **M**edium or **L**ow, in terms of what it means for us.

## 7.1 Distribution and trust

| # | Issue | Evidence | Severity | What we do instead |
|---|---|---|---|---|
| 1 | **Unsigned and un-notarized on every OS.** macOS users have to run `xattr -dr com.apple.quarantine`. Windows gets SmartScreen warnings. The landing page documents workarounds, and two of them disagree (`xattr -dr` vs `xattr -cr`). | `electron-builder.yml` `notarize: false`, no `identity` and no Windows signing config. Commits 8daba37 and d334fc9. | H | Budget an Apple Developer ID plus notarization, and an Authenticode/Azure Trusted Signing certificate, from day one. Sign in CI. |
| 2 | **No auto-update.** `publish.url` is a placeholder, `https://example.com/auto-updates`. The "update check" only compares the newest GitHub release tag and sends the user to the website. | `electron-builder.yml`. `src/main/updateCheck.ts`. | M | electron-updater (or Squirrel/Sparkle) against signed artifacts. Delta updates via blockmaps. |
| 3 | **Naive version compare.** Numeric segments are split on `.`/`-`, so `1.0.0-rc.2` becomes `[1,0,0,0,2]`, and `1.0.0` (`[1,0,0]`) reads as *older* than `1.0.0-rc.14`. When 1.0.0 ships, rc users will **never** be told about it. Any build metadata (`+build`) also breaks it. The author notes it in a `ponytail:` comment. | `src/main/updateCheck.ts:9-25`. | H, as a latent bug | Use real semver (the `semver` package). Add a unit test for prerelease → release ordering. |
| 4 | **Update source is `releases?per_page=1`,** which is the most recently *created* release, not the highest version. A hotfix to an older line, or a draft ordering quirk, picks the wrong one. Unauthenticated GitHub API calls are limited to 60/hour per IP (NAT'd offices). | `updateCheck.ts:36`. Commit 3946d64. | M | A signed update feed that we control (a JSON manifest on our CDN). |
| 5 | **Packaging is never tested in CI.** There is no build-installers job. The `afterPack` guard (`scripts/check-torrent-package.mjs`) only runs on a developer's laptop. rc.12 shipped torrents that crashed on Windows, because the macOS-host build bundled `node-datachannel` (8cc15e4). Releases are built on one machine for all OSes (cross-build). | `.github/workflows/*`, `CONTRIBUTING.md`. | H | Native per-OS build runners. Smoke-launch the *packaged* app in CI (run headless and check that `--version` exits 0). Assert the asar contents. |
| 6 | **Build scripts are inconsistent.** `build:win` runs typecheck, but `build:mac` and `build:linux` skip it (`electron-vite build && electron-builder`). | `package.json` scripts. | L | One `release` script per target, always gated. |
| 7 | **Placeholder Linux maintainer** (`maintainer: electronjs.org`) in deb/rpm metadata. | `electron-builder.yml:71`. | L | Real maintainer and email. Repo signing for apt/rpm if we offer repos. |
| 8 | **Manual release process.** `npm version`, three local builds, `release-notes.mjs`, `gh release create --prerelease`. CONTRIBUTING says "seven files" while the spec expects nine. | `CONTRIBUTING.md`, `e2e/releaseNotes.spec.ts`. | M | Tag-triggered release workflow with provenance/attestations. |
| 9 | **Old manifests are cleared, not migrated.** Each manifest version bump (v1 → v7) throws away in-progress downloads from older versions. That was acceptable for an rc. | 5404d92, `savedProgress.ts`. | M | Versioned schema with forward migrations. Never drop user progress silently. |

## 7.2 Electron security posture

The posture is reasonable overall, with gaps:
- **Good:**
  - `contextIsolation` is on (the default).
  - Only a typed `window.plexo` API is exposed. The raw `ipcRenderer`/`electronAPI` is deliberately *not* bridged (`src/preload/index.ts`, 715e257).
  - `sandbox:false` was removed (b1a39eb), so the renderer gets Electron's default sandbox.
  - `setWindowOpenHandler` passes only `http(s)` URLs to `shell.openExternal` and denies everything else (`src/main/index.ts`).
  - The CSP meta tag is `default-src 'self'; script-src 'self'`.
  - Test knobs are ignored when `app.isPackaged` (`testKnobs.ts`).
  - Settings are sanitized field by field (`settings.ts`).
- **Gaps:**
  1. **No runtime validation of IPC arguments.** `IpcContract` is TypeScript-only. `handle()` casts the listener (`handlers.ts:52-62`). `startDownload`, `relinkDownload`, `freeSpace(dir)`, `chooseTorrentFiles(id, selected[])` and `setDownloadNetwork` trust renderer-supplied shapes. Only `updateSettings` is sanitized. A compromised renderer (XSS through a filename or torrent metadata rendered into the DOM) could start downloads to arbitrary absolute paths, or probe arbitrary URLs, including `file:` and intranet addresses (SSRF from the user's machine, which matters little on a desktop). **For us:** schema-validate every IPC payload in main (zod/valibot). Restrict destinations to user-chosen folders. Allow only http(s)/magnet schemes in probe.
  2. **No sender verification.** Handlers don't check `event.senderFrame.url`, so any frame that could reach `ipcRenderer` gets the full API.
  3. **No `will-navigate` / `will-attach-webview` guards.** Nothing stops the main window navigating away if an `<a href>` is ever rendered without `target=_blank`. Nothing calls `session.setPermissionRequestHandler`, so notifications, media and similar permissions use the defaults.
  4. **The CSP allows `style-src 'unsafe-inline'`.** That's minor, but it is still an XSS foothold for CSS exfiltration.
  5. **`exec('gnome-control-center network || nm-connection-editor || true')`** goes through a shell (`handlers.ts:40`). The string is constant, so it isn't injectable, but `execFile` with a fallback chain is cleaner.
  6. **The preload falls back to `window.plexo = plexoApi` without context isolation** (dead code today, but a foot-gun).
  7. **No Electron fuses** (`RunAsNode`, `EnableNodeOptionsEnvironmentVariable`, `EnableEmbeddedAsarIntegrityValidation`, `OnlyLoadAppFromAsar`). An unsigned app with default fuses can be driven via `ELECTRON_RUN_AS_NODE`. **For us:** flip the fuses at package time with `@electron/fuses`.
  8. **The Windows torrent fix depends on patching third-party packages with `patch-package`.** Patches are pinned to exact versions (webtorrent 3.0.21, @thaunknown/simple-peer 10.1.2). Every upgrade means a manual rebase, and a silent patch failure would re-introduce WebRTC or unbound peers. CI does run `npx patch-package` explicitly on Windows.

## 7.3 Architecture and code health

- **God objects.** `downloadManager.ts` (1722 lines) and `httpTransfer.ts` (1186) hold lifecycle, persistence, queueing, notifications, power-save, restore and the run loop. The 5404d92 split (2168 → 936, then it grew back) shows the pressure. **For us:** separate the lifecycle state machine, the persistence store, the transfer engine and the OS-integration adapters from the start, behind interfaces, so they can be unit-tested without Electron.
- **No unit-test runner.** "Unit" tests are Playwright specs that import `src/main` modules. They're untagged, so roughly 90 fast pure tests (scheduler, concurrency, plan, savedProgress and others) **never run on PRs**, only nightly. **For us:** Vitest for pure logic on every PR, with coverage. Playwright only for e2e.
- **Polling everywhere.**
  - Main polls `os.networkInterfaces()` **every 1000 ms** (`network/interfaces.ts:185`). A device-set change also costs a `networksetup` or **PowerShell** child process (up to 5 s timeout), with a cache keyed by device names.
  - The renderer polls usage every 2 s (`hooks/useNetworks.ts:17`) and free space every 30 s (`StatusBar.tsx:13`), plus 1 s, 30 s and 60 s "now" tickers.
  - The HTTP run loop idles at 250 ms (`httpTransfer.ts:184`).

  Change detection is `JSON.stringify` equality on the whole list. PowerShell startup on Windows is slow (around 300–1000 ms) and shows up as CPU spikes on docking and undocking. **For us:** use OS change notifications where available: `NotifyIpInterfaceChange` (Windows), `SCNetworkReachability`/`NWPathMonitor` (macOS), netlink `RTMGRP_LINK|IPV4_IFADDR` (Linux). Keep polling as a fallback. Push usage to the renderer instead of pulling it.
- **Sync IPC at startup.** `getInitialState` uses `sendSync`, which blocks the renderer. It's capped by a 300 ms `stat` race for the destination (765964f), but it's still a sync round-trip on the launch path. **For us:** prefer an async bootstrap with a skeleton screen, or preload the data with `additionalArguments`.
- **Persistence.** Writes in `jsonFile.ts` are atomic (temp file + rename, with a per-file queue and EPERM/EBUSY rename retries on Windows), **but there is no `fsync`** of the temp file or the directory. After a power loss, the file can rename into place as a zero-length or garbage file. A corrupt file is then read as `undefined`, which **silently resets settings, history and usage** (`readJsonNow` returns undefined on SyntaxError), with no backup. History is capped at 500 entries (`history.ts:12`). **For us:** fsync the file and its parent directory, keep a `.bak` of the last good copy, log or warn on corruption. Consider SQLite (better-sqlite3) for the queue, history and usage.
- **Manifests in `userData/downloads/<id>/manifest.json`** are rewritten on a checkpoint cadence. Large downloads keep tens of thousands of block records in JSON. Rewriting the whole object on every checkpoint is O(blocks). That's fine at 8 MB blocks but grows with file size.
- **Error handling vs the user's own rules** (a 413/415 equivalent: specific, actionable errors). Plexo translates errors through one translator (df45254) and strips Electron's "Error invoking remote method" prefix (38a954d). However, `checkForUpdate` swallows all failures, the network-label lookups swallow errors (falling back to raw names), and `freeSpace` returns `null` on any error. Those are deliberate, but they're invisible when debugging. **For us:** structured logging (a rotating log file plus a "Copy diagnostics" button). Plexo has none: there's only `console.error` and `PLEXO_DEBUG=1`.
- **No telemetry or crash reporting** (no `crashReporter`, no Sentry). Field failures like the rc.12 Windows torrent crash were found by users. **For us:** opt-in crash reporting.
- **Network colour stability.** Unpinned colours are assigned by sorted id, so plugging in a same-kind adapter can shift colours (`theme.ts:123`, a `ponytail` note).
- **Torrent peer spreading is "fewest peers first"** and ignores each network's measured throughput (`torrent/peers.ts:16`, a `ponytail` note). On asymmetric links (fast fibre plus a slow phone) a torrent underuses the fast link.
- **Webtorrent backpressure quirk.** WebTorrent's pipe resumes the socket when its wire drains, which can cut a speed limit short (`torrentTransfer.ts:293`, a `ponytail` note). Speed limits on torrents are approximate.
- **Torrents are cut down.** No DHT/uTP/LSD/UPnP/NAT-PMP/WebRTC (they bypass the per-network connect hook), and no seeding after completion. Users who expect a real torrent client will find it slow on poorly-seeded torrents and unable to reach NAT'd peers.
- **Missing table-stakes features** (from Part 1 §19 and Part 6):
  - No browser extension or download-capture integration.
  - No proxy support. Nothing handles HTTP auth or cookies, so links that need a login fail.
  - No HTTPS-specific tests and no HTTP/2.
  - No checksum verification UI (user-supplied SHA-256).
  - No scheduling by time of day, no queue reordering, no tray/menu-bar mode, no dock/taskbar progress, no window-state persistence, no i18n.
  - `cancelDownload` IPC is unused by the renderer.
  - `docs/changelog.js` is orphaned.
- **Platform reality.** Bonding on Windows is limited by interface metrics: two adapters on the same subnet or router give no gain, and the app only *warns* about it (32f5e09). On Linux it needs kernel ≥ 5.7 for unprivileged `SO_BINDTODEVICE`. On macOS it relies on source-address binding, which macOS honours per interface (scoped routing). **For us:** document these per OS, detect them at runtime, and show the user an honest "bonding effective / not effective" indicator.

## 7.4 Testing and CI gaps (summary, detail in Part 6 §5)

- **No macOS CI at all,** although the dmg is a primary target. Windows runs the full suite only nightly.
- **No real multi-NIC tests.** The "second network" is the host LAN IP sending to loopback, so `deviceBinding.ts`, `getMacHardwarePortNames` and `getWindowsAdapters` are untested. Multi-network tests skip silently when there is no LAN address.
- `@disk` needs passwordless sudo (Linux tmpfs) or `hdiutil` (macOS). It is always skipped on Windows, and skips aren't reported.
- **The `lsof` open-handle check is silently skipped where `lsof` is absent,** which includes Windows, the platform most prone to handle-lock bugs.
- **The chaos alphabet is narrow:** HTTP only, one download, 3 MB, with no stalls, no 429, no ETag changes, no network topology events and no disk faults.
- **Untested:** 429/Retry-After as an HTTP date, TLS/HTTP2, proxies, 401/cookies, `Content-Encoding: gzip` bodies, files over 4 GB end to end, and real DNS.
- Node 22 only. Electron, Playwright and the build output aren't cached. `retries: 0` is deliberate, and good discipline.

## 7.5 Risks specific to multi-interface bonding (carry these into our design)

1. **Per-network data caps and USB tethering.** Plexo counts usage per network per day, week or month. It relies on the app's own byte counts, not OS counters, so traffic from other apps on a metered phone tether isn't seen. We should state this in the UI.
2. **Server-side abuse heuristics.** Many parallel ranges from two different public IPs can trip CDN anti-leech protection or produce 403/429 responses. Plexo's adaptive ceiling (start at 8, double up to 32, back off on refusals) mitigates this. We need a polite default and per-host memory of connection limits.
3. **Signed or expiring URLs bound to the client IP.** A URL signed for the Wi-Fi IP fails over the tether. Plexo blames that network (it becomes `unreachable`, or 403) and continues on the other. We should detect a per-network 403 and stop using that network for the host.
4. **Battery and thermal cost** of keeping a phone tether and Wi-Fi saturated, plus `powerSaveBlocker` keeping the computer awake while downloading. Make that visible and controllable.


---

# Part 8: Cross-cutting rules for our project (synthesis)

1. **Verify behaviour, not headers.**
   - Probe ranges with `bytes=0-0`.
   - Accept a 206 only if Content-Range starts at exactly the requested offset and the body length matches.
   - Normalize validators: strip `W/`, quotes and `-gzip`.
   - Settle a change in validators only by sampling bytes already on disk (up to 8 samples). A change in size is fatal.
2. **One staging file at fixed offsets, on the destination volume.**
   - Make it sparse on NTFS.
   - Claim the name with an exclusive create.
   - Publish with `lstat` + `rename`, not a hard link (exFAT).
   - Budget 255 bytes per path component, including suffixes.
   - Check free space for roughly 1× the file size.
3. **Layered deadlines.** DNS, connect (10 s), first byte, and body silence (20 s) all sit inside a total deadline. Happy-Eyeballs across a network's own addresses, 250 ms apart.
4. **Attribute every failure to a layer**, because each one is handled differently:
   - **Server busy** (408/429/5xx): wait it out for up to 5 minutes, honouring Retry-After capped at 2 minutes, for the whole network.
   - **Server refusal** (4xx): counts as a strike.
   - **Network gone:** wait instead of failing.
   - **Network can't reach the host:** mark it unreachable and keep probing with 1 stream.
   - **Disk backpressure:** pause the watchdogs and don't hedge.
   - **Sleep/wake:** refresh connections.
5. **Scheduling as a pure function of a snapshot.**
   - Property-test it for deadlock freedom.
   - Block size is 1–8 MiB, about 2 blocks per stream.
   - Interleave streams across networks.
   - Hedge a slow tail block: at most 2 hedges per block, another network first, the first finisher wins, and writes are idempotent at fixed offsets.
6. **Simple concurrency rule.** Start at 8 streams per network, double while every stream is served, cap at 32. Back off only on observed refusals, where a stream that is accepted but never answers counts as a refusal. Recover one stream per minute. Keep any user override.
7. **Keep-alive through a real Agent.** A custom `createConnection` with no agent silently disables pooling.
8. **Per-OS binding.**
   - Linux needs `SO_BINDTODEVICE` (kernel ≥ 5.7 for unprivileged use); detect it and degrade visibly.
   - Windows routes by interface metric, so warn when two adapters share a subnet.
   - Never pin loopback.
   - Parse OS tools by position, never by localized labels.
9. **Persistence.**
   - Temp file + rename, plus fsync, which Plexo lacks.
   - A per-file write queue.
   - "Missing" is not "unreadable": never overwrite a file you couldn't read.
   - A versioned schema with migrations, which Plexo lacks.
   - Freeze writes while shutting down.
   - Reconcile with the disk before resuming.
10. **Measure on a clock.**
    - Read rate meters on a tick, with a floor of at least 1 s on the window.
    - Peak = the best 5-second average.
    - ETA is computed once, in main, with asymmetric smoothing: 30%/s toward a lower estimate, 10%/s toward a higher one.
    - Send only changed blocks to the renderer, with sequence numbers.
11. **Packaging discipline.**
    - Set the architecture explicitly for every target.
    - Build on native runners.
    - Assert the asar contents after packing.
    - Smoke-launch the packaged app in CI.
    - Sign and notarize.
    - Use real semver.
    - Use a CI job per OS, including macOS.
12. **Testing discipline.**
    - A fault-injecting origin with byte-exact `hold()`.
    - Invariant checks on every event.
    - sha256 of every finished file.
    - Fail on Node warnings and on leaked handles.
    - Isolate temp directories per test.
    - Model-based chaos testing with shrinking and seed replay.
    - Retries off.
    - Fast pure tests run on every PR, unlike Plexo.
13. **Security baseline.** Typed *and* runtime-validated IPC, sender checks, `openExternal` allow-list, navigation guards, Electron fuses, a strict CSP, and test knobs compiled out of packaged builds.
