// Ported from apps/site/privacy/index.html by scripts/convert.py.
import type { Metadata } from 'next'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Privacy policy: Fuselane',
  },
  description:
    'Fuselane collects nothing: no accounts, no analytics, no tracking. What the app, the browser extension and this website send, and when.',
  alternates: {
    canonical: 'https://fuselane.app/privacy/',
  },
  robots: 'index, follow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/privacy/',
    title: 'Privacy policy: Fuselane',
    description:
      'Fuselane collects nothing: no accounts, no analytics, no tracking. What the app, the browser extension and this website send, and when.',
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
    title: 'Privacy policy: Fuselane',
    description:
      'Fuselane collects nothing: no accounts, no analytics, no tracking. What the app, the browser extension and this website send, and when.',
    images: ['https://fuselane.app/og.png'],
  },
}

const LD: object[] = [
  {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: [
      { '@type': 'ListItem', position: 1, name: 'Fuselane', item: 'https://fuselane.app/' },
      { '@type': 'ListItem', position: 2, name: 'Privacy', item: 'https://fuselane.app/privacy/' },
    ],
  },
]

export default function Page() {
  return (
    <>
      <JsonLd data={LD} />
      <main id="main" className="wrap">
        {' '}
        <section className="page-hero" aria-labelledby="privacy-title">
          {' '}
          <h1 id="privacy-title">Privacy policy</h1>{' '}
          <p>
            {' '}
            No accounts, no analytics, no tracking. Here is exactly what goes over the network, and{' '}
            when.{' '}
          </p>{' '}
        </section>{' '}
        <article className="doc legal">
          {' '}
          <p className="legal-date">Last updated 10 October 2026.</p>{' '}
          <section className="doc-group" aria-labelledby="p-short">
            {' '}
            <h2 id="p-short">The short version</h2>{' '}
            <p>
              {' '}
              Fuselane has no accounts, no analytics, no ads and no tracking, in the app, the
              browser extension or this website. Your downloads, settings and history stay on your
              computer. Nothing about you is collected, sold or shared.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="p-app">
            {' '}
            <h2 id="p-app">The app</h2>{' '}
            <p>
              {' '}
              Your download list and settings are kept in a file on your computer. Removing a
              download removes its entry. Proxy passwords are kept in your system&rsquo;s
              keychain.{' '}
            </p>{' '}
            <p>The app only goes online for these:</p>{' '}
            <ul>
              {' '}
              <li>
                {' '}
                <strong>Your downloads:</strong> it connects to the servers in the links you give
                it, over each network you allow.{' '}
              </li>{' '}
              <li>
                {' '}
                <strong>Sign-in page checks:</strong> about once a minute it asks its own page on{' '}
                GitHub Pages, through each network, whether a hotel or caf&eacute; sign-in page is
                in the way. GitHub sees your network&rsquo;s public IP address; nothing else is
                sent.{' '}
              </li>{' '}
              <li>
                {' '}
                <strong>Update checks:</strong> a request for a small signed file listing the newest{' '}
                version. It carries no identifier.{' '}
              </li>{' '}
              <li>
                {' '}
                <strong>Speedtest, only when you run it:</strong> test data goes to and from{' '}
                Cloudflare&rsquo;s free speed test through each network, and the speed server says{' '}
                which internet provider and city it sees. The results stay on your computer.{' '}
              </li>{' '}
              <li>
                {' '}
                <strong>Server lookups, only if you turn them on:</strong> Cloudflare and Google DNS{' '}
                see the names of the servers you download from, never the files.{' '}
              </li>{' '}
              <li>
                {' '}
                <strong>Fuse Send and Nearby, only when you use them:</strong> files go straight to{' '}
                the other device, encrypted. A Fuse Send link carries its own key after the{' '}
                <code>#</code>, which never reaches any server.{' '}
              </li>{' '}
            </ul>{' '}
            <p>
              {' '}
              <strong>Copy diagnostics</strong> in Settings builds a report for bug reports and
              shows it to you first. It leaves out links, file names and addresses, and only leaves
              your computer if you paste it somewhere.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="p-ext">
            {' '}
            <h2 id="p-ext">The browser extension</h2>{' '}
            <p>
              {' '}
              The extension only talks to the Fuselane app on your own computer. When a download{' '}
              starts, it passes the app the link, the file name, size and type, and the page it came{' '}
              from. When you open its toolbar button, it looks once at the tab you&rsquo;re on to
              list its videos, file links and feeds, and keeps nothing. It never reads other tabs or
              your history, and sends nothing to the internet.{' '}
            </p>{' '}
            <p>
              {' '}
              <strong>Signed-in downloads</strong> are off unless you turn them on. Then, for a{' '}
              download handed to the app, it passes that site&rsquo;s cookies to the app, which uses{' '}
              them only for that download, keeps them in memory only, and sends them only to that{' '}
              site.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="p-site">
            {' '}
            <h2 id="p-site">This website</h2>{' '}
            <p>
              {' '}
              fuselane.app has no cookies, no analytics and no trackers. It is served by Cloudflare{' '}
              Pages, which handles requests like any web host. To show the star count and file
              sizes, your browser asks GitHub&rsquo;s public API directly.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="p-kids">
            {' '}
            <h2 id="p-kids">Children, changes and questions</h2>{' '}
            <p>
              {' '}
              Fuselane collects nothing from anyone, children included. If this policy changes, the{' '}
              new version is published here and in the{' '}
              <a href="https://github.com/ArshPunisher/fuselane/blob/main/docs/PRIVACY.md">
                project&rsquo;s privacy document
              </a>
              , with the date at the top. Questions:{' '}
              <a href="mailto:hello@fuselane.app">hello@fuselane.app</a> or{' '}
              <a href="https://github.com/ArshPunisher/fuselane/issues">GitHub issues</a>.{' '}
            </p>{' '}
          </section>{' '}
        </article>{' '}
      </main>
      <PageScript />
    </>
  )
}
