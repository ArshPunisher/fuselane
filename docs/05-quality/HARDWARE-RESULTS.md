# Real-hardware results

One entry per check from [HARDWARE-CHECKLIST.md](HARDWARE-CHECKLIST.md): date, Fuselane version, machine and networks, what happened, numbers. Newest on top.

## Template

```
### YYYY-MM-DD, vX.Y.Z, <machine> (<OS version>)
- Check: <section number and name>
- Networks: <e.g. Wi-Fi (home ISP) + iPhone USB (Jio 5G)>
- Result: pass / fail
- Numbers: <speed per network, together, file size, time>
- Notes: <anything odd, screenshots>
```

## Results so far

### 2026-10-10, 0.1.0-beta.9 development build, MacBook (macOS, Apple silicon)
- Check: 2.2 (network check), 1.1-style bonded download without a phone
- Networks: Ethernet + Wi‑Fi on the same Mac
- Result: pass
- Numbers: network check measured Ethernet 113 Mbps and Wi‑Fi 117 Mbps; a 246 MB 1080p video (yt-dlp) over Wi‑Fi + Ethernet in 18.6 s
- Notes: found and fixed a DNS lookup that waited 3 s on a network that drops the second of two queries

### 2026-10-08, 0.1.0-beta.1 to beta.2, MacBook (macOS)
- Check: 5.3 update path (TESTING.md 4.7)
- Result: pass
- Notes: a real update from beta.1 to beta.2; a 31% download resumed in beta.2, byte-exact

### P3 and P5 (before beta.1), MacBook (macOS)
- Check: bonded download and torrent over Ethernet (en0) + Wi‑Fi (en1)
- Result: pass
- Numbers: a 300 MB download at about 50 MB/s, byte-exact, Ethernet 73% and Wi‑Fi 27%; Debian 13.7 netinst torrent at 16 MB/s, SHA-256 matched, credit 62% / 38%

Still open: everything with a phone (section 1), Windows and Linux (sections 2.1, 5.1, 5.2), Nearby with the LocalSend apps and a real phone scan (section 3), Fuse Send across the internet (section 4), sleep and wake (section 6).
