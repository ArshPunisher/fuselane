# 0011. Fuse Send: peer-to-peer sharing replaces cloud uploads
- Status: Accepted (design); the connection rate is proven or disproven by spike S7
- Date: 2026-10-09
- Supersedes: [ADR 0007](0007-upload-backend.md), and the "Upload storage" row of [ADR 0009](0009-zero-cost-policy.md)

## Context
ADR 0007 put shared files in Cloudflare R2 behind a Worker. At 10,000 users sharing about 5.5 GB a month, that is roughly $200 a month at 7-day links and $800 at 30-day links. Keeping it free would need quotas or a paid plan. The owner's decision (2026-10-09): **no plans, and no hosted service whose cost grows with users.** The cloud upload design is dropped, and the Worker, the R2 bucket and the code are removed.

## Decision
Sharing becomes **direct, computer to computer**, built on the torrent engine Fuselane already has ([FUSE-SEND.md](../03-architecture/FUSE-SEND.md)):
- The sender's Fuselane makes an encrypted torrent of the file and gives a link. The key sits in the link's `#fragment`, so it never reaches any server.
- The receiver's Fuselane finds the sender through the public BitTorrent DHT (and public trackers as a fallback) and downloads directly.
- Both sides use every network they have, so the transfer is bonded at both ends.
- No server stores or relays anything. Fuselane's cost is $0 at any number of users.

## Consequences
- The sender must stay online until the receiver has the file (a call, not a voicemail). The UI says so plainly and shows when the file has fully arrived.
- The receiver needs Fuselane. Receiving in a plain browser would need WebRTC on the sender side, which librqbit lacks; that is a later idea, not a promise.
- Two peers both behind strict NAT (CGNAT, many phone tethers) may fail to connect, because there is no paid relay. UPnP and uTP help; spike S7 measures how often it works. When it can't connect, the app says why and what to try. It never shows a silent spinner.
- Encryption is mandatory, because DHT announces are public: anyone can see that *some* info-hash is being shared, but never the name or contents.
- Same-network sending (AirDrop-style) and bring-your-own-cloud (the user's own Dropbox or S3, at their cost) remain possible later additions; neither costs Fuselane anything.
