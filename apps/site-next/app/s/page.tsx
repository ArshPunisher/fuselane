// Ported from apps/site/s/index.html by scripts/convert.py.
import type { Metadata } from 'next'
import Link from 'next/link'
import { Icon } from '@/components/icon'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Someone sent you a file with Fuselane',
  },
  description:
    "Open this Fuse Send link in Fuselane to receive the file straight from the sender's computer, encrypted end to end. Free, no account.",
  alternates: {
    canonical: 'https://fuselane.app/s/',
  },
  robots: 'noindex, nofollow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/s/',
    title: 'Someone sent you a file with Fuselane',
    description:
      "Open this Fuse Send link in Fuselane to receive the file straight from the sender's computer, encrypted end to end. Free, no account.",
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
    title: 'Someone sent you a file with Fuselane',
    description:
      "Open this Fuse Send link in Fuselane to receive the file straight from the sender's computer, encrypted end to end. Free, no account.",
    images: ['https://fuselane.app/og.png'],
  },
}

const LD: object[] = []

export default function Page() {
  return (
    <>
      <JsonLd data={LD} />
      <main id="main" className="wrap">
        {' '}
        <section className="receive" aria-labelledby="receive-title">
          {' '}
          <div className="receive-art" aria-hidden="true">
            {' '}
            <svg viewBox="0 0 360 140" className="beam">
              {' '}
              <g className="dev-a">
                {' '}
                <rect className="dev" x="14" y="40" width="104" height="64" rx="9" />{' '}
                <rect className="screen" x="24" y="50" width="84" height="44" rx="4" />{' '}
                <rect className="base" x="44" y="110" width="44" height="4" rx="2" />{' '}
              </g>{' '}
              <g className="dev-b">
                {' '}
                <rect className="dev" x="242" y="40" width="104" height="64" rx="9" />{' '}
                <rect className="screen" x="252" y="50" width="84" height="44" rx="4" />{' '}
                <rect className="bar" x="262" y="78" width="64" height="6" rx="3" />{' '}
                <rect className="base" x="272" y="110" width="44" height="4" rx="2" />{' '}
              </g>{' '}
              <path className="beam-path" d="M118 70 C 160 30, 200 110, 242 70" />{' '}
              <rect className="packet p1" x="-5" y="-5" width="10" height="10" rx="2.5" />{' '}
              <rect className="packet p2" x="-4" y="-4" width="8" height="8" rx="2" />{' '}
              <rect className="packet p3" x="-3" y="-3" width="6" height="6" rx="1.5" />{' '}
            </svg>{' '}
            <span className="receive-lock">
              <Icon name="lock" />
            </span>{' '}
          </div>{' '}
          <p className="label receive-kicker">Fuse Send</p>{' '}
          <h1 id="receive-title" data-state-title="">
            Someone sent you a file
          </h1>{' '}
          <p className="receive-lead" data-state-lead="">
            {' '}
            It comes straight from their computer to yours, encrypted on the way. Open it in
            Fuselane to receive it.{' '}
          </p>{' '}
          <div className="receive-actions" data-ok="">
            {' '}
            <a className="btn btn-primary" href="#" data-open="">
              <Icon name="arrow-right" />
              Open in Fuselane
            </a>{' '}
            <button className="btn" type="button" data-copy-link="">
              {' '}
              <Icon name="copy" />
              <span>Copy link</span>{' '}
            </button>{' '}
          </div>{' '}
          <ol className="receive-steps panel" data-ok="">
            {' '}
            <li>
              {' '}
              <span className="num">1</span>{' '}
              <span>
                <strong>Get Fuselane</strong> if you don&rsquo;t have it. It&rsquo;s free for macOS,{' '}
                Windows and Linux. <Link href="/download/">Download</Link>
              </span>{' '}
            </li>{' '}
            <li>
              {' '}
              <span className="num">2</span>{' '}
              <span>
                <strong>Open the link in Fuselane.</strong> Nothing happened? Copy the link, then in{' '}
                Fuselane go to <em>Send</em> and paste it under Receive.
              </span>{' '}
            </li>{' '}
            <li>
              {' '}
              <span className="num">3</span>{' '}
              <span>
                <strong>Ask the sender to keep Fuselane open</strong> until the file arrives.{' '}
                It&rsquo;s checked when it lands, so you get exactly what they sent.
              </span>{' '}
            </li>{' '}
          </ol>{' '}
          <p className="receive-private note">
            {' '}
            <Icon name="shield" />{' '}
            <span>
              The key to this file is in the part of the link after <code>#</code>. Browsers never{' '}
              send that part to any server, and this page doesn&rsquo;t either.
            </span>{' '}
          </p>{' '}
        </section>{' '}
      </main>
      <PageScript name="receive" />
    </>
  )
}
