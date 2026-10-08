# HTTP download engine (`crates/engine-http`, `crates/storage`)

The engine must satisfy every rule in [`../02-product/LESSONS-FROM-PLEXO.md`](../02-product/LESSONS-FROM-PLEXO.md) §1–7. The constants below are **starting values** (Plexo's tuned ones, as a reference point); our network-lab benchmarks will tune them, and every one lives in a single `Tuning` struct.

## 1. Pipeline

```text
probe → plan → claim staging file → run (scheduler + streams) → verify → publish → history
            ↑                                  │
            └──── resume (reconcile with disk) ┘
```

## 2. Probe

1. Validate the URL: only `http`, `https` and (for torrents) `magnet` (L-97).
2. Send `GET` with `Range: bytes=0-0` and `Accept-Encoding: identity`, following up to 5 redirects within one 20 s total budget (L-10). Use the first selected network.
3. Results:
   - **206** with a parseable Content-Range means ranges are supported, and the total comes from `/N`.
   - **200** means a single stream, size from Content-Length (or unknown).
   - **416 `bytes */0`** means an empty file.
4. Record the final URL, the version (size, normalized ETag, Last-Modified), Content-Type and the file name (RFC 6266/5987, L-08).
5. A torrent MIME type or `.torrent` name hands over to the torrent engine.
6. **Per-network target probe** (NETWORKING.md §6) decides which selected networks may join.

## 3. Plan

| Constant | Start value | Note |
|---|---|---|
| `max_block` | 8 MiB | Caps what a failed or raced block costs |
| `min_block` | 1 MiB | |
| `blocks_per_stream` | 2 | Every stream has at least 2 blocks of work |
| block size | `clamp(ceil(size / (networks × 32 × 2)), 1 MiB, 8 MiB)` | Fixed for the job's life; recorded in the DB |
| unknown size or no ranges | 1 open-ended block, 1 stream, 1 network | Enabling another network moves the job there |

## 4. Scheduler (pure function, L-24)

`pick_work(snapshot, stream) -> Option<Work>`:
1. **Primary:** the lowest pending block, skipping blocks marked "avoid this network" (the last attempt here delivered 0 bytes) while another network has an idle stream. It never leaves a block stranded.
2. **Hedge** (only when nothing is pending and the disk is keeping up, L-25, L-27):
   - Target a downloading block where every attempt has run for at least 2 s.
   - Its ETA must be at least 2× what this stream could do.
   - At most 2 hedges per block.
   - Prefer another network.
   - Pick the block with the largest ETA.
   - The hedge starts at the block's frontier and writes the same bytes to the same offsets; the first to finish wins and the others are aborted as `lost`.
3. **Split (new, + over Plexo):** if a block's remaining size is at least 4 MiB and its holder is much slower, split off the tail as a new pending block instead of hedging. This is decided by the same pure function and property-tested.
4. Starts are interleaved across networks (L-23).

Property tests: every byte is covered exactly once by completed blocks; no deadlock (some stream always has work while blocks remain); hedges never exceed their caps; when both a split and a hedge are possible, the result is the same either way.

## 5. Streams and concurrency (pure controller, L-26/27)

| Constant | Start value |
|---|---|
| starting streams per network | 8 (or the user's 4/8/16/32) |
| growth | double while **every** stream has received data, up to 32 |
| refusal | `ceiling = streams − refused`, only when others are served at the same time (L-18) |
| recovery | +1 stream per 60 s without a refusal |
| disk behind for 500 ms | cap = ceil(max streams / 2) for every network; ×2 after 10 s keeping up |
| per-host memory (+) | remember the last refusal ceiling per host for 24 h, so the next job starts polite |

Each stream holds one keep-alive connection on its network and loops: pick work → request → stream the body to the writer → report.

## 6. Requests and checking responses (L-01–L-08)

- Headers: `Range: bytes=a-b`, `If-Range: <etag or last-modified>`, `Accept-Encoding: identity`, `User-Agent: Fuselane/<ver>`, plus the forwarded cookies, Referer and auth when the job came from the extension.
- Follow redirects per request, keeping Range and the network pin (Plexo c2e9281).
- Accept a response only if:
  - it is a 206 with Content-Range starting exactly at `a` and not past `b`, and the body is exactly `b−a+1` bytes (cut and fail an overrun); **or**
  - it is a 200 when `a == 0` and the job is single-stream.
- The version check runs on every response before any byte is written (L-05).

## 7. Retry policy (L-14–L-21)

| Situation | Action |
|---|---|
| Connection error / timeout | Retry forever while the network exists. Backoff `min(1 s·2^(n−1), 15 s) × (0.8–1.2)`; reset after progress. |
| Network silent for 5 s while it is the only one trying | `Unreachable`: 1 probe stream, retry every 5 s |
| 408/429/5xx | Wait it out for up to 5 min. Retry-After (capped at 2 min) holds **the whole network**. |
| 401/403/404/410 | Strike. If every network gets it, the job becomes `Failed{LinkExpired}` → "Fix link". If only some networks do, mark them `Blocked` (IP-locked). |
| Wrong range or short body | Strike; after 5 in a row the stream retires |
| Version changed | Sample up to 8 × 16 KiB of bytes on disk: same → accept the new version; different → fail and discard |
| Write error ENOSPC/EDQUOT | **Pause the job immediately** with "Not enough space" (+ over Plexo's 5 strikes) |
| Write error EIO/EROFS | Pause with a drive message |
| Sleep/wake, address change | Refresh the affected connections; not a failure |

## 8. Storage (`crates/storage`)

- **Staging file:** `<final name>.fuselane` in the destination folder, created with an exclusive create (L-37).
  - Then preallocated (fallocate / F_PREALLOCATE / SetFileInformationByHandle).
  - Or, where preallocation isn't supported, sparse via `set_len`; sparse on NTFS with `FSCTL_SET_SPARSE` (L-41).
  - FAT32: refuse files over 4 GiB with a clear message.
- **Writer pool:** a fixed set of blocking threads fed by a bounded channel. Positional writes (`pwrite` / `seek_write` in a loop) are coalesced to at least 512 KiB. When the channel is full, streams stop reading, which is TCP backpressure; the watchdog is paused meanwhile (L-13).
- **Durability:** fsync the staging file, then record progress in SQLite (L-55), every 15 s and at state changes.
- **Publish:**
  1. Check every block is complete and the size matches (L-43).
  2. Run the optional checksum.
  3. Record the publish intent.
  4. Check the final name is free (otherwise use `name (N).ext`).
  5. Rename, retrying on Windows lock errors (L-45).
  6. fsync the directory.
  7. Mark the history entry.
  8. Apply quarantine / Mark of the Web (L-99).
- **Names:** sanitize for all three OSes at once, so a file name is portable (L-40), within 255 bytes (L-39).

## 9. Resume

1. Load the job; validate the plan and progress rows against the file size (L-53).
2. Check the staging file exists. Missing → `Failed{not resumable}`. On an unmounted drive → "Reconnect the drive" (L-48).
3. Reconcile: never trust progress beyond what the last fsync covered.
4. Re-probe; compare versions (L-05). Then schedule only the bytes still missing.

## 10. Measurement (L-30–L-35)

- Meters per stream and per network: a 3 s rolling window, read on a 500 ms tick, with a 1 s minimum denominator.
- AVG = bytes ÷ active time. PEAK = best 5 s window (never below AVG). ETA smoothed asymmetrically.
- 60 s history per network for the chart.

## 11. Checksums (+)

- Inputs: user-pasted hex (MD5/SHA-1/SHA-256/SHA-512, length decides), a `.sha256` file next to the URL, `Digest`/`Repr-Digest` headers, or Metalink.
- Verification runs at publish, hashing the staging file in one sequential pass on the writer pool. If the file supplies per-block hashes (Metalink pieces), only bad blocks are re-fetched.

## 12. Limits

- Token buckets (1 s capacity, can go into debt) at three levels: global, per network, per job (+). The largest wait wins. Slow mode replaces the global limit.
- Data usage counted per network per day/week/month, and for downloads and uploads separately. Reaching a limit → network `AtLimit` and its streams retire.
