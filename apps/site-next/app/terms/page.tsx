// Ported from the original plain-HTML site.
import type { Metadata } from 'next'
import Link from 'next/link'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Terms of use: Fuselane',
  },
  description:
    'Fuselane is free software under Apache-2.0, provided as is. What that means, and your responsibility for what you download.',
  alternates: {
    canonical: 'https://fuselane.app/terms/',
  },
  robots: 'index, follow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/terms/',
    title: 'Terms of use: Fuselane',
    description:
      'Fuselane is free software under Apache-2.0, provided as is. What that means, and your responsibility for what you download.',
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
    title: 'Terms of use: Fuselane',
    description:
      'Fuselane is free software under Apache-2.0, provided as is. What that means, and your responsibility for what you download.',
    images: ['https://fuselane.app/og.png'],
  },
}

const LD: object[] = [
  {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: [
      { '@type': 'ListItem', position: 1, name: 'Fuselane', item: 'https://fuselane.app/' },
      { '@type': 'ListItem', position: 2, name: 'Terms', item: 'https://fuselane.app/terms/' },
    ],
  },
]

export default function Page() {
  return (
    <>
      <JsonLd data={LD} />
      <main id="main" className="wrap">
        {' '}
        <section className="page-hero" aria-labelledby="terms-title">
          {' '}
          <h1 id="terms-title">Terms of use</h1>{' '}
          <p>
            Fuselane is free software. These are the few things worth knowing before you use it.
          </p>{' '}
        </section>{' '}
        <article className="doc legal">
          {' '}
          <p className="legal-date">Last updated 10 October 2026.</p>{' '}
          <section className="doc-group" aria-labelledby="t-licence">
            {' '}
            <h2 id="t-licence">Free software</h2>{' '}
            <p>
              {' '}
              Fuselane (the app, the command-line tool and the browser extension) is free and open{' '}
              source under the{' '}
              <a href="https://github.com/ArshPunisher/fuselane/blob/main/LICENSE">
                Apache License 2.0
              </a>
              . You may use it for anything, at home or at work, copy it, change it and share it, as{' '}
              that licence says. There is no price, no account and nothing to sign up for.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="t-asis">
            {' '}
            <h2 id="t-asis">No warranty</h2>{' '}
            <p>
              {' '}
              Fuselane is provided &ldquo;as is&rdquo;, without warranties or conditions of any
              kind, as set out in sections 7 and 8 of the licence. It is made carefully and checks
              every file it downloads, but no software is perfect: keep copies of anything
              important.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="t-use">
            {' '}
            <h2 id="t-use">Your downloads, your responsibility</h2>{' '}
            <ul>
              {' '}
              <li>
                {' '}
                Only download and share what you have the right to. Respect copyright and the laws{' '}
                where you are.{' '}
              </li>{' '}
              <li>
                {' '}
                Fuselane uses every network you allow, including a phone on a data plan. Set a data{' '}
                allowance or a speed limit in Networks if your plan is limited; the Speedtest also{' '}
                uses data while it runs.{' '}
              </li>{' '}
              <li>
                Follow the rules of the networks you use, such as a workplace or hotel network.
              </li>{' '}
            </ul>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="t-others">
            {' '}
            <h2 id="t-others">Other services</h2>{' '}
            <p>
              {' '}
              Some features rely on services run by others: the servers you download from,{' '}
              Cloudflare&rsquo;s speed test, GitHub (updates and this project), and tools you
              install yourself, such as yt-dlp. Their own terms apply to them.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="t-changes">
            {' '}
            <h2 id="t-changes">Changes and contact</h2>{' '}
            <p>
              {' '}
              If these terms change, the new version is published here with the date at the top. See{' '}
              also the <Link href="/privacy/">privacy policy</Link>. Questions:{' '}
              <a href="mailto:hello@fuselane.app">hello@fuselane.app</a>.{' '}
            </p>{' '}
          </section>{' '}
        </article>{' '}
      </main>
      <PageScript />
    </>
  )
}
