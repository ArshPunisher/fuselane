# Market Research: Multi-Network Bonded Download/Upload App

Research date: 2026-10-08. Prepared for a cross-platform app (desktop first: macOS, Windows, Linux; Android later) that bonds Wi-Fi, a tethered phone and Ethernet for faster downloads (HTTP range requests and torrents) and faster uploads (parallel multipart uploads to S3/R2 with share links), plus a browser extension that captures downloads.

**How to read the sources.** "Verified" means I fetched the page during this research. Where only third-party or secondary sources existed, or sources disagreed, the text says so. Items marked *(background knowledge)* come from general product knowledge and were not re-checked; confirm them before using them in marketing.

---

## 1. Competitors

### 1.1 Download managers

| Product | Platforms | Key features | Pricing | What users complain about |
|---|---|---|---|---|
| **IDM (Internet Download Manager)** | Windows only | Browser capture for all major browsers, video grabber panel, segmented download with dynamic segmentation, scheduler, categories, queue, site grabber, speed limiter *(background knowledge)* | Verified from the official buy page (INR): 1 year on 1 PC ₹1,140; **lifetime on 1 PC ₹2,390**; multi-PC ₹950 per year or ₹1,910 lifetime per PC; 30-day trial ([buy page](https://secure.internetdownloadmanager.com/buy_idm.html)) | **No macOS or Linux**, and Wine doesn't work well ([TechEnclave](https://techenclave.com/t/idm-alternative-suggestion-needed/203384)); "fake serial" lockouts and widespread cracked copies ([IDM FAQ](https://www.internetdownloadmanager.com/register/new_faq/register7.html)); the YouTube grabber breaks after site updates ([Aimersoft](https://www.aimersoft.com/youtube-tips/idm-not-working-on-youtube.html), vendor source); dated UI; no torrents |
| **Free Download Manager (FDM)** | Windows, macOS, Linux, Android | Segmented downloads, BitTorrent, browser extension (Chrome, Firefox, Edge, Safari, Brave), video download incl. 5K/8K, scheduler, traffic control, HTML spider, remote control over the internet ([Neowin changelogs](https://www.neowin.net/software/free-download-manager-62616177/)) | Free (freeware, closed source today) | **The 2020–22 Linux supply-chain attack** spread a backdoored .deb from its site ([Kaspersky/Securelist](https://securelist.com/backdoored-free-download-manager-linux-malware/110465/), [THN](https://thehackernews.com/2023/09/free-download-manager-site-compromised.html)); seen as slower than IDM in some cases |
| **Motrix** | Windows, macOS, Linux (Electron and aria2) | HTTP, FTP, BitTorrent, magnet, up to 64 threads, tray, dark mode | Free (MIT) | **Effectively abandoned**: latest release v1.8.19 on 2023-05-03 despite about 56k stars ([GitHub](https://github.com/agalwood/Motrix)); forks such as Motrix Next exist ([Motrix Next](https://github.com/AnInsomniacy/motrix-next)) |
| **Gopeed** | Windows, macOS, Linux, **Android, iOS**, Web, Docker, CLI (Go and Flutter) | HTTP, BitTorrent, magnet, ed2k; seeding, DHT, PEX, uTP; REST API; **JavaScript extension system** (e.g., video sites); browser extension *(background knowledge)* ([docs](https://gopeed.com/docs/)) | Free (GPL-3.0); about 26.7k stars; v1.9.3 released 2026-03 | Smaller extension ecosystem; mobile versions limited by OS background rules *(background knowledge)* |
| **aria2** | CLI on all platforms | Multi-protocol, **multi-source and mirror** downloads, Metalink, checksums, JSON-RPC and XML-RPC remote control | Free (GPL-2.0) | **Stagnant**: last release 1.37.0 in 2023-11 ([GitHub](https://github.com/aria2/aria2)); marked unmaintained by some distros ([Crux](https://git.crux.nu/ports/contrib/commit/bea09deea41de0928a343254735581cdc18cdb62)); needs a GUI wrapper |
| **Surge** | Terminal UI on macOS, Linux and Windows (Go) | Up to 32 connections, **multi-mirror with failover**, sequential "streaming" mode, browser extension (Chrome, Edge, Brave, Firefox), local HTTP API with a single background engine ([GitHub](https://github.com/SurgeDM/Surge), [LinuxLinks](https://www.linuxlinks.com/surge-terminal-based-download-manager/)) | Free (MIT); about 3.6k stars; v0.12.2 | Terminal only, so it's niche; young |
| **JDownloader 2** | Java on all platforms | One-click-hoster plugins (1,000+ sites), link grabber, CAPTCHA handling, auto-extract, reconnect scripts, **MyJDownloader remote control** *(background knowledge)* | Free (ads and bundles in the installer) | Adware in the installer (2012, 2014), heavy RAM use ([Wikipedia](https://en.wikipedia.org/wiki/JDownloader), [AlternativeTo](https://alternativeto.net/software/jdownloader/about)); **jdownloader.org was compromised in May 2026** and served malicious installers ([BornCity](https://borncity.com/blog/2026/05/10/warnung-die-webseite-jdownloader-ist-kompromittiert-worden/)) |
| **XDM (Xtreme Download Manager)** | Windows, macOS, Linux | Browser capture, **video grabber** for HLS and DASH, conversion, scheduler ([GitHub](https://github.com/subhra74/xdm)) | Free (GPL-2.0) | **Last tagged release 7.2.11 in 2020**, although the repo is still pushed to (8.x betas); browser extension breakage *(background knowledge)* |
| **qBittorrent** | Windows, macOS, Linux | Full BitTorrent: v1/v2, sequential download, RSS auto-download, search plugins, **Web UI remote control**, IP filtering, scheduler, network-interface binding *(background knowledge)* ([GitHub](https://github.com/qbittorrent/qBittorrent), v5.2.4 released 2026-09) | Free (GPL) | Binds to only **one** interface (no bonding); dated UI *(background knowledge)* |

### 1.2 Connection bonding

| Product | Platforms | Model | Pricing | Complaints |
|---|---|---|---|---|
| **Speedify** | Windows, macOS, Linux, iOS, Android, routers | VPN-style **packet-level bonding** through Speedify servers. Works system-wide for every app, including single TCP streams and uploads. Streaming and video-call prioritization. ([pricing](https://speedify.com/pricing/)) | Verified: $14.99/month, $89.99/year, or $179.99 for 3 years (Individual); Families and Teams tiers; router plans $10.75–$45/month by data; dedicated or self-hosted servers $40–$120/month; 7-day trial. Reviews mention a 2 GB/month free tier ([Security.org](https://www.security.org/vpn/speedify-vpn/)). | Expensive ([Security.org](https://www.security.org/vpn/speedify-vpn/)); **latency and dropped frames, overloaded shared servers, far-away servers** ([vMix forum](https://forums.vmix.com/posts/m62071-Bonding-solution---Speedify-or-something-else), [Speedify blog](https://speedify.com/blog/combining-internet-connections/fix-slow-bonding-speeds/)); logs IP addresses and weak VPN privacy ([Gizmodo](https://gizmodo.com/best-vpn/speedify), [TechRepublic](https://www.techrepublic.com/article/speedify-vpn-review/)); "combined speed" marketing seen as misleading ([TidBITS](https://tidbits.com/2021/08/12/speedify-bonds-multiple-internet-connections-for-speed-and-reliability/)) |
| **Connectify Dispatch** | Windows | **Per-socket load balancing** (not bonding). Only multi-connection apps such as torrents and download managers benefit. | Was about $40; **discontinued** and replaced by Speedify ("Dispatch Lite" survives in Connectify Hotspot) ([support](https://support.connectify.me/article/24-will-dispatch-accelerate-my-vpn-connection)) | Couldn't speed up VPNs or single streams; it's the same model as our download path |
| **MPTCP (Multipath TCP)** | iOS and macOS (client APIs), Linux 5.6+ kernel | Protocol-level multipath. **Both ends must support it.** | Free | Server adoption is tiny: about 0.4% of traffic, mostly Apple ([APNIC 2022](https://blog.apnic.net/2022/08/23/analyzing-mptcp-adoption-in-the-internet/)); Linux client support "less ideal" ([Cloudflare via Phoronix](https://www.phoronix.com/news/Cloudflare-MPTCP-Multi-Path-TCP)); useless toward arbitrary HTTP servers unless you run an MPTCP proxy |
| **OpenMPTCProuter** | Router OS (OpenWrt) on a Raspberry Pi or x86, plus your own VPS | True aggregation of up to 8 WANs via MPTCP to a VPS ([site](https://www.openmptcprouter.com/)) | Free (GPL-3.0); costs a VPS (from about $4/month) | Needs networking skills, VLANs and a VPS ([Korben](https://korben.info/en/openmptcprouter-aggregate-internet-connections-maximum-bandwidth.html)); **the VPS IP gets blocked by some sites** ([Lawrence Systems forum](https://forums.lawrencesystems.com/t/multi-wan-bandwidth-aggregation-options/10244)) |

### 1.3 File transfer and sharing

| Product | Platforms | Key features | Pricing and limits | Complaints |
|---|---|---|---|---|
| **WeTransfer** | Web, desktop, mobile | Links, email delivery, branding (paid) | Verified: **Free is 10 transfers or 3 GB per 30-day rolling window**; Starter 300 GB per 30 days; Ultimate unlimited with up to 1 TB per transfer ([plan limits](https://wetransfer.com/help-center/subscriptions/plan-limits)). Free links reportedly expire after 3 days ([Fast.io](https://fast.io/resources/wetransfer-file-size-limit/)). | The free tier was cut hard; reportedly 1.3/5 on Trustpilot (per a competitor: [TransferNow](https://www.transfernow.net/en/wetransfer/limits)); [ITdaily on the 10-per-month limit](https://itdaily.com/news/cloud/wetransfer-limit-10-transfers-month) |
| **Smash** | Web, apps | No size limit on paid plans; previews; branding | Verified: Free up to 2 GB "prioritized", **larger files are queued behind Pro**, 7-day links; Pro and Team plans have unlimited size and 30-day links ([pricing](https://fromsmash.com/pricing)) | The free large-file queue is slow |
| **Wormhole.app** | Web | **E2E encrypted** (AES-GCM in the browser); stored on servers up to 5 GB for 24 hours; beyond that it's peer-to-peer and the sender's tab must stay open ([FAQ](https://wormhole.app/faq)) | Free | Short 24-hour expiry; the sender must stay online for large files |
| **Send (Firefox Send forks)** | Self-hosted web, CLI (ffsend) | E2E encryption, expiry by download count or time ([timvisee/send](https://github.com/timvisee/send)) | Free; defaults of 1 GB (2.5 GB logged in) are configurable per instance | Instances vary; you have to trust the operator ([Privacy Guides](https://discuss.privacyguides.net/t/add-send-firefox-send-fork-to-the-file-sharing-and-sync-page/187)); the mirror was last pushed in 2025-07 |
| **croc** | CLI on all platforms | PAKE-secured relay transfer, resume, multiple files, self-hostable relay ([GitHub](https://github.com/schollz/croc), about 40k stars, v11.5.4) | Free (MIT) | CLI only; both parties must be online |
| **LocalSend** | Windows, macOS, Linux, Android, iOS | LAN-only AirDrop alternative, no internet needed ([GitHub](https://github.com/localsend/localsend), about 94k stars) | Free (Apache-2.0) | LAN only; no links for remote recipients |
| *Adjacent: MASV* | Web, desktop, Premiere Pro panel | "Accelerated" uploads for video professionals, growing-file uploads ([pricing](https://massive.io/pricing)) | 15 GB/month free, then **$0.25/GB** | Expensive per GB; shows that creators pay for upload speed |

### 1.4 Feature matrix

Legend: Y = yes, P = partial or by plugin, N = no, ? = unverified.

| Feature | IDM | FDM | Motrix | Gopeed | aria2 | Surge | JDL2 | XDM | qBit | Speedify | WeTransfer | Wormhole | croc |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Multi-network bonding (downloads)** | N | N | N | N | P¹ | N | N | N | N | Y (VPN) | N | N | N |
| **Multi-network bonding (uploads)** | N | N | N | N | N | N | N | N | N | Y (VPN) | N | N | N |
| Multi-connection segmented HTTP | Y | Y | Y | Y | Y | Y | Y | Y | n/a | n/a | n/a | n/a | n/a |
| BitTorrent and magnet | N | Y | Y | Y | Y | N | N | N | Y | n/a | N | N | N |
| **Browser capture extension** | Y | Y | N | Y? | N | Y | P | Y | N | n/a | n/a | n/a | n/a |
| **Video/stream grabbing (HLS/DASH)** | Y | Y | N | P (ext) | N | N | Y | Y | N | n/a | n/a | n/a | n/a |
| Scheduler (time-based start/stop) | Y | Y | N | ? | N | N | P | Y | Y | n/a | n/a | n/a | n/a |
| Queue | Y | Y | Y | Y | Y | Y | Y | Y | Y | n/a | n/a | n/a | n/a |
| **Checksum verify (MD5/SHA)** | N? | P | N | N | Y | N | Y | N | Y (pieces) | n/a | n/a | n/a | Y |
| **Mirrors / multi-source** | N | N | N | N | Y | Y | P | N | n/a | n/a | n/a | n/a | n/a |
| Categories and auto-folders by type | Y | Y | N | N | N | N | Y | Y | Y | n/a | n/a | n/a | n/a |
| **Remote control (web/mobile/API)** | N | Y | P (RPC) | Y (API) | Y (RPC) | Y (API) | Y | N | Y (WebUI) | P | n/a | n/a | n/a |
| Per-network speed and data caps | N | P | P | P | P | N | P | P | P | P | n/a | n/a | n/a |
| Proxy (HTTP/SOCKS) | Y | Y | Y | Y | Y | ? | Y | Y | Y | n/a | n/a | n/a | n/a |
| Share links for uploads | N | N | N | N | N | N | N | N | N | N | Y | Y | P (code) |
| E2E encryption | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Y (tunnel) | N | Y | Y |
| Resumable upload after network drop | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | P | N | Y |
| macOS / Windows / Linux | N/Y/N | Y/Y/Y | Y/Y/Y | Y/Y/Y | Y/Y/Y | Y/Y/Y | Y/Y/Y | Y/Y/Y | Y/Y/Y | Y/Y/Y | Y/Y/N | web | Y/Y/Y |
| Android | N | Y | N | Y | P | N | N | N | N | Y | Y | web | P |
| Signed installers | Y | Y | Y | Y | n/a | n/a | Y | ? | Y | Y | Y | n/a | n/a |
| Open source | N | N | Y | Y | Y | Y | P | Y | Y | N | N | N | Y |
| Price | ₹2,390 lifetime | Free | Free | Free | Free | Free | Free | Free | Free | $90/year | Free (3 GB) to paid | Free | Free |

¹ aria2 can bind to one interface per instance only *(background knowledge)*.

**Features users love that bonding tools lack:** browser capture, video and stream grabbing, a scheduler (start at night, shut down when done), checksum verification, multi-mirror sources, categories and auto-sorting, remote control (phone or web), proxy support, upload bonding, share links, Android, and signed builds.

---

## 2. User pain points

### 2.1 Creators and videographers with large uploads

- **Uploads are the bottleneck.** Home connections are usually asymmetric, and Dropbox "almost never uses anything more than a fraction of the bandwidth available to it… with large 4K video files this can be downright painful" ([No Film School](https://nofilmschool.com/2018/04/deliver-faster-new-frameio-transfer-app)).
- **Silent overnight failures.** People "uploaded large assets… overnight, only to discover one or more files has failed" ([ProVideo Coalition](https://www.provideocoalition.com/frame-io-debuts-watch-folders-a-high-speed-file-transfer-app/)). Frame.io's own advice for failed Dropbox publishes is to re-upload, send fewer files, and check the Wi-Fi ([Frame.io support](https://support.frame.io/en/articles/5822737-troubleshooting-failed-uploads-when-publishing-to-dropbox)).
- **Free tiers shrank.** WeTransfer Free now allows 3 GB or 10 transfers per 30 days ([WeTransfer](https://wetransfer.com/help-center/subscriptions/plan-limits)). Smash queues free files over 2 GB ([Smash](https://fromsmash.com/pricing)). Wormhole only stores files up to 5 GB for 24 hours ([Wormhole](https://wormhole.app/faq)).
- **Creators pay a premium for speed.** MASV charges $0.25/GB and sells "accelerated" uploads ([MASV](https://massive.io/pricing)), and gearspace users are "blown away by the speed" of MASV compared with Dropbox ([gearspace](https://gearspace.com/threads/video-file-transfer.1381196/)).
- **Clients find delivery confusing.** Dropbox "got to be very slow and somewhat confusing for clients" ([gearspace](https://gearspace.com/threads/video-file-transfer.1381196/)).
- **Live streaming.** Users of bonding tools ask for upload bonding for live streaming. Speedify users on vMix report dropped frames when bonding ([vMix forum](https://forums.vmix.com/posts/m62071-Bonding-solution---Speedify-or-something-else)). Live streaming is a hard, latency-sensitive problem, so treat it as a later phase.
- **Storage backend economics.** R2 has **no egress fee**; storage is about $0.015/GB-month and there is a free 10 GB tier (per a secondary source). Multipart limits: parts of 5 MiB to 5 GiB, at most 10,000 parts, and **every part except the last must be the same size** (stricter than S3). Incomplete uploads are aborted after 7 days ([R2 multipart docs](https://developers.cloudflare.com/r2/objects/multipart-objects), [R2 limits](https://developers.cloudflare.com/r2/platform/limits/), [Arrow issue](https://github.com/apache/arrow/issues/41506)). Design fixed-size parts and resume state with this in mind.

### 2.2 India: daily mobile data caps

- **The daily-pack model.** Jio and Airtel prepaid plans give 1.5 to 2 GB per day (for example, Jio ₹239 for 1.5 GB/day; Airtel ₹299 for 1.5 GB/day for 28 days; prices vary by date). After the quota, speed drops to **64 kbps** ([TelecomTalk](https://telecomtalk.info/?p=495382), [Voice&Data](https://www.voicendata.com/mobile-phones/jio-vs-airtel-comparing-15gb-daily-data-plans-10496254)). Unused daily data **expires at midnight**, which is a strong "use it or lose it" motive for scheduling big downloads before midnight.
- **Unlimited 5G is the key opportunity.** Jio offers unlimited 5G on plans of 2 GB/day or more and **allows hotspot sharing**, with no published cap. Airtel caps 5G at **300 GB per 30 days and reportedly blocks hotspot use**: tethered traffic draws from the daily 4G quota ([Bridge Chronicle](https://www.thebridgechronicle.com/tech/airtel-5g-new-rules-hotspot-blocked-300gb-limit), [TelecomTalk](https://telecomtalk.info/?p=1003525)). Regulators have pressed both operators on these terms ([Business Standard](https://www.business-standard.com/industry/news/trai-directs-jio-airtel-to-clarify-terms-on-unlimited-5g-data-offerings-123120500562_1.html)). A Jio 5G phone tethered alongside home broadband is *the* killer setup for Indian users.
- **Evidence from bonding-tool users:** a Jio SIM tethered to a Mac, Hinglish bug reports, and an Instagram-driven audience. IDM is priced in INR and is ubiquitous (often cracked) in India, so a free IDM replacement that also bonds has a ready market.
- **Needs this implies:**
  - "Spend exactly my remaining daily quota" caps that reset at midnight local time (existing bonding tools already have daily caps).
  - Show remaining quota; ideally read it from the phone on Android.
  - Pause or move work to other links when a link drops to the 64 kbps throttle (detect it automatically).
  - A scheduler for the window before the midnight reset.
  - Airtel-aware warnings that hotspot traffic may not count as unlimited 5G.

---

## 3. Gap and opportunity: top 15 features

Ranked by impact multiplied by how clearly the gap is validated.

1. **Bonded uploads with share links.** Parallel multipart upload to R2 or S3 striped across every network, resumable, giving a WeTransfer-style link with expiry, password, download count and a notification. No competitor bonds uploads except Speedify, and it does so through a paid VPN. Validated by  and the WeTransfer and Smash limits. Note R2's equal-part-size rule.
2. **Browser capture extension (Chrome, Edge, Firefox, Safari).** Intercept downloads, add a right-click "download with [app]", forward cookies, referrer and user agent so authenticated and IP-locked links work, and use rules for size, extension and excluded domains. This is the top IDM feature, and existing bonding tools lack it.
3. **Video and stream grabbing (HLS/DASH), with parallel segment fetch across networks.** HLS segments are ideal for multi-network fetching. This is IDM's, XDM's and FDM's most-loved feature. Be careful legally: exclude DRM content and respect site terms.
4. **Signed and notarized installers, plus auto-update.** Apple notarization, Windows code signing, winget, Homebrew and the MS Store, with reproducible and verifiable builds. This removes the number-one support burden of unsigned apps and earns trust after the FDM and JDownloader site compromises.
5. **Rock-solid per-interface routing on every OS.** Use `IP_UNICAST_IF` (Windows), `IP_BOUND_IF` (macOS) and `SO_BINDTODEVICE` (Linux). Add the Windows "keep Wi-Fi on with Ethernet" fix, filter virtual and VPN adapters, and detect links that share an upstream. Fixes ,  and .
6. **Guided "add a network" onboarding and a built-in speed test.** Detect the tethered phone, walk through Android USB tethering on macOS (bundle or guide a driver), test each link alone and combined, and explain when adding a link won't help. Requested in  and .
7. **Data-cap intelligence for mobile links.** Daily, weekly or monthly allowances (existing bonding tools have these), with a "use only what's left today" option, 64 kbps throttle detection with automatic failover, a "before midnight" preset, and carrier-aware hints (Airtel blocks hotspot use of unlimited 5G). Counts upload and download separately.
8. **Scheduler and power actions.** Start, stop and pause windows, per-network schedules (for example, use the phone only after 11pm), and shut down or sleep the computer when done. IDM, FDM, XDM and qBittorrent have this; bonding tools don't.
9. **Remote control and a mobile companion.** Phone or web UI (with an LAN or cloud relay) to add links, watch progress and share links, plus a REST/RPC API compatible with aria2 RPC so existing tools such as AriaNg work. FDM, JDownloader (MyJD), qBittorrent and Gopeed have this.
10. **Integrity: checksum verification and repair.** Paste or auto-detect MD5/SHA-256 (from `.sha256` files, Metalink or the Digest header), verify on completion, re-fetch only bad blocks, and use per-part hashes for uploads (S3 `x-amz-checksum`). Truncated files marked "100%" are a known failure in this kind of tool.
11. **Multi-mirror and multi-source downloads.** Fetch one file from several URLs or CDNs at once (aria2 and Surge do this), with Metalink support. This multiplies with multi-network: N networks × M mirrors also gets around per-IP server caps.
12. **Per-network and per-download proxy and VPN routing.** HTTP and SOCKS5 per link, for restricted networks, IPs blocked by a WAF, and IP-locked links. Pin a download to a single network when the link is IP-locked.
13. **Categories, auto-sorting and a smart library.** Sort by file type or source site into folders, with rules and tags, and auto-extract archives (from JDownloader). Speeds up daily use for IDM converts.
14. **Optional "boost everything" mode through a relay (later phase).** A local SOCKS or system proxy plus a self-hosted or managed relay (MPTCP or QUIC multipath) for per-app or system-wide aggregation, including single-stream uploads and live streaming. This answers  and  and competes with Speedify at lower cost. A "bring your own VPS" option avoids Speedify's shared-server and privacy complaints. Note that the VPS IP can be blocked by some sites.
15. **Android app as a first-class node.** Android can bond its own Wi-Fi and cellular at once. It should also act as a "network donor" for the desktop over USB, Wi-Fi Direct or a LAN relay, and as a remote control and share target. Requested in . Gopeed and FDM are the only download managers on Android, and neither bonds.

**Honourable mentions:**

- Disk-aware concurrency and write coalescing for HDDs.
- Manual stream override.
- BitTorrent v2 and uTP with per-interface binding.
- Sequential "stream while downloading" for media (Surge).
- Upload watch folders for creators (as Frame.io and MASV offer).
- End-to-end encrypted links (as Wormhole offers).
- Share-link branding and a download page for clients.
- An NLE export panel later (as MASV has for Premiere Pro).

### Positioning summary

> **"IDM + WeTransfer, on every network you have."** The free, open, signed, cross-platform download manager that also makes big uploads fast. It stripes one file across Wi-Fi, a Jio or Airtel 5G tether and Ethernet in both directions, without a paid VPN.

Existing bonding download tools cover "bonded downloads" but none of the daily-use download-manager features, and it has nothing for uploads. Speedify bonds everything, but costs $90 a year and adds latency. WeTransfer, Smash and MASV make sharing easy, but they are capped or priced per GB and limited by a single uplink.
