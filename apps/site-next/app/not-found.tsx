// Ported from apps/site/404.html. Its styles are scoped in styles/notfound.css.
import type { Metadata } from 'next'
import Link from 'next/link'
import '@/styles/notfound.css'

export const metadata: Metadata = {
  title: { absolute: 'Page not found: Fuselane' },
  robots: { index: false },
}

export default function NotFound() {
  return (
    <main className="nf" id="main">
      <div className="nf-inner">
        <svg
          viewBox="0 0 200 200"
          role="img"
          aria-label="A ring of downloaded parts with one part missing"
        >
          <line
            className="t tide"
            style={{ '--i': '0' }}
            x1="104.1"
            y1="38.1"
            x2="105.2"
            y2="20.2"
          />
          <line
            className="t iris"
            style={{ '--i': '1' }}
            x1="112.1"
            y1="39.2"
            x2="115.6"
            y2="21.5"
          />
          <line
            className="t tide"
            style={{ '--i': '2' }}
            x1="119.9"
            y1="41.3"
            x2="125.7"
            y2="24.2"
          />
          <line
            className="t volt"
            style={{ '--i': '3' }}
            x1="127.4"
            y1="44.4"
            x2="135.4"
            y2="28.3"
          />
          <line
            className="t iris"
            style={{ '--i': '4' }}
            x1="134.4"
            y1="48.4"
            x2="144.4"
            y2="33.5"
          />
          <line
            className="t tide"
            style={{ '--i': '5' }}
            x1="140.9"
            y1="53.4"
            x2="152.7"
            y2="39.9"
          />
          <line
            className="t tide"
            style={{ '--i': '6' }}
            x1="146.6"
            y1="59.1"
            x2="160.1"
            y2="47.3"
          />
          <line
            className="t iris"
            style={{ '--i': '7' }}
            x1="151.6"
            y1="65.6"
            x2="166.5"
            y2="55.6"
          />
          <line
            className="t tide"
            style={{ '--i': '8' }}
            x1="155.6"
            y1="72.6"
            x2="171.7"
            y2="64.6"
          />
          <line
            className="t volt"
            style={{ '--i': '9' }}
            x1="158.7"
            y1="80.1"
            x2="175.8"
            y2="74.3"
          />
          <line
            className="t iris"
            style={{ '--i': '10' }}
            x1="160.8"
            y1="87.9"
            x2="178.5"
            y2="84.4"
          />
          <line
            className="t tide"
            style={{ '--i': '11' }}
            x1="161.9"
            y1="95.9"
            x2="179.8"
            y2="94.8"
          />
          <line
            className="t tide"
            style={{ '--i': '12' }}
            x1="161.9"
            y1="104.1"
            x2="179.8"
            y2="105.2"
          />
          <line
            className="t iris"
            style={{ '--i': '13' }}
            x1="160.8"
            y1="112.1"
            x2="178.5"
            y2="115.6"
          />
          <line
            className="t tide"
            style={{ '--i': '14' }}
            x1="158.7"
            y1="119.9"
            x2="175.8"
            y2="125.7"
          />
          <line
            className="t volt"
            style={{ '--i': '15' }}
            x1="155.6"
            y1="127.4"
            x2="171.7"
            y2="135.4"
          />
          <line
            className="t iris"
            style={{ '--i': '16' }}
            x1="151.6"
            y1="134.4"
            x2="166.5"
            y2="144.4"
          />
          <line
            className="t tide"
            style={{ '--i': '17' }}
            x1="146.6"
            y1="140.9"
            x2="160.1"
            y2="152.7"
          />
          <line
            className="t tide"
            style={{ '--i': '18' }}
            x1="140.9"
            y1="146.6"
            x2="152.7"
            y2="160.1"
          />
          <line
            className="t iris"
            style={{ '--i': '19' }}
            x1="134.4"
            y1="151.6"
            x2="144.4"
            y2="166.5"
          />
          <line
            className="t tide"
            style={{ '--i': '20' }}
            x1="127.4"
            y1="155.6"
            x2="135.4"
            y2="171.7"
          />
          <line
            className="t volt"
            style={{ '--i': '21' }}
            x1="119.9"
            y1="158.7"
            x2="125.7"
            y2="175.8"
          />
          <line
            className="t iris"
            style={{ '--i': '22' }}
            x1="112.1"
            y1="160.8"
            x2="115.6"
            y2="178.5"
          />
          <line
            className="t tide"
            style={{ '--i': '23' }}
            x1="104.1"
            y1="161.9"
            x2="105.2"
            y2="179.8"
          />
          <line
            className="t tide"
            style={{ '--i': '24' }}
            x1="95.9"
            y1="161.9"
            x2="94.8"
            y2="179.8"
          />
          <line
            className="t iris"
            style={{ '--i': '25' }}
            x1="87.9"
            y1="160.8"
            x2="84.4"
            y2="178.5"
          />
          <line
            className="t tide"
            style={{ '--i': '26' }}
            x1="80.1"
            y1="158.7"
            x2="74.3"
            y2="175.8"
          />
          <line
            className="t volt"
            style={{ '--i': '27' }}
            x1="72.6"
            y1="155.6"
            x2="64.6"
            y2="171.7"
          />
          <line
            className="t iris"
            style={{ '--i': '28' }}
            x1="65.6"
            y1="151.6"
            x2="55.6"
            y2="166.5"
          />
          <line
            className="t tide"
            style={{ '--i': '29' }}
            x1="59.1"
            y1="146.6"
            x2="47.3"
            y2="160.1"
          />
          <line
            className="t tide"
            style={{ '--i': '30' }}
            x1="53.4"
            y1="140.9"
            x2="39.9"
            y2="152.7"
          />
          <line
            className="t iris"
            style={{ '--i': '31' }}
            x1="48.4"
            y1="134.4"
            x2="33.5"
            y2="144.4"
          />
          <line
            className="t tide"
            style={{ '--i': '32' }}
            x1="44.4"
            y1="127.4"
            x2="28.3"
            y2="135.4"
          />
          <line className="gap" x1="41.3" y1="119.9" x2="24.2" y2="125.7" />
          <line
            className="t iris"
            style={{ '--i': '34' }}
            x1="39.2"
            y1="112.1"
            x2="21.5"
            y2="115.6"
          />
          <line
            className="t tide"
            style={{ '--i': '35' }}
            x1="38.1"
            y1="104.1"
            x2="20.2"
            y2="105.2"
          />
          <line
            className="t tide"
            style={{ '--i': '36' }}
            x1="38.1"
            y1="95.9"
            x2="20.2"
            y2="94.8"
          />
          <line
            className="t iris"
            style={{ '--i': '37' }}
            x1="39.2"
            y1="87.9"
            x2="21.5"
            y2="84.4"
          />
          <line
            className="t tide"
            style={{ '--i': '38' }}
            x1="41.3"
            y1="80.1"
            x2="24.2"
            y2="74.3"
          />
          <line
            className="t volt"
            style={{ '--i': '39' }}
            x1="44.4"
            y1="72.6"
            x2="28.3"
            y2="64.6"
          />
          <line
            className="t iris"
            style={{ '--i': '40' }}
            x1="48.4"
            y1="65.6"
            x2="33.5"
            y2="55.6"
          />
          <line
            className="t tide"
            style={{ '--i': '41' }}
            x1="53.4"
            y1="59.1"
            x2="39.9"
            y2="47.3"
          />
          <line
            className="t tide"
            style={{ '--i': '42' }}
            x1="59.1"
            y1="53.4"
            x2="47.3"
            y2="39.9"
          />
          <line
            className="t iris"
            style={{ '--i': '43' }}
            x1="65.6"
            y1="48.4"
            x2="55.6"
            y2="33.5"
          />
          <line
            className="t tide"
            style={{ '--i': '44' }}
            x1="72.6"
            y1="44.4"
            x2="64.6"
            y2="28.3"
          />
          <line
            className="t volt"
            style={{ '--i': '45' }}
            x1="80.1"
            y1="41.3"
            x2="74.3"
            y2="24.2"
          />
          <line
            className="t iris"
            style={{ '--i': '46' }}
            x1="87.9"
            y1="39.2"
            x2="84.4"
            y2="21.5"
          />
          <line
            className="t tide"
            style={{ '--i': '47' }}
            x1="95.9"
            y1="38.1"
            x2="94.8"
            y2="20.2"
          />
          <text className="code" x="100" y="111" textAnchor="middle">
            404
          </text>
        </svg>
        <h1>This page isn&rsquo;t here</h1>
        <p>
          The address may be mistyped, or the page has moved. Everything about Fuselane is a click
          away.
        </p>
        <nav className="nf-links" aria-label="Pages">
          <Link href="/">Home</Link>
          <Link href="/download/">Download</Link>
          <Link href="/guide/">Guide</Link>
          <Link href="/faq/">FAQ</Link>
        </nav>
      </div>
    </main>
  )
}
