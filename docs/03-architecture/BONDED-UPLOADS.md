# Bonded uploads and share links

> **Superseded (2026-10-09)** by [FUSE-SEND.md](FUSE-SEND.md) and [ADR 0011](../adr/0011-fuse-send-p2p.md): no hosted storage. Kept for history; nothing here is planned.

The feature that sets Fuselane apart. Crates: `engine-upload`, `crypto`. Service: `apps/backend`. Decision: [ADR 0007](../adr/0007-upload-backend.md).

## 1. Why it works

S3-compatible multipart upload accepts **parts in any order, in parallel, from any IP**. Each network PUTs different parts at the same time, and the backend completes the object. Google Drive, YouTube and OneDrive need their chunks in order, so they can't be bonded; Dropbox "concurrent" sessions can (later).

## 2. Protocol limits we design around

| Rule | Value | Consequence |
|---|---|---|
| Part size | 5 MiB – 5 GiB (the last part may be smaller) | |
| Parts per object | 10,000 max | `part = max(16 MiB, ceil(size / 9,000))`, rounded up to 1 MiB |
| **R2: every part except the last must be the same size** | – | Parts never split dynamically; balancing happens part by part (fast networks take more parts) |
| Incomplete multipart uploads | R2 aborts them after 7 days by default | Resume must finish within the expiry; lifecycle rule as a safety net |
| Object size | 5 TiB | |

## 3. Flow

```text
client                                   backend (Worker + D1 + R2)
  │ POST /v1/uploads {name,size,mime,enc,expiry,…} ─►  validate tier limits (413 if too big)
  │                                                    CreateMultipartUpload → row in D1
  │ ◄─ {uploadId, partSize, partCount, objectKey}
  │ POST /v1/uploads/:id/parts {from,count} ───────►  presign UploadPart URLs (TTL 60 min)
  │ ◄─ [{n, url}]   (batches of about 50; refreshed as needed)
  │ PUT part n ── over Wi-Fi ┐
  │ PUT part m ── over tether├────────────────────────► straight to R2 (no bytes go through the Worker)
  │ PUT part k ── over Eth ──┘     each returns ETag → stored in SQLite
  │ POST /v1/uploads/:id/complete {parts:[{n,etag}]} ► CompleteMultipartUpload → share row
  │ ◄─ {shareUrl: https://fuselane.app/s/<slug>[#k=<key>]}
```

- **The client never holds R2 credentials**; it only gets presigned per-part URLs.
- **Resume:** the upload ID, part size and every finished part's ETag are in SQLite. On resume the client calls `GET /v1/uploads/:id/parts` (backend ListParts), reconciles, and uploads only the missing parts.
- **Scheduling:** the same pure scheduler as downloads, over parts. Hedging the last slow part is allowed: a second PUT of the same part number with identical bytes is harmless, and the ETag that returns first is used.
- **Upload concurrency:** start with 2 parts in flight per network and grow while the network's throughput rises (uplinks fill with fewer connections than downlinks). It's a separate controller with its own tuning, the same pure-function style.
- **Reading:** each in-flight part is streamed from disk with positional reads; parts are never buffered whole.
- **Per-part integrity:** send `Content-MD5` (or `x-amz-checksum-crc32c` if R2 supports it — **verify in spike S6**) so corruption in transit is rejected.

## 4. Share links

- Slug: 128-bit random, base62. URL: `https://fuselane.app/s/<slug>`.
- Options: expiry (1, 7 or 30 days; the tier decides the maximum), download-count limit, password (Argon2id-wrapped key or a server-side hash for unencrypted shares).
- **Share page** (served by the Worker): file name, size, expiry, a Download button. The file is served by a 302 redirect to a short-lived presigned GET, which supports ranges, so **receivers using Fuselane download it bonded too**.
- A Cron Trigger every hour deletes expired objects and rows. R2 lifecycle rules are a backstop (object age, abort incomplete multipart after 2 days).

## 5. Optional end-to-end encryption

- A random 256-bit key `K`. `PK = HKDF-SHA256(K, salt, "fuselane payload v1")`.
- The plaintext is split into **64 KiB records**. Record `i` is encrypted with AES-256-GCM, nonce = `prefix(7) ‖ be32(i) ‖ last_flag(1)` (STREAM construction). Each record has a 16-byte tag, so ciphertext size and offsets are deterministic.
- Part size is a multiple of the record size, so **every part is encrypted on its own, in parallel, on any network**.
- Header record 0 holds the version, salt, record size and encrypted metadata (file name, MIME).
- The key travels in the URL **fragment** (`#k=…`), so it never reaches the server. The browser decrypts with WebCrypto, streaming through a Service Worker. Fuselane receivers decrypt per segment.
- Test vectors are shared between `crates/crypto` and `packages/crypto-web`; both must pass the same JSON vectors in CI.
- With encryption on, we can't scan content. That's a policy decision for the abuse posture (see §7 and open questions).

## 6. Backend API errors (following the user's error-handling standard)

Every error returns JSON `{ "error": { "code": "…", "message": "…", "hint": "…" } }` with the right status, never a generic 500:

| Case | Status | Example message |
|---|---|---|
| File larger than the tier allows | **413** | "This file is 62 GB. Free links allow up to 50 GB. Split it or upgrade." |
| File type blocked by policy (if any) | **415** | "Executable files can't be shared with free links." |
| Bad request body | 400 | "`size` must be a positive integer." |
| Missing or invalid device token | 401 | "Sign in again from the app." |
| Not your upload | 403 | – |
| Upload ID unknown or already completed | 404 / 409 | "This upload was already completed." |
| Share link expired or deleted | **410** | "This link expired on 12 Oct. Ask the sender for a new one." |
| Download limit reached | 410 | "This link reached its download limit." |
| Rate limited | **429** + Retry-After | "Too many uploads at once. Try again in 30 s." |
| R2 or D1 unavailable | 503 + Retry-After | "Storage is busy. The app retries automatically." |

## 7. Accounts, quotas, abuse (decisions open: see OPEN-QUESTIONS.md)

- **MVP auth:** a device key created on first use (no sign-up), stored in the OS keychain. Optional account later to manage links across devices.
- **Quotas per device and per IP:** size per link, active storage, links per day.
- **Abuse:** a report-link button on every share page, a takedown process, rate limits (Workers rate-limiting binding), terms of service, no anonymous public listing. Firefox Send was shut down over abuse, so this has to be in place before launch.

## 8. Cost sketch (R2, 2026 prices). Policy: stay inside free tiers (ADR 0009)

- 10 GB file, 16 MiB parts → 640 Class A ops ≈ **$0.003**. Storage 10 GB × 7 days ≈ **$0.035**. Egress **$0**.
- The free tier covers roughly the first 10 GB stored and 1 M Class A ops a month.

## 9. Spike S6 must answer

1. Real R2 multipart from 2 pinned networks in parallel: does throughput add up? Do presigned URLs work from different IPs?
2. Is per-part checksum support (`Content-MD5` / `x-amz-checksum-*`) available on R2?
3. Is the equal-part-size rule confirmed, and is the error clear if it's broken?
4. Worker request limits for the control-plane routes.
