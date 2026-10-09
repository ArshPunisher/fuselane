# Nearby: send to devices on the same network

Status: building in beta.8 (STEPS B8.11, B8.12). Owner-approved design: the Figma file "Fuselane", frames "Send, Nearby", "Nearby, incoming file" and "Phone browser page".

## What it does

Computers and phones on the same network show up on the Send page by themselves. Pick one, pick a file, and it goes straight there: no link, no internet, no account. A new device shows four words on both screens to check before anything is sent; trusting it skips the question next time. A phone without Fuselane scans a QR code and uses a page in its browser.

## Protocol: LocalSend v2

Fuselane speaks the open [LocalSend](https://localsend.org) protocol (v2), so the LocalSend apps on Android, iOS, Windows, macOS and Linux see Fuselane and Fuselane sees them. We implement the protocol from its public description; no LocalSend code is used (ADR 0012).

- **Discovery:** JSON announcements on UDP multicast `224.0.0.167:53317` on every usable network (Wi-Fi, Ethernet; never a phone's USB tether, which has no other devices). A device that hears an announcement answers with `POST /api/localsend/v2/register` to the sender, so both sides list each other.
- **Transfer:** HTTPS on TCP 53317 (the next free port if taken), self-signed certificate. `POST /api/localsend/v2/prepare-upload` (who and what) → the receiver asks the person → `200 {sessionId, files: {id: token}}`, `403` declined, `409` busy. Then `POST /api/localsend/v2/upload?sessionId&fileId&token` with the bytes, and `POST /api/localsend/v2/cancel` to stop.
- **Fingerprint:** SHA-256 of the device's certificate (DER), hex. It is the device's identity.

## Security

- **Identity:** each Fuselane has one key pair and self-signed certificate, made on first use and kept in its data folder. Its fingerprint is what trust is about; names (aliases) are only labels and can collide.
- **Pinning:** when Fuselane sends to a device, the TLS connection must present a certificate whose fingerprint matches the one it announced. A different certificate is refused, so a device on the network can't sit in the middle.
- **Four words:** both Fuselanes derive the same four words from the two fingerprints (SHA-256 of the sorted pair, first four bytes, a 256-word list of short plain words). The receiver shows them in the accept dialog and the sender shows them while it waits. Different words mean someone is in between: decline. LocalSend apps don't show words; Fuselane says the device is unverified.
- **Trust:** accepting with "Trust this device" stores its fingerprint, name and date. A trusted device's files arrive without asking (into the downloads folder, never overwriting). Trusted devices are listed with Forget.
- **Who can see this computer:** *Trusted only* (default): no multicast announcements; Fuselane answers only announcements from trusted fingerprints, and refuses others' requests with 403. *Everyone, 10 minutes*: announces and accepts requests (asking first) from anyone, then falls back to trusted only. Nearby is off entirely until the person opens the Send page once.
- **Limits:** request bodies and JSON are size-capped, at most 1000 files per request, names are cleaned like downloads (no paths, no `..`, no hidden or reserved names), files land in the downloads folder under free names, and a transfer must match the size it announced. One incoming session at a time.

## Phone browser page (B8.12)

For phones without an app: Fuselane serves a small page over plain HTTP on the LAN at `http://<this computer>:<port>/p/<token>` (a QR code on the Send page). The random token in the link is the permission: only someone who saw the screen can open it, and it changes each time the page is shown. The page sends files to the computer (they land in the downloads folder) and lists files the person put up for the phone to save. It stops working when the Send page's sharing ends or Fuselane quits. It is plain HTTP because phones would warn about a self-signed certificate; it never leaves the local network.

## Not in this round

Sending folders, resuming an interrupted transfer, Bluetooth discovery, and LocalSend's "share via link" reverse mode for desktop apps.
