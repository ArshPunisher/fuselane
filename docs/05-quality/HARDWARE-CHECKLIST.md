# Real-hardware checklist

What can only be proved with real devices: a phone, a second computer, a Windows PC, a Linux PC. Each check says what to set up, what to do, and what you should see. Write the outcome in [HARDWARE-RESULTS.md](HARDWARE-RESULTS.md) (date, version, machine, numbers, anything odd). The matrix in [TESTING.md §5](TESTING.md) is the summary; tick a box there once a check here passes.

Use a published build (or a packaged build from the release workflow), not `cargo run`, so installers, signing and "Open with" are part of the test.

A good test file for downloads: `https://speed.cloudflare.com/__down?bytes=500000000` (500 MB, takes ranges). For a fixed checksum use a Linux ISO with its published SHA-256, for example Debian's netinst image.

## 1. Phone tethering (the core promise)

### 1.1 iPhone on a Mac
1. Turn on Personal Hotspot on the iPhone. Connect it with a USB cable. Trust the computer if asked.
2. Open Fuselane → Networks. **Expect:** "iPhone USB" appears within a few seconds, marked usable, with a phone icon. The welcome (if it's a fresh install) lists it too.
3. Start the 500 MB download. **Expect:** the Fuse Core shows both networks carrying parts; the finished screen shows each network's share. Write down the speed of each and the total.
4. Unplug the cable mid-download. **Expect:** the download keeps going on the other network, no error; plugging back in adds the phone again.

### 1.2 Android on a Mac
1. Without a driver, turn on USB tethering. **Expect:** no new network appears (macOS limitation); the welcome's "Phone or second network not showing up?" explains why.
2. Install TetherKit (or another driver), turn USB tethering on again. **Expect:** a new network appears and works as in 1.1.

### 1.3 Phone on Windows and on Linux
1. Android: turn on USB tethering. iPhone: Personal Hotspot over USB (Windows needs iTunes or the Apple Devices app for the driver; Linux needs `ipheth`, normally built in).
2. Repeat 1.1 steps 2–4.

### 1.4 Data allowance, hours and throttling on a real phone plan
1. Set a small daily allowance (say 200 MB) on the phone network. Start a large download. **Expect:** the phone stops at the allowance; the others carry on; the network shows "allowance used".
2. Set hours for the phone (now plus 2 minutes to now plus 5 minutes). **Expect:** the phone joins when the window opens and leaves when it closes; running downloads move over.
3. If the plan slows to a crawl after its daily quota (Jio/Airtel), start a download after the quota is used. **Expect:** the download notes that the phone was throttled and the other networks carry the rest.

## 2. Two networks without a phone

### 2.1 Wi‑Fi + Ethernet on Windows
1. Plug in Ethernet while on Wi‑Fi. **Expect:** both appear in Networks. If Wi‑Fi disconnects, the welcome's tip about "Minimize the number of simultaneous connections" fixes it; note whether it was needed.
2. Repeat 1.1 step 3.

### 2.2 Same router vs different providers
1. With Wi‑Fi and Ethernet on the same router, run Networks → Check. **Expect:** "Together" is not much faster than the fastest alone, and the verdict says one connection is likely the limit.
2. With two different providers, run the check again. **Expect:** "Together" is close to the sum.

## 3. Nearby with real devices

### 3.1 LocalSend on a phone
1. Install LocalSend on a phone on the same Wi‑Fi. Open Send → Nearby in Fuselane.
2. **Expect:** the phone appears on the radar. Send a photo from the phone: Fuselane asks (or accepts if trusted); the file lands in Downloads.
3. Send a file from Fuselane to the phone. **Expect:** the phone asks and receives it.
4. Send text from LocalSend. **Expect:** Fuselane shows it in Activity (copied straight away only for trusted Fuselane computers).

### 3.2 Phone without an app (browser page)
1. Send → Show a code to scan. Scan with the phone's camera.
2. **Expect:** the page opens over Wi‑Fi. Send a photo: it arrives. Offer a file from Fuselane: the phone saves it. Paste text on the phone: it lands on the computer's clipboard. Offer text: Copy works on the phone (iPhone Safari and Android Chrome).

### 3.3 Two Fuselane computers
1. Both on the same Wi‑Fi. **Expect:** each sees the other; the first send shows the same four check words on both; Trust works.
2. Copy text on one, Send text on the other side's card. **Expect:** it lands on the other clipboard.
3. Keep a folder in sync. Add and change files. **Expect:** they appear on the other computer within a minute; nothing is deleted there.
4. Pause a download, "Continue on another computer". **Expect:** it appears paused there and finishes byte-exact after Resume.

## 4. Fuse Send across the internet
1. Computer A at home, computer B on another network (a phone hotspot is enough). Share a 1 GB file on A.
2. Open the link on B (in the browser, then "Open in Fuselane"). **Expect:** it connects (over IPv6 or with UPnP on A's router), downloads, and the checksum matches. Write down whether IPv6 or UPnP made it work.
3. Restart A mid-transfer. **Expect:** the same link resumes after A is back.

## 5. Installers and "Open with"

### 5.1 Windows
1. Run the setup .exe. **Expect:** SmartScreen may warn until signing is approved; More info → Run anyway installs it.
2. Click a magnet link in the browser, double-click a .torrent file, open a `fuselane://` link. **Expect:** each opens Fuselane at the right place.
3. Update from the previous version through the app. **Expect:** downloads in progress resume after the update.

### 5.2 Linux (AppImage, then .deb or .rpm)
1. Install and start. **Expect:** the window opens; networks appear.
2. Repeat 5.1 step 2 (the desktop entry registers the magnet and .torrent types).

### 5.3 macOS
1. Install from the DMG on a Mac that has never run Fuselane. Follow the Open Anyway guide on fuselane.app. **Expect:** the guide matches what macOS shows.
2. `brew install --cask arshpunisher/tap/fuselane` on another Mac. **Expect:** it installs and opens.

## 6. Sleep, wake and network changes
1. Start a long download, close the lid for two minutes, open it. **Expect:** it carries on by itself within a few seconds; no restart needed.
2. Switch Wi‑Fi networks mid-download. **Expect:** the old network drops out, the new one joins, the file is byte-exact.
3. Join a Wi‑Fi with a sign-in page. **Expect:** that network shows "Needs a sign-in page" and isn't used until you sign in.

## 7. Remote control and the browser extension
1. Settings → Other apps → turn on remote control, allow the network, show the code. Scan it with a phone. **Expect:** the remote page lists downloads; add a link from the phone; it starts on the computer.
2. Point AriaNg (or an Aria2 browser extension) at the address with the secret. **Expect:** downloads list and add.
3. Install the browser extension from the Chrome Web Store. Download a large file in Chrome. **Expect:** it goes to Fuselane. On a page with a feed, the popup offers "Follow this site's feed".
