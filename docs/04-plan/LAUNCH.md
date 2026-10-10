# Launch plan (STEPS 9.4)

How Fuselane 1.0 meets people. Everything here is free to do; nothing needs a paid tool or ad spend (ADR 0009). The owner posts from their own accounts; drafts live here so they can be reused.

## Before launch day

- **The build people get is the build we tested.** 1.0 only after the real-hardware checklist passes ([HARDWARE-CHECKLIST.md](../05-quality/HARDWARE-CHECKLIST.md)), two weeks of clean 24 h soaks (9.1), and signed Windows installers if SignPath has approved (4.3).
- **Install paths ready:** DMG and install script (macOS), Homebrew cask, winget manifest merged (9.3), Flathub submitted (4.4), AppImage/deb/rpm on the release page, the Chrome extension published.
- **Site:** fuselane.app with the guide, FAQ and download page; Google Search Console verified and the sitemap submitted.
- **Assets** (made once, used everywhere):
  - A 30–45 s screen recording: plug in a phone, start a big download, the Fuse Core fills from two networks, the finished screen shows each network's share. No music needed; captions on.
  - A 60–90 s walkthrough: download, network check, Nearby to a phone, Fuse Send link.
  - Four screenshots (light and dark) at 1280×800 and one square image for social posts.
  - One real number with its setup, for example "246 MB in 18.6 s over Wi‑Fi + Ethernet". Never round up or invent.
- **Support ready:** GitHub issue templates, Discussions turned on, Settings → Report a problem tested, the FAQ honest about limits (a server that refuses ranges uses one network; macOS isn't notarized; DHT goes over the default route).

## The message

One sentence: **Fuselane downloads one file over every network your computer has (Wi‑Fi, Ethernet, your phone) at once. Free and open source, no account, no cloud.**

Three proof points, in this order: it's faster when you have two connections (show the number); it's free and private (no account, nothing leaves your computer except the download); it replaces several apps (video from pages, network check, sending to phones, feeds).

Who it's for first: people with a broadband line plus a phone plan (India: Jio/Airtel with a daily quota), people downloading large files (games, ISOs, datasets), and people who like open-source tools.

## Where, in order

1. **Show HN** (weekday, about 8–9 am US Eastern). Title: "Show HN: Fuselane - download one file over Wi‑Fi, Ethernet and your phone at once". First comment from the owner: why it exists, how it works (byte ranges per network, sockets bound per interface, joined and verified), what it can't do, that it's Apache-2.0. Answer every question for the first few hours.
2. **Reddit**, one post per community, spaced over a week, each written for that community: r/DataHoarder (big files, checksums), r/selfhosted and r/opensource (no account, no cloud, aria2-compatible remote), r/macapps and r/software (the app itself), r/india and r/developersIndia (broadband plus Jio/Airtel quota, hours per network, data allowances). Follow each subreddit's self-promotion rules; reply in the comments.
3. **Product Hunt** (a Tuesday–Thursday, 00:01 Pacific). Gallery: the short video first, then the screenshots. Maker comment = the HN first comment, shortened.
4. **Video:** the 30–45 s clip as a YouTube Short and an Instagram Reel; the longer walkthrough on YouTube with chapters. Reach out (free, by email) to Indian tech YouTubers who review download tools and phone-plan hacks.
5. **Communities:** Indian tech Discord/Telegram groups, Hacker News "Who's using what" threads later, LocalSend and AriaNg communities (Fuselane speaks both), the yt-dlp community (Fuselane uses the user's own yt-dlp).
6. **Directories** (free listings): AlternativeTo (as an alternative to IDM, Free Download Manager, Motrix, JDownloader), Homebrew, winget, Flathub, Softpedia, MacUpdate.

## On the day

- Watch GitHub issues and Discussions; label and answer within hours. Ship fixes as 1.0.x quickly; the updater delivers them.
- Keep a running list of repeated questions; add them to the FAQ the same day.
- Note numbers people report (networks and speeds) in a Discussions thread; they're better proof than ours.

## After

- Week 1: a short "what we learned" post with real numbers (downloads, issues fixed). Thank contributors.
- Month 1: a release with the most-asked feature; tell the same channels only if it's a real improvement.
- Keep the promise: no account, no tracking, free. Say no to anything that would cost money per user (ADR 0009).
