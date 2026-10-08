# Fuse Send: sharing a file directly, over every network

Decision: [ADR 0011](../adr/0011-fuse-send-p2p.md). This replaces the cloud design in
[BONDED-UPLOADS.md](BONDED-UPLOADS.md) (kept for history).

## 1. What the person sees

1. Drop a file (or a folder) on Fuselane and choose **Send**.
2. Fuselane prepares it (one pass over the file, with progress) and shows a link plus
   **Copy link**, a QR code, and a line: "Keep Fuselane open until it arrives."
3. The receiver opens the link. Fuselane opens (or the page says how to get it), shows the
   name and size, and downloads over every network both sides have.
4. The sender sees "Sending to 1 person · 2.1 GB of 5.4 GB" and then "Arrived".
   Stopping the share is one click; links can also stop after the first full download.

## 2. The link

```
https://arshpunisher.github.io/fuselane/s#v1.<base64url(info-hash ‖ key ‖ flags)>
```

- The page is a static file on GitHub Pages (free). Everything after `#` stays in the
  browser and is never sent to GitHub or anyone else.
- The page hands the link to the app (`fuselane://send/…`); if the app isn't installed it
  explains how to install it. It never asks for or stores anything.
- `fuselane://send/v1.…` also works directly (pasted into the app).

## 3. How the bytes move

- **Torrent**: Fuselane builds a v1 torrent over the *encrypted* bytes with
  `librqbit::create_torrent`, private flag off so DHT works, and a generic name
  (the real name is inside the encrypted header).
- **Finding each other**: the public mainline DHT, plus a short list of public UDP trackers
  as a fallback. Both are free and run by the community.
- **Bonding**: on both sides peer connections go through Fuselane's per-network relay, the
  same one torrents already use (TORRENT.md), so both upload and download are spread across
  networks.
- **Reachability**: UPnP port forwarding (librqbit) on every network that allows it, uTP and
  TCP. Spike S7 measures how often two home or phone connections can reach each other.
  Incoming peers per network (L-71) matter here, because the sender is usually the one who
  needs to be reachable.

## 4. Encryption without a second copy

- **Key**: 32 random bytes per share, only in the link.
- **Cipher**: XChaCha20 keystream XORed at the byte offset (seekable, so any piece can be
  served without reading earlier ones), plus a header with the name, size and a BLAKE3 hash
  of the plaintext, sealed with XChaCha20-Poly1305.
- **Integrity**: torrent piece hashes cover the ciphertext (no tampering in transit), and the
  BLAKE3 hash checks the decrypted whole at the end.
- **No temp copy**: a custom librqbit `StorageFactory` encrypts on read (sender) and
  decrypts on write (receiver), so a 30 GB file never needs 30 GB more disk.
- **Why not torrent-level obfuscation**: BitTorrent's MSE hides traffic from simple
  filters but not contents from peers. Our encryption is what keeps the file private.

## 5. Errors (ERRORS.md rules: say what happened and what to do)

| Situation | What the app says |
|---|---|
| Link malformed or from a newer version | "This link isn't complete. Ask for it again." / "Update Fuselane to open this link." |
| Sender offline, nobody found after 60 s | "The sender isn't online right now. Fuselane will keep looking; ask them to open Fuselane." |
| Found but can't connect (both behind strict NAT) | "You and the sender can't reach each other on these networks. Try another network (for example, a phone hotspot on one side)." |
| Wrong key / tampered | "This file didn't check out and was deleted. Ask the sender for a new link." |
| Not enough disk | "This needs 5.4 GB and only 3.1 GB is free on Macintosh HD." |
| File changed on the sender's side while sharing | The share stops: "This file changed after it was shared. Share it again." |

## 6. Limits and honesty

- The sender must stay online; nothing is stored anywhere else.
- DHT announces are public: observers can see that an info-hash exists and which IPs
  share it, but never names or contents. The share page says so in one line.
- A share is not anonymous: peers see each other's IP addresses, as in any direct transfer.

## 7. Later, still $0 to Fuselane

- **Same network**: discover the receiver on the LAN (mDNS) and skip the DHT entirely.
- **Bring your own cloud**: bonded upload into the user's own Dropbox (concurrent upload
  sessions) or S3-compatible bucket, at their cost, for "voicemail" shares.
- **Browser receiving**: WebRTC peers (WebTorrent), if librqbit or a sidecar ever supports it.
