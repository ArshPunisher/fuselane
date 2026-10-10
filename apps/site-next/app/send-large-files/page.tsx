// Ported from apps/site/send-large-files/index.html by scripts/convert.py.
import type { Metadata } from 'next'
import Link from 'next/link'
import { Icon } from '@/components/icon'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Send large files free, no upload and no size limit: Fuselane',
  },
  description:
    'Send a file or folder of any size straight from your computer, encrypted, with no upload, no account and no size limit. Free with Fuselane on macOS, Windows and Linux.',
  alternates: {
    canonical: 'https://fuselane.app/send-large-files/',
  },
  robots: 'index, follow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/send-large-files/',
    title: 'Send large files free, no upload and no size limit: Fuselane',
    description:
      'Send a file or folder of any size straight from your computer, encrypted, with no upload, no account and no size limit. Free with Fuselane on macOS, Windows and Linux.',
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
    title: 'Send large files free, no upload and no size limit: Fuselane',
    description:
      'Send a file or folder of any size straight from your computer, encrypted, with no upload, no account and no size limit. Free with Fuselane on macOS, Windows and Linux.',
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
        name: 'Send large files',
        item: 'https://fuselane.app/send-large-files/',
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
        <section className="page-hero" aria-labelledby="send-large-files-title">
          {' '}
          <h1 id="send-large-files-title">Send large files free, with no size limit</h1>{' '}
          <p>
            {' '}
            A link that sends your file straight from your computer to theirs, encrypted. No upload,{' '}
            no account and no size limit.{' '}
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
          <section className="doc-group" aria-labelledby="s-how">
            {' '}
            <h2 id="s-how">Send any size, straight from your computer</h2>{' '}
            <p>
              {' '}
              Fuse Send gives you a link for a file or a folder. The person you send it to opens the{' '}
              link and the file comes straight from your computer to theirs, encrypted. Nothing is{' '}
              uploaded to a server first, so there&rsquo;s no size limit, no waiting for an upload
              and no copy left in a cloud.{' '}
            </p>{' '}
            <ul>
              {' '}
              <li>No account, no upload, no size limit.</li>{' '}
              <li>Encrypted end to end: the key is only in the link, after the #.</li>{' '}
              <li>The download is checked against what you sent.</li>{' '}
            </ul>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="s-near">
            {' '}
            <h2 id="s-near">Nearby: computers and phones on the same Wi-Fi</h2>{' '}
            <p>
              {' '}
              On the same network, drop files on a device in Send and they go across directly.
              Phones without the app can scan a code and send or receive in the browser. It works
              with LocalSend too, and you can keep a folder in sync between two computers.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="s-keep">
            {' '}
            <h2 id="s-keep">What to know</h2>{' '}
            <p>
              {' '}
              Keep Fuselane open until the file arrives. On the same network it connects straight{' '}
              away; across the internet your router needs UPnP turned on. Share the link the way{' '}
              you&rsquo;d share a password.{' '}
            </p>{' '}
            <p>
              More in the guide: <Link href="/guide/#sending">sending</Link>.
            </p>{' '}
          </section>{' '}
        </article>{' '}
      </main>
      <PageScript />
    </>
  )
}
