// Ported from apps/site/combine-internet/index.html by scripts/convert.py.
import type { Metadata } from 'next'
import Link from 'next/link'
import { Icon } from '@/components/icon'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Combine Wi-Fi and mobile data for faster downloads: Fuselane',
  },
  description:
    'Use Wi-Fi, Ethernet and your phone’s mobile data at the same time to download one file faster. Free download manager for macOS, Windows and Linux, no VPN or router change.',
  alternates: {
    canonical: 'https://fuselane.app/combine-internet/',
  },
  robots: 'index, follow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/combine-internet/',
    title: 'Combine Wi-Fi and mobile data for faster downloads: Fuselane',
    description:
      'Use Wi-Fi, Ethernet and your phone’s mobile data at the same time to download one file faster. Free download manager for macOS, Windows and Linux, no VPN or router change.',
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
    title: 'Combine Wi-Fi and mobile data for faster downloads: Fuselane',
    description:
      'Use Wi-Fi, Ethernet and your phone’s mobile data at the same time to download one file faster. Free download manager for macOS, Windows and Linux, no VPN or router change.',
    images: ['https://fuselane.app/og.png'],
  },
}

const LD: object[] = [
  {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: [
      { '@type': 'ListItem', position: 1, name: 'Fuselane', item: 'https://fuselane.app/' },
      {
        '@type': 'ListItem',
        position: 2,
        name: 'Combine internet connections',
        item: 'https://fuselane.app/combine-internet/',
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
        <section className="page-hero" aria-labelledby="combine-internet-title">
          {' '}
          <h1 id="combine-internet-title">Combine Wi-Fi and mobile data to download faster</h1>{' '}
          <p>
            {' '}
            Fuselane downloads one file over every connection you have at once: Wi-Fi, Ethernet and
            a phone tethered over USB. Free, and nothing to set up on your router.{' '}
          </p>{' '}
          <p className="hero-cta">
            {' '}
            <Link className="btn btn-primary" href="/download/">
              <Icon name="download" />
              Download Fuselane free
            </Link>{' '}
          </p>{' '}
        </section>{' '}
        <article className="doc landing">
          {' '}
          <section className="doc-group" aria-labelledby="c-how">
            {' '}
            <h2 id="c-how">Use two internet connections for one download</h2>{' '}
            <p>
              {' '}
              Your computer can be on Wi-Fi, Ethernet and a phone tethered over USB at the same
              time, but normally only one of them carries your downloads. Fuselane uses all of them
              at once: it asks the server for the file in parts, sends each part over whichever
              network is free, checks every part and joins them into the exact file the server
              sent.{' '}
            </p>{' '}
            <p>
              {' '}
              If your broadband gives 50 Mbps and your phone 30 Mbps, a large file can come down at{' '}
              close to 80 Mbps together. When both connections share one line, the speed test shows{' '}
              it, because then adding them can&rsquo;t help.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="c-what">
            {' '}
            <h2 id="c-what">What you can combine</h2>{' '}
            <ul>
              {' '}
              <li>Home Wi-Fi and wired Ethernet from different providers.</li>{' '}
              <li>Broadband and mobile data from a phone connected by USB.</li>{' '}
              <li>Two Wi-Fi adapters on different networks.</li>{' '}
            </ul>{' '}
            <p>
              {' '}
              No router change, no VPN, no relay server and no admin rights: it all happens on your{' '}
              computer, and it works with ordinary download links.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="c-data">
            {' '}
            <h2 id="c-data">Careful with a phone&rsquo;s data</h2>{' '}
            <p>
              {' '}
              Give each network a monthly allowance or a speed limit, choose when it may help
              (always, only for long downloads, or at set hours), and Fuselane stops using it when
              the allowance is reached. The built-in speed test shows what each network really
              delivers.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="c-limits">
            {' '}
            <h2 id="c-limits">When it can&rsquo;t combine</h2>{' '}
            <p>
              {' '}
              A server that doesn&rsquo;t allow downloading in parts sends the whole file over one{' '}
              network. Streaming sites and games manage their own connections, so Fuselane speeds up{' '}
              downloads, not your whole internet.{' '}
            </p>{' '}
            <p>
              Read <Link href="/guide/#combining">how combining works</Link> in the guide.
            </p>{' '}
          </section>{' '}
        </article>{' '}
      </main>
      <PageScript />
    </>
  )
}
