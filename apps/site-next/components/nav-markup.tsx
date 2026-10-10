// Ported from the original plain-HTML site.
import Link from 'next/link'
import { Icon } from './icon'

export function NavMarkup({ current }: { current: string }) {
  return (
    <>
      <a className="skip" href="#main">
        Skip to content
      </a>{' '}
      <div className="nav-sentinel" aria-hidden="true"></div>{' '}
      <header className="nav" data-nav="">
        {' '}
        <div className="nav-inner wrap">
          {' '}
          <Link className="wordmark" href="/" translate="no" aria-label="Fuselane home">
            {' '}
            <svg className="mark" viewBox="0 0 512 512" aria-hidden="true">
              {' '}
              <g fill="none" strokeLinecap="round" strokeWidth="34">
                {' '}
                <path
                  className="lane l1"
                  pathLength="1"
                  d="M70 150 C 180 150, 220 256, 300 256"
                  stroke="var(--lane-tide)"
                />{' '}
                <path
                  className="lane l2"
                  pathLength="1"
                  d="M70 256 L 300 256"
                  stroke="var(--lane-volt)"
                />{' '}
                <path
                  className="lane l3"
                  pathLength="1"
                  d="M70 362 C 180 362, 220 256, 300 256"
                  stroke="var(--lane-iris)"
                />{' '}
                <path
                  className="fuse"
                  pathLength="1"
                  d="M300 256 L 442 256"
                  stroke="var(--fuse)"
                  strokeWidth="46"
                />{' '}
              </g>{' '}
            </svg>{' '}
            <span>Fuselane</span>{' '}
          </Link>{' '}
          <nav className="nav-main" aria-label="Main">
            {' '}
            <div className="nav-track">
              {' '}
              <span className="nav-glide" aria-hidden="true"></span>{' '}
              <ul className="nav-links">
                {' '}
                <li>
                  <Link href="/#features">Features</Link>
                </li>{' '}
                <li>
                  <Link href="/#send">Send</Link>
                </li>{' '}
                <li>
                  <Link href="/guide/" aria-current={current === '/guide/' ? 'page' : undefined}>
                    Guide
                  </Link>
                </li>{' '}
                <li>
                  <Link href="/faq/" aria-current={current === '/faq/' ? 'page' : undefined}>
                    FAQ
                  </Link>
                </li>{' '}
                <li>
                  <Link
                    href="/support/"
                    aria-current={current === '/support/' ? 'page' : undefined}
                  >
                    Support
                  </Link>
                </li>{' '}
              </ul>{' '}
            </div>{' '}
            <a
              className="star"
              href="https://github.com/ArshPunisher/fuselane"
              aria-label="Fuselane on GitHub (star it)"
            >
              {' '}
              <Icon name="github" /> <span className="count num" data-stars=""></span>{' '}
            </a>{' '}
            <Link
              className="btn btn-primary nav-cta"
              href="/download/"
              aria-current={current === '/download/' ? 'page' : undefined}
            >
              <Icon name="download" />
              Download
            </Link>{' '}
          </nav>{' '}
          <button
            className="nav-menu"
            type="button"
            aria-expanded="false"
            aria-controls="nav-sheet"
          >
            {' '}
            <Icon name="menu" /> <span className="sr-only">Menu</span>{' '}
          </button>{' '}
        </div>{' '}
        <span className="nav-progress" aria-hidden="true"></span>{' '}
      </header>{' '}
      <nav className="nav-sheet" id="nav-sheet" aria-label="Pages" hidden>
        {' '}
        <ul>
          {' '}
          <li style={{ '--i': '0' }}>
            <Link href="/#features">Features</Link>
          </li>{' '}
          <li style={{ '--i': '1' }}>
            <Link href="/#send">Send</Link>
          </li>{' '}
          <li style={{ '--i': '2' }}>
            <Link href="/guide/" aria-current={current === '/guide/' ? 'page' : undefined}>
              Guide
            </Link>
          </li>{' '}
          <li style={{ '--i': '3' }}>
            <Link href="/faq/" aria-current={current === '/faq/' ? 'page' : undefined}>
              FAQ
            </Link>
          </li>{' '}
          <li style={{ '--i': '4' }}>
            <Link href="/support/" aria-current={current === '/support/' ? 'page' : undefined}>
              Support
            </Link>
          </li>{' '}
          <li style={{ '--i': '5' }}>
            {' '}
            <Link
              className="btn btn-primary"
              href="/download/"
              aria-current={current === '/download/' ? 'page' : undefined}
            >
              <Icon name="download" />
              Download
            </Link>{' '}
          </li>{' '}
        </ul>{' '}
      </nav>{' '}
    </>
  )
}
