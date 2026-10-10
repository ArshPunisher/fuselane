// Ported from apps/site/idm-alternative/index.html by scripts/convert.py.
import type { Metadata } from 'next'
import Link from 'next/link'
import { Icon } from '@/components/icon'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Free IDM alternative for Mac, Windows and Linux: Fuselane',
  },
  description:
    'Looking for an Internet Download Manager alternative? Fuselane is a free, open-source download manager for macOS, Windows and Linux that also uses Wi-Fi, Ethernet and your phone at once.',
  alternates: {
    canonical: 'https://fuselane.app/idm-alternative/',
  },
  robots: 'index, follow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/idm-alternative/',
    title: 'Free IDM alternative for Mac, Windows and Linux: Fuselane',
    description:
      'Looking for an Internet Download Manager alternative? Fuselane is a free, open-source download manager for macOS, Windows and Linux that also uses Wi-Fi, Ethernet and your phone at once.',
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
    title: 'Free IDM alternative for Mac, Windows and Linux: Fuselane',
    description:
      'Looking for an Internet Download Manager alternative? Fuselane is a free, open-source download manager for macOS, Windows and Linux that also uses Wi-Fi, Ethernet and your phone at once.',
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
        name: 'IDM alternative',
        item: 'https://fuselane.app/idm-alternative/',
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
        <section className="page-hero" aria-labelledby="idm-alternative-title">
          {' '}
          <h1 id="idm-alternative-title">A free IDM alternative for Mac, Windows and Linux</h1>{' '}
          <p>
            {' '}
            Fuselane is a free, open-source download manager. It speeds up downloads like IDM, runs
            on every computer, and goes further: it uses all your networks at once.{' '}
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
          <section className="doc-group" aria-labelledby="i-why">
            {' '}
            <h2 id="i-why">Why people look for an IDM alternative</h2>{' '}
            <p>
              {' '}
              Internet Download Manager (IDM) is a popular Windows download accelerator. It is paid{' '}
              after a trial, only runs on Windows, and its code is closed. If you use a Mac or
              Linux, don&rsquo;t want to pay, or want software you can inspect, you need something
              else.{' '}
            </p>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="i-cmp">
            {' '}
            <h2 id="i-cmp">Fuselane and IDM side by side</h2>{' '}
            <div className="cmp-wrap">
              {' '}
              <table className="cmp">
                {' '}
                <thead>
                  {' '}
                  <tr>
                    {' '}
                    <th scope="col">
                      <span className="sr-only">Feature</span>
                    </th>{' '}
                    <th scope="col">Fuselane</th> <th scope="col">IDM</th>{' '}
                  </tr>{' '}
                </thead>{' '}
                <tbody>
                  {' '}
                  <tr>
                    {' '}
                    <th scope="row">Price</th> <td>Free, forever</td>{' '}
                    <td>Paid after a trial</td>{' '}
                  </tr>{' '}
                  <tr>
                    {' '}
                    <th scope="row">Runs on</th> <td>macOS, Windows, Linux</td>{' '}
                    <td>Windows</td>{' '}
                  </tr>{' '}
                  <tr>
                    {' '}
                    <th scope="row">Open source</th> <td>Yes, Apache-2.0</td> <td>No</td>{' '}
                  </tr>{' '}
                  <tr>
                    {' '}
                    <th scope="row">Splits a download into parts</th> <td>Yes</td> <td>Yes</td>{' '}
                  </tr>{' '}
                  <tr>
                    {' '}
                    <th scope="row">Uses several networks at once</th>{' '}
                    <td>Yes: Wi-Fi, Ethernet and a USB-tethered phone together</td>{' '}
                    <td>No, one connection</td>{' '}
                  </tr>{' '}
                  <tr>
                    {' '}
                    <th scope="row">Torrents and magnet links</th> <td>Yes</td> <td>No</td>{' '}
                  </tr>{' '}
                  <tr>
                    {' '}
                    <th scope="row">Videos from web pages</th> <td>Yes, with the free yt-dlp</td>{' '}
                    <td>Yes</td>{' '}
                  </tr>{' '}
                  <tr>
                    {' '}
                    <th scope="row">Browser extension</th> <td>Chrome, Edge, Firefox</td>{' '}
                    <td>Yes</td>{' '}
                  </tr>{' '}
                  <tr>
                    {' '}
                    <th scope="row">Account, ads or tracking</th> <td>None</td>{' '}
                    <td>No account needed</td>{' '}
                  </tr>{' '}
                </tbody>{' '}
              </table>{' '}
            </div>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="i-diff">
            {' '}
            <h2 id="i-diff">What makes Fuselane different</h2>{' '}
            <p>
              {' '}
              A download accelerator opens several connections to the server, but they all go over
              the same internet line, so it can never be faster than that line. Fuselane spreads the
              parts across every network your computer has. With home broadband and a phone plugged
              in over USB, a big file comes down over both at once, and Fuselane checks every byte
              before it joins the parts into one file.{' '}
            </p>{' '}
            <ul>
              {' '}
              <li>Resumes after a dropped connection, a pause or a restart.</li>{' '}
              <li>Finds and checks published checksums by itself.</li>{' '}
              <li>
                {' '}
                Schedules, speed limits and monthly data allowances per network, so a phone plan{' '}
                isn&rsquo;t used up.{' '}
              </li>{' '}
              <li>
                {' '}
                A built-in speed test for each network, and sending large files straight to another{' '}
                computer.{' '}
              </li>{' '}
            </ul>{' '}
          </section>{' '}
          <section className="doc-group" aria-labelledby="i-switch">
            {' '}
            <h2 id="i-switch">Switching takes a minute</h2>{' '}
            <ol className="doc-steps">
              {' '}
              <li>
                <Link href="/download/">Download Fuselane</Link> for your computer and open it.
              </li>{' '}
              <li>Add the browser extension, so big downloads go to Fuselane by themselves.</li>{' '}
              <li>
                {' '}
                Plug in your phone over USB or join a second network, and every download uses it
                too.{' '}
              </li>{' '}
            </ol>{' '}
            <p>
              More in the <Link href="/guide/">guide</Link> and the <Link href="/faq/">FAQ</Link>.
            </p>{' '}
          </section>{' '}
        </article>{' '}
      </main>
      <PageScript />
    </>
  )
}
