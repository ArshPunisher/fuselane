// Ported from the original plain-HTML site.
import type { Metadata } from 'next'
import { Icon } from '@/components/icon'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Support Fuselane: star, share, report, suggest',
  },
  description:
    'Fuselane is free and open source. Star it on GitHub, share it, report a problem or suggest what it should do next.',
  alternates: {
    canonical: 'https://fuselane.app/support/',
  },
  robots: 'index, follow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/support/',
    title: 'Support Fuselane: star, share, report, suggest',
    description:
      'Fuselane is free and open source. Star it on GitHub, share it, report a problem or suggest what it should do next.',
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
    title: 'Support Fuselane: star, share, report, suggest',
    description:
      'Fuselane is free and open source. Star it on GitHub, share it, report a problem or suggest what it should do next.',
    images: ['https://fuselane.app/og.png'],
  },
}

const LD: object[] = [
  {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: [
      { '@type': 'ListItem', position: 1, name: 'Fuselane', item: 'https://fuselane.app/' },
      { '@type': 'ListItem', position: 2, name: 'Support', item: 'https://fuselane.app/support/' },
    ],
  },
]

export default function Page() {
  return (
    <>
      <JsonLd data={LD} />
      <main id="main" className="wrap">
        {' '}
        <section className="page-hero" aria-labelledby="support-title">
          {' '}
          <h1 id="support-title">Keep Fuselane free and fast</h1>{' '}
        </section>{' '}
        <div className="support-grid">
          {' '}
          <div className="panel support-lead spot reveal">
            {' '}
            <p>
              {' '}
              <strong>Fuselane is free and open source.</strong> If it helps you, star it, share it{' '}
              with someone who waits on downloads, and tell us what breaks or what it should do
              next.{' '}
            </p>{' '}
            <ul className="lead-facts">
              {' '}
              <li>
                <Icon name="shield" />
                No ads, no account, no tracking
              </li>{' '}
              <li>
                <Icon name="code" />
                Every line public, under Apache-2.0
              </li>{' '}
              <li>
                <Icon name="refresh" />
                Signed updates that install themselves
              </li>{' '}
            </ul>{' '}
            <div>
              {' '}
              <a className="btn btn-primary" href="https://github.com/ArshPunisher/fuselane">
                <Icon name="star" />
                Star on GitHub<span className="num" data-stars=""></span>
              </a>{' '}
              <p className="label" style={{ marginTop: '10px' }}>
                A star helps more people find it.
              </p>{' '}
            </div>{' '}
          </div>{' '}
          <div className="panel ways reveal" style={{ '--i': '1' }}>
            {' '}
            <button className="way" type="button" data-share="">
              {' '}
              <span className="icon-tile">
                <Icon name="share" />
              </span>{' '}
              <span>
                <strong>Share Fuselane</strong>
                <small>Know someone with slow downloads? Send them the link.</small>
              </span>{' '}
              <Icon name="arrow-up-right" />{' '}
            </button>{' '}
            <a
              className="way"
              href="https://github.com/ArshPunisher/fuselane/issues/new?labels=bug&title=Bug%3A%20"
            >
              {' '}
              <span className="icon-tile">
                <Icon name="bug" />
              </span>{' '}
              <span>
                <strong>Report a problem</strong>
                <small>Say what happened; Copy diagnostics in Settings helps.</small>
              </span>{' '}
              <Icon name="arrow-up-right" />{' '}
            </a>{' '}
            <a
              className="way"
              href="https://github.com/ArshPunisher/fuselane/issues/new?labels=enhancement&title=Idea%3A%20"
            >
              {' '}
              <span className="icon-tile">
                <Icon name="bulb" />
              </span>{' '}
              <span>
                <strong>Suggest a feature</strong>
                <small>What should Fuselane do next?</small>
              </span>{' '}
              <Icon name="arrow-up-right" />{' '}
            </a>{' '}
            <a className="way" href="https://github.com/ArshPunisher/fuselane">
              {' '}
              <span className="icon-tile">
                <Icon name="code" />
              </span>{' '}
              <span>
                <strong>Read or improve the code</strong>
                <small>Rust and TypeScript, Apache-2.0.</small>
              </span>{' '}
              <Icon name="arrow-up-right" />{' '}
            </a>{' '}
            <a className="way" href="https://github.com/ArshPunisher/fuselane/releases">
              {' '}
              <span className="icon-tile">
                <Icon name="notes" />
              </span>{' '}
              <span>
                <strong>Release notes</strong>
                <small>What changed in each version.</small>
              </span>{' '}
              <Icon name="arrow-up-right" />{' '}
            </a>{' '}
          </div>{' '}
        </div>{' '}
      </main>
      <PageScript name="support" />
    </>
  )
}
