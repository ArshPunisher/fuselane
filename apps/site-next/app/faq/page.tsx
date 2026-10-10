// Ported from apps/site/faq/index.html by scripts/convert.py.
import type { Metadata } from 'next'
import Link from 'next/link'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Fuselane questions, answered',
  },
  description:
    'How Fuselane combines Wi-Fi, Ethernet and a tethered phone, which connections work, why your computer may warn the first time, and what it does with your data.',
  alternates: {
    canonical: 'https://fuselane.app/faq/',
  },
  robots: 'index, follow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/faq/',
    title: 'Fuselane questions, answered',
    description:
      'How Fuselane combines Wi-Fi, Ethernet and a tethered phone, which connections work, why your computer may warn the first time, and what it does with your data.',
    images: [
      {
        url: 'https://fuselane.app/og.png',
        width: 1200,
        height: 630,
        alt: 'The Fuselane logo, the headline Every network. One fast download., and a ring of file parts coloured by the Wi-Fi, Ethernet and iPhone USB that fetched them.',
      },
    ],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'Fuselane questions, answered',
    description:
      'How Fuselane combines Wi-Fi, Ethernet and a tethered phone, which connections work, why your computer may warn the first time, and what it does with your data.',
    images: ['https://fuselane.app/og.png'],
  },
}

const LD: object[] = [
  {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: [
      { '@type': 'ListItem', position: 1, name: 'Fuselane', item: 'https://fuselane.app/' },
      { '@type': 'ListItem', position: 2, name: 'Questions', item: 'https://fuselane.app/faq/' },
    ],
  },
  {
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: [
      {
        '@type': 'Question',
        name: 'How does Fuselane combine my connections?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'It splits a download into parts and fetches them over every network you have at the same time, each part over whichever network is free. Faster networks simply take more parts. When they are all in, the parts are checked and joined into one file.',
        },
      },
      {
        '@type': 'Question',
        name: 'Which connections can I combine?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Any your computer shows as a network: Wi-Fi, Ethernet, a phone tethered over USB, a second Wi-Fi or USB adapter. They help most when they come from different internet lines (your home Wi-Fi plus your phone’s mobile data, say). Two cables to the same router share one line, so they won’t be faster together.',
        },
      },
      {
        '@type': 'Question',
        name: 'Does it speed up my whole computer?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'No. It speeds up what you download with Fuselane: links you add, torrents, and big downloads the browser extension hands over. Everything else uses your normal connection.',
        },
      },
      {
        '@type': 'Question',
        name: 'Will it eat my phone’s data?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Only as much as you let it. Set a speed limit or a monthly allowance for each network (for example, 5 GB a month on the phone), turn on Slow mode during calls, or let it download only at night. When a network reaches its allowance, Fuselane stops using it until the reset day.',
        },
      },
      {
        '@type': 'Question',
        name: 'Does it work with torrents?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Yes: magnet links and .torrent files, with a choice of files before anything downloads. Peers are spread across your networks, and sharing after a download is off unless you turn it on.',
        },
      },
      {
        '@type': 'Question',
        name: 'Do servers mind?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Fuselane asks for parts the standard way (byte ranges), as browsers do when they resume. If a server limits connections, Fuselane backs off; if it doesn’t allow parts at all, the file comes over one network.',
        },
      },
      {
        '@type': 'Question',
        name: 'Can it download videos from YouTube and other sites?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Yes, if you install the free yt-dlp tool (and ffmpeg for HD). Paste the video’s page and choose Get the video from this page; pick a quality and Fuselane downloads it over every network. yt-dlp only finds the links; Fuselane never bundles it. Download only what you have the right to.',
        },
      },
      {
        '@type': 'Question',
        name: 'What does the network check send, and to whom?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'It downloads a 25 MB test file per network from Cloudflare’s public speed test, times short connections to 1.1.1.1 and one name lookup. Nothing is uploaded and no results leave your computer unless you send the report yourself.',
        },
      },
      {
        '@type': 'Question',
        name: 'How does Fuse Send work?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'In Fuselane, open Send and pick a file. You get a link; send it any way you like. When the other person opens it, their Fuselane fetches the file straight from yours, encrypted on the way, and checks it matches before saving it. Nothing is uploaded anywhere, so there is no size limit and nothing expires.',
        },
      },
      {
        '@type': 'Question',
        name: 'Do I need to keep Fuselane open while sending?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Yes: the file comes from your computer, so keep Fuselane open (it can sit in the tray) until it arrives. If you close it or restart, the share comes back by itself and the same link keeps working. You can also have it stop after one full copy is sent.',
        },
      },
      {
        '@type': 'Question',
        name: 'The receiver can’t reach me. Why?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'The file comes straight from your computer, so the receiver’s Fuselane has to be able to connect to it. On the same Wi-Fi or office network that just works. Across the internet it works when your router lets it in: over IPv6, which most internet providers now give out, or over IPv4 when UPnP is on in the router’s settings. Fuselane has no server in the middle to relay files, which is why it’s free and has no size limit.',
        },
      },
      {
        '@type': 'Question',
        name: 'Who can open a Fuse Send link?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Anyone with the whole link, so share it like a password. The key is in the part after #, which browsers never send to a server; the share page reads it on the receiver’s own device and hands it to Fuselane. Others on the network see only a random identifier, never the file’s name or contents.',
        },
      },
      {
        '@type': 'Question',
        name: 'Why does my computer warn me the first time I open it?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Fuselane is free and doesn’t pay for Apple’s notarization, and its Windows signing through the SignPath Foundation is still being set up. So macOS asks you to choose Open Anyway once (in System Settings, Privacy & Security), and Windows may say More info, then Run anyway. The Mac install command and Homebrew skip the step. See Download for each system.',
        },
      },
      {
        '@type': 'Question',
        name: 'Is it stable?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'It is in beta. Downloads survive a dropped network, sleep, quitting and even a crash, and pick up where they stopped. Updates arrive inside the app. If something goes wrong, tell us; Settings has a Copy diagnostics button that never includes links or file names.',
        },
      },
      {
        '@type': 'Question',
        name: 'Fuselane says my download list is from a newer Fuselane',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'A newer copy of Fuselane used your list, and then an older copy was opened, for example an old one still in your Downloads folder. Choose Update now in that window: Fuselane updates itself and opens with your downloads as they were. If it can’t find the update, get the newest version from Download and install it over the old one. Nothing is lost either way. Copies older than 0.1.0-beta.11 can’t show this window and seem to do nothing when opened; install the newest version once and it won’t happen again.',
        },
      },
      {
        '@type': 'Question',
        name: 'My Android phone over USB doesn’t show up on my Mac',
        acceptedAnswer: {
          '@type': 'Answer',
          text: "Fuselane uses the networks macOS sees. iPhones work over USB without anything extra. Some Android phones' USB tethering needs a driver macOS doesn’t include; if the phone doesn’t appear under Network in System Settings, Fuselane can’t see it either. On Windows and Linux, Android USB tethering usually just works.",
        },
      },
      {
        '@type': 'Question',
        name: 'What does Fuselane do with my data?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Nothing leaves your computer except the downloads themselves and a check for signed updates. No account, no analytics, no crash reports. Read the privacy policy.',
        },
      },
      {
        '@type': 'Question',
        name: 'Is it really free?',
        acceptedAnswer: {
          '@type': 'Answer',
          text: 'Yes, with no paid plan and nothing to sign up for. It is open source under the Apache-2.0 licence, so anyone can read and check the code.',
        },
      },
    ],
  },
]

export default function Page() {
  return (
    <>
      <JsonLd data={LD} />
      <main id="main" className="wrap">
        {' '}
        <section className="page-hero" aria-labelledby="faq-title">
          {' '}
          <h1 id="faq-title">Questions, answered</h1>{' '}
          <p>
            {' '}
            Something missing?{' '}
            <a href="https://github.com/ArshPunisher/fuselane/issues">Ask on GitHub</a>.{' '}
          </p>{' '}
        </section>{' '}
        <div className="doc-layout">
          {' '}
          <nav className="toc" aria-label="Topics" data-toc="">
            {' '}
            <ul>
              {' '}
              <li>
                {' '}
                <a href="#basics">
                  How it works<span className="num">8</span>
                </a>{' '}
              </li>{' '}
              <li>
                {' '}
                <a href="#send">
                  Fuse Send<span className="num">4</span>
                </a>{' '}
              </li>{' '}
              <li>
                {' '}
                <a href="#trust">
                  Installing and trust<span className="num">5</span>
                </a>{' '}
              </li>{' '}
            </ul>{' '}
          </nav>{' '}
          <div className="doc-groups">
            {' '}
            <section className="doc-group" id="basics" aria-labelledby="basics-title">
              {' '}
              <h2 id="basics-title">How it works</h2>{' '}
              <div className="qa">
                {' '}
                <details open>
                  {' '}
                  <summary>How does Fuselane combine my connections?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      It splits a download into parts and fetches them over every network you have
                      at the same time, each part over whichever network is free. Faster networks
                      simply take more parts. When they are all in, the parts are checked and joined
                      into one file.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Which connections can I combine?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Any your computer shows as a network: Wi-Fi, Ethernet, a phone tethered over{' '}
                      USB, a second Wi-Fi or USB adapter. They help most when they come from
                      different internet lines (your home Wi-Fi plus your phone&rsquo;s mobile data,
                      say). Two cables to the same router share one line, so they won&rsquo;t be
                      faster together.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Does it speed up my whole computer?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      No. It speeds up what you download with Fuselane: links you add, torrents, and{' '}
                      big downloads the browser extension hands over. Everything else uses your
                      normal connection.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Will it eat my phone&rsquo;s data?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Only as much as you let it. Set a speed limit or a monthly allowance for each{' '}
                      network (for example, 5&nbsp;GB a month on the phone), turn on Slow mode
                      during calls, or let it download only at night. When a network reaches its
                      allowance, Fuselane stops using it until the reset day.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Does it work with torrents?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Yes: magnet links and .torrent files, with a choice of files before anything{' '}
                      downloads. Peers are spread across your networks, and sharing after a download{' '}
                      is off unless you turn it on.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Do servers mind?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Fuselane asks for parts the standard way (byte ranges), as browsers do when
                      they resume. If a server limits connections, Fuselane backs off; if it
                      doesn&rsquo;t allow parts at all, the file comes over one network.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Can it download videos from YouTube and other sites?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Yes, if you install the free yt-dlp tool (and ffmpeg for HD). Paste the{' '}
                      video&rsquo;s page and choose <strong>Get the video from this page</strong>;
                      pick a quality and Fuselane downloads it over every network. yt-dlp only finds
                      the links; Fuselane never bundles it. Download only what you have the right
                      to.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>What does the network check send, and to whom?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      It downloads a 25&nbsp;MB test file per network from Cloudflare&rsquo;s public{' '}
                      speed test, times short connections to 1.1.1.1 and one name lookup. Nothing is{' '}
                      uploaded and no results leave your computer unless you send the report
                      yourself.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
              </div>{' '}
            </section>{' '}
            <section className="doc-group" id="send" aria-labelledby="send-title">
              {' '}
              <h2 id="send-title">Fuse Send</h2>{' '}
              <div className="qa">
                {' '}
                <details>
                  {' '}
                  <summary>How does Fuse Send work?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      In Fuselane, open <em>Send</em> and pick a file. You get a link; send it any
                      way you like. When the other person opens it, their Fuselane fetches the file{' '}
                      straight from yours, encrypted on the way, and checks it matches before saving{' '}
                      it. Nothing is uploaded anywhere, so there is no size limit and nothing
                      expires.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Do I need to keep Fuselane open while sending?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Yes: the file comes from your computer, so keep Fuselane open (it can sit in
                      the tray) until it arrives. If you close it or restart, the share comes back
                      by itself and the same link keeps working. You can also have it stop after one
                      full copy is sent.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>The receiver can&rsquo;t reach me. Why?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      The file comes straight from your computer, so the receiver&rsquo;s Fuselane
                      has to be able to connect to it. On the same Wi-Fi or office network that just{' '}
                      works. Across the internet it works when your router lets it in: over IPv6,{' '}
                      which most internet providers now give out, or over IPv4 when UPnP is on in
                      the router&rsquo;s settings. Fuselane has no server in the middle to relay
                      files, which is why it&rsquo;s free and has no size limit.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Who can open a Fuse Send link?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Anyone with the whole link, so share it like a password. The key is in the
                      part after <code>#</code>, which browsers never send to a server; the share
                      page reads it on the receiver&rsquo;s own device and hands it to Fuselane.
                      Others on the network see only a random identifier, never the file&rsquo;s
                      name or contents.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
              </div>{' '}
            </section>{' '}
            <section className="doc-group" id="trust" aria-labelledby="trust-title">
              {' '}
              <h2 id="trust-title">Installing and trust</h2>{' '}
              <div className="qa">
                {' '}
                <details>
                  {' '}
                  <summary>Why does my computer warn me the first time I open it?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Fuselane is free and doesn&rsquo;t pay for Apple&rsquo;s notarization, and its{' '}
                      Windows signing through the SignPath Foundation is still being set up. So
                      macOS asks you to choose Open Anyway once (in System Settings, Privacy &amp;{' '}
                      Security), and Windows may say More info, then Run anyway. The Mac install{' '}
                      command and Homebrew skip the step. See{' '}
                      <Link href="/download/">Download</Link> for each system.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Is it stable?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      It is in beta. Downloads survive a dropped network, sleep, quitting and even a{' '}
                      crash, and pick up where they stopped. Updates arrive inside the app. If{' '}
                      something goes wrong,{' '}
                      <a href="https://github.com/ArshPunisher/fuselane/issues">tell us</a>;
                      Settings has a Copy diagnostics button that never includes links or file
                      names.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Fuselane says my download list is from a newer Fuselane</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      A newer copy of Fuselane used your list, and then an older copy was opened,
                      for example an old one still in your Downloads folder. Choose Update now in
                      that window: Fuselane updates itself and opens with your downloads as they
                      were. If it can&rsquo;t find the update, get the newest version from{' '}
                      <Link href="/download/">Download</Link> and install it over the old one.
                      Nothing is lost either way. Copies older than 0.1.0-beta.11 can&rsquo;t show
                      this window and seem to do nothing when opened; install the newest version
                      once and it won&rsquo;t happen again.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>My Android phone over USB doesn&rsquo;t show up on my Mac</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Fuselane uses the networks macOS sees. iPhones work over USB without anything{' '}
                      extra. Some Android phones' USB tethering needs a driver macOS doesn&rsquo;t{' '}
                      include; if the phone doesn&rsquo;t appear under Network in System Settings,{' '}
                      Fuselane can&rsquo;t see it either. On Windows and Linux, Android USB
                      tethering usually just works.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>What does Fuselane do with my data?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Nothing leaves your computer except the downloads themselves and a check for{' '}
                      signed updates. No account, no analytics, no crash reports. Read the{' '}
                      <a href="https://github.com/ArshPunisher/fuselane/blob/main/docs/PRIVACY.md">
                        privacy policy
                      </a>
                      .{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
                <details>
                  {' '}
                  <summary>Is it really free?</summary>{' '}
                  <div className="answer">
                    {' '}
                    <p>
                      {' '}
                      Yes, with no paid plan and nothing to sign up for. It is open source under the{' '}
                      Apache-2.0 licence, so anyone can read and check the code.{' '}
                    </p>{' '}
                  </div>{' '}
                </details>{' '}
              </div>{' '}
            </section>{' '}
          </div>{' '}
        </div>{' '}
      </main>
      <PageScript />
    </>
  )
}
