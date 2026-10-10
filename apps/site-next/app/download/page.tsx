// Ported from the original plain-HTML site.
import type { Metadata } from 'next'
import { Icon } from '@/components/icon'
import { JsonLd } from '@/components/json-ld'
import { PageScript } from '@/components/page-script'

export const metadata: Metadata = {
  title: {
    absolute: 'Download Fuselane for macOS, Windows and Linux',
  },
  description:
    'Get Fuselane, the free download manager that uses every network at once. Universal macOS app, Windows installer, Linux AppImage, deb and rpm, and a command line tool.',
  alternates: {
    canonical: 'https://fuselane.app/download/',
  },
  robots: 'index, follow',
  openGraph: {
    type: 'website',
    siteName: 'Fuselane',
    locale: 'en_US',
    url: 'https://fuselane.app/download/',
    title: 'Download Fuselane for macOS, Windows and Linux',
    description:
      'Get Fuselane, the free download manager that uses every network at once. Universal macOS app, Windows installer, Linux AppImage, deb and rpm, and a command line tool.',
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
    title: 'Download Fuselane for macOS, Windows and Linux',
    description:
      'Get Fuselane, the free download manager that uses every network at once. Universal macOS app, Windows installer, Linux AppImage, deb and rpm, and a command line tool.',
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
        name: 'Download',
        item: 'https://fuselane.app/download/',
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
        <section className="page-hero dl-hero" aria-labelledby="dl-title">
          {' '}
          <h1 id="dl-title">Download Fuselane</h1>{' '}
          <p>
            {' '}
            <span data-ver="" hidden>
              Version <span className="num" id="version-tag"></span>, for
            </span>
            <span data-nover="">For</span> macOS, Windows and Linux.{' '}
            <a id="notes" href="https://github.com/ArshPunisher/fuselane/releases">
              Release notes
            </a>
            . Every file is listed with its checksum in{' '}
            <a id="sums" href="https://github.com/ArshPunisher/fuselane/releases">
              SHA256SUMS
            </a>
            .{' '}
          </p>{' '}
        </section>{' '}
        <div className="os-tabs" role="tablist" aria-label="Your system">
          {' '}
          <button
            role="tab"
            id="tab-mac"
            aria-controls="panel-mac"
            aria-selected="true"
            data-os="mac"
          >
            {' '}
            <Icon name="apple" />
            macOS{' '}
          </button>{' '}
          <button
            role="tab"
            id="tab-windows"
            aria-controls="panel-windows"
            aria-selected="false"
            tabIndex={-1}
            data-os="windows"
          >
            {' '}
            <Icon name="windows" />
            Windows{' '}
          </button>{' '}
          <button
            role="tab"
            id="tab-linux"
            aria-controls="panel-linux"
            aria-selected="false"
            tabIndex={-1}
            data-os="linux"
          >
            {' '}
            <Icon name="linux" />
            Linux{' '}
          </button>{' '}
        </div>{' '}
        <section className="tab-panel" id="panel-mac" role="tabpanel" aria-labelledby="tab-mac">
          {' '}
          <div className="panel file-row spot">
            {' '}
            <div>
              {' '}
              <h2>Mac app</h2>{' '}
              <p className="sub">Apple silicon and Intel, macOS 13.3 or later</p>{' '}
            </div>{' '}
            <p className="fname num">
              {' '}
              <span data-name="macos-universal.dmg">Disk image (.dmg)</span>{' '}
              <span data-size="macos-universal.dmg"></span>{' '}
            </p>{' '}
            <a className="btn btn-primary btn-small" data-file="macos-universal.dmg">
              <Icon name="download" />
              Download
            </a>{' '}
          </div>{' '}
          <section className="first-open" id="open-anyway" aria-labelledby="first-open-title">
            {' '}
            <div className="first-open-head">
              {' '}
              <h2 id="first-open-title">Opening it the first time</h2>{' '}
              <p>
                {' '}
                Fuselane is open source but not notarized by Apple (that needs a paid account), so{' '}
                macOS stops it once. It takes three clicks:{' '}
              </p>{' '}
            </div>{' '}
            <ol className="open-steps guide-steps">
              {' '}
              <li className="guide-step" style={{ '--i': '0', '--cx': '61%', '--cy': '69%' }}>
                {' '}
                <figure className="ill-wrap">
                  {' '}
                  <svg
                    className="ill"
                    viewBox="0 0 320 200"
                    role="img"
                    aria-label="The macOS alert “Fuselane” Not Opened, with its Done button marked."
                  >
                    {' '}
                    <rect className="ill-desk" width="320" height="200" rx="10" />{' '}
                    <rect className="ill-win" x="84" y="16" width="152" height="170" rx="16" />{' '}
                    <rect className="ill-icon" x="142" y="30" width="36" height="36" rx="9" />{' '}
                    <g
                      transform="translate(146 34) scale(0.055)"
                      fill="none"
                      strokeLinecap="round"
                      strokeWidth="34"
                    >
                      {' '}
                      <path d="M70 150 C 180 150, 220 256, 300 256" stroke="var(--tide)" />{' '}
                      <path d="M70 256 L 300 256" stroke="var(--volt)" />{' '}
                      <path d="M70 362 C 180 362, 220 256, 300 256" stroke="var(--iris)" />{' '}
                      <path d="M300 256 L 442 256" stroke="var(--fuse)" strokeWidth="46" />{' '}
                    </g>{' '}
                    <text className="ill-title" x="160" y="84" textAnchor="middle">
                      {' '}
                      &#8220;Fuselane&#8221; Not Opened{' '}
                    </text>{' '}
                    <rect className="ill-line" x="102" y="93" width="116" height="4" rx="2" />{' '}
                    <rect className="ill-line" x="110" y="101" width="100" height="4" rx="2" />{' '}
                    <rect className="ill-line" x="120" y="109" width="80" height="4" rx="2" />{' '}
                    <rect className="ill-target" x="98" y="124" width="124" height="22" rx="7" />{' '}
                    <text className="ill-btn-text on" x="160" y="139" textAnchor="middle">
                      Done
                    </text>{' '}
                    <rect className="ill-btn" x="98" y="152" width="124" height="22" rx="7" />{' '}
                    <text className="ill-btn-text" x="160" y="167" textAnchor="middle">
                      {' '}
                      Move to Trash{' '}
                    </text>{' '}
                  </svg>{' '}
                  <span className="ill-cursor" aria-hidden="true">
                    <Icon name="cursor" />
                  </span>{' '}
                </figure>{' '}
                <strong>&ldquo;Fuselane&rdquo; Not Opened</strong>{' '}
                <span>
                  Open Fuselane from Applications. When macOS says it can&rsquo;t verify it, click{' '}
                  <b>Done</b>.
                </span>{' '}
              </li>{' '}
              <li className="guide-step" style={{ '--i': '1', '--cx': '30%', '--cy': '68%' }}>
                {' '}
                <figure className="ill-wrap">
                  {' '}
                  <svg
                    className="ill"
                    viewBox="0 0 320 200"
                    role="img"
                    aria-label="System Settings with Privacy & Security chosen in the sidebar."
                  >
                    {' '}
                    <rect className="ill-desk" width="320" height="200" rx="10" />{' '}
                    <rect className="ill-win" x="10" y="14" width="300" height="174" rx="12" />{' '}
                    <rect className="ill-side" x="10" y="14" width="112" height="174" rx="12" />{' '}
                    <circle className="ill-light" cx="22" cy="26" r="3.5" />{' '}
                    <circle className="ill-light" cx="33" cy="26" r="3.5" />{' '}
                    <circle className="ill-light" cx="44" cy="26" r="3.5" />{' '}
                    <rect className="ill-field" x="18" y="34" width="96" height="10" rx="5" />{' '}
                    <rect className="ill-dot" x="23" y="50" width="8" height="8" rx="2" />{' '}
                    <rect className="ill-line" x="35" y="52" width="44" height="4" rx="2" />{' '}
                    <rect className="ill-dot" x="23" y="70" width="8" height="8" rx="2" />{' '}
                    <rect className="ill-line" x="35" y="72" width="52" height="4" rx="2" />{' '}
                    <rect className="ill-dot" x="23" y="90" width="8" height="8" rx="2" />{' '}
                    <rect className="ill-line" x="35" y="92" width="38" height="4" rx="2" />{' '}
                    <rect className="ill-dot" x="23" y="110" width="8" height="8" rx="2" />{' '}
                    <rect className="ill-line" x="35" y="112" width="48" height="4" rx="2" />{' '}
                    <rect className="ill-sel" x="18" y="126" width="98" height="16" rx="5" />{' '}
                    <rect className="ill-dot on" x="23" y="130" width="8" height="8" rx="2" />{' '}
                    <text className="ill-small on" x="35" y="137">
                      Privacy &amp; Security
                    </text>{' '}
                    <rect className="ill-dot" x="23" y="150" width="8" height="8" rx="2" />{' '}
                    <rect className="ill-line" x="35" y="152" width="46" height="4" rx="2" />{' '}
                    <rect
                      className="ill-target ring-only"
                      x="16"
                      y="124"
                      width="102"
                      height="20"
                      rx="6"
                    />{' '}
                    <text className="ill-title" x="134" y="38">
                      Privacy &amp; Security
                    </text>{' '}
                    <rect className="ill-row" x="134" y="50" width="164" height="40" rx="7" />{' '}
                    <rect className="ill-line" x="144" y="61" width="90" height="4" rx="2" />{' '}
                    <rect className="ill-line" x="144" y="73" width="110" height="4" rx="2" />{' '}
                    <rect className="ill-row" x="134" y="98" width="164" height="40" rx="7" />{' '}
                    <rect className="ill-line" x="144" y="109" width="70" height="4" rx="2" />{' '}
                    <rect className="ill-line" x="144" y="121" width="100" height="4" rx="2" />{' '}
                    <text className="ill-small strong" x="134" y="158">
                      Security
                    </text>{' '}
                    <path className="ill-arrow" d="M296 150 v18 m-5 -5 l5 5 l5 -5" />{' '}
                  </svg>{' '}
                  <span className="ill-cursor" aria-hidden="true">
                    <Icon name="cursor" />
                  </span>{' '}
                </figure>{' '}
                <strong>Privacy &amp; Security</strong>{' '}
                <span>
                  Open <b>System Settings</b> and choose <b>Privacy &amp; Security</b>. Scroll down{' '}
                  to Security.
                </span>{' '}
              </li>{' '}
              <li className="guide-step" style={{ '--i': '2', '--cx': '84%', '--cy': '77%' }}>
                {' '}
                <figure className="ill-wrap">
                  {' '}
                  <svg
                    className="ill"
                    viewBox="0 0 320 200"
                    role="img"
                    aria-label="The Security section saying Fuselane was blocked, with its Open Anyway button marked."
                  >
                    {' '}
                    <rect className="ill-desk" width="320" height="200" rx="10" />{' '}
                    <rect className="ill-win" x="10" y="14" width="300" height="174" rx="12" />{' '}
                    <circle className="ill-light" cx="22" cy="26" r="3.5" />{' '}
                    <circle className="ill-light" cx="33" cy="26" r="3.5" />{' '}
                    <circle className="ill-light" cx="44" cy="26" r="3.5" />{' '}
                    <text className="ill-title" x="24" y="56">
                      Security
                    </text>{' '}
                    <rect className="ill-row" x="22" y="66" width="276" height="34" rx="7" />{' '}
                    <rect className="ill-line" x="32" y="76" width="120" height="4" rx="2" />{' '}
                    <rect className="ill-line" x="32" y="86" width="84" height="4" rx="2" />{' '}
                    <rect className="ill-row" x="22" y="108" width="276" height="62" rx="7" />{' '}
                    <rect className="ill-icon small" x="32" y="118" width="18" height="18" rx="5" />{' '}
                    <g
                      transform="translate(34 120) scale(0.028)"
                      fill="none"
                      strokeLinecap="round"
                      strokeWidth="34"
                    >
                      {' '}
                      <path d="M70 150 C 180 150, 220 256, 300 256" stroke="var(--tide)" />{' '}
                      <path d="M70 256 L 300 256" stroke="var(--volt)" />{' '}
                      <path d="M70 362 C 180 362, 220 256, 300 256" stroke="var(--iris)" />{' '}
                      <path d="M300 256 L 442 256" stroke="var(--fuse)" strokeWidth="46" />{' '}
                    </g>{' '}
                    <text className="ill-small" x="58" y="125">
                      &#8220;Fuselane&#8221; was blocked
                    </text>{' '}
                    <text className="ill-small" x="58" y="136">
                      to protect your Mac.
                    </text>{' '}
                    <rect className="ill-target" x="200" y="140" width="88" height="22" rx="7" />{' '}
                    <text className="ill-btn-text on" x="244" y="155" textAnchor="middle">
                      {' '}
                      Open Anyway{' '}
                    </text>{' '}
                  </svg>{' '}
                  <span className="ill-cursor" aria-hidden="true">
                    <Icon name="cursor" />
                  </span>{' '}
                </figure>{' '}
                <strong>Open Anyway</strong>{' '}
                <span>
                  Click <b>Open Anyway</b>, then <b>Open</b>. From now on it opens like any other{' '}
                  app.
                </span>{' '}
              </li>{' '}
            </ol>{' '}
          </section>{' '}
          <section className="skip-warning" aria-labelledby="skip-title">
            {' '}
            <div className="skip-copy">
              {' '}
              <h2 id="skip-title" className="label-row">
                Or skip the warning: install with one command
              </h2>{' '}
              <p>
                {' '}
                Paste this in Terminal. The script downloads the app, checks it against the{' '}
                release&rsquo;s SHA256SUMS, moves it to Applications and clears the quarantine flag,{' '}
                which is what Open Anyway does. Nothing is installed if the check fails.{' '}
              </p>{' '}
              <p className="label-row small">Or with Homebrew</p>{' '}
              <div className="cmd">
                {' '}
                <code id="cmd-brew">brew install --cask arshpunisher/tap/fuselane</code>
                <button className="copy" data-copy="cmd-brew" type="button">
                  Copy
                </button>{' '}
              </div>{' '}
            </div>{' '}
            <div className="term" aria-label="Terminal running the install script">
              {' '}
              <div className="term-bar" aria-hidden="true">
                {' '}
                <span></span>
                <span></span>
                <span></span>Terminal{' '}
              </div>{' '}
              <div className="cmd term-cmd">
                {' '}
                <code id="cmd-script">
                  curl -fsSL{' '}
                  https://raw.githubusercontent.com/ArshPunisher/fuselane/main/packaging/macos/install.sh{' '}
                  | sh
                </code>
                <button className="copy" data-copy="cmd-script" type="button">
                  Copy
                </button>{' '}
              </div>{' '}
              <ol className="term-out num" aria-label="What it prints">
                {' '}
                <li style={{ '--i': '0' }}>
                  {' '}
                  fuselane: downloading Fuselane <span data-ver-text="">&lt;version&gt;</span>{' '}
                </li>{' '}
                <li style={{ '--i': '1' }}>
                  {' '}
                  fuselane: installed Fuselane <span data-ver-text="">&lt;version&gt;</span> in{' '}
                  /Applications. Open it from Launchpad or Spotlight.{' '}
                </li>{' '}
              </ol>{' '}
            </div>{' '}
          </section>{' '}
        </section>{' '}
        <section
          className="tab-panel"
          id="panel-windows"
          role="tabpanel"
          aria-labelledby="tab-windows"
          hidden
        >
          {' '}
          <div className="panel file-row spot">
            {' '}
            <div>
              {' '}
              <h2>Windows installer</h2> <p className="sub">Windows 10 or 11, x64</p>{' '}
            </div>{' '}
            <p className="fname num">
              {' '}
              <span data-name="windows-x64-setup.exe">Installer (.exe)</span>{' '}
              <span data-size="windows-x64-setup.exe"></span>{' '}
            </p>{' '}
            <a className="btn btn-primary btn-small" data-file="windows-x64-setup.exe">
              <Icon name="download" />
              Download
            </a>{' '}
          </div>{' '}
          <div className="qa">
            {' '}
            <details>
              {' '}
              <summary>Windows says it protected your PC?</summary>{' '}
              <div className="answer">
                {' '}
                <p>
                  {' '}
                  Code signing through the SignPath Foundation is being set up. Until then,{' '}
                  SmartScreen may stop a new app it hasn&rsquo;t seen often:{' '}
                </p>{' '}
                <ol className="guide-steps win-steps">
                  {' '}
                  <li className="guide-step" style={{ '--i': '0', '--cx': '27%', '--cy': '50%' }}>
                    {' '}
                    <figure className="ill-wrap">
                      {' '}
                      <svg
                        className="ill"
                        viewBox="0 0 320 200"
                        role="img"
                        aria-label="The Windows message Windows protected your PC, with its More info link marked."
                      >
                        {' '}
                        <rect className="ill-desk" width="320" height="200" rx="10" />{' '}
                        <rect
                          className="ill-win sq"
                          x="22"
                          y="18"
                          width="276"
                          height="164"
                          rx="4"
                        />{' '}
                        <text className="ill-title big" x="40" y="52">
                          Windows protected your PC
                        </text>{' '}
                        <rect className="ill-line" x="40" y="66" width="220" height="4" rx="2" />{' '}
                        <rect className="ill-line" x="40" y="76" width="190" height="4" rx="2" />{' '}
                        <rect
                          className="ill-target ring-only"
                          x="36"
                          y="88"
                          width="60"
                          height="18"
                          rx="4"
                        />{' '}
                        <text className="ill-link" x="42" y="101">
                          More info
                        </text>{' '}
                        <rect
                          className="ill-btn sq"
                          x="206"
                          y="146"
                          width="78"
                          height="22"
                          rx="3"
                        />{' '}
                        <text className="ill-btn-text" x="245" y="161" textAnchor="middle">
                          {' '}
                          Don&#8217;t run{' '}
                        </text>{' '}
                      </svg>{' '}
                      <span className="ill-cursor" aria-hidden="true">
                        <Icon name="cursor" />
                      </span>{' '}
                    </figure>{' '}
                    <span>
                      Choose <b>More info</b>.
                    </span>{' '}
                  </li>{' '}
                  <li className="guide-step" style={{ '--i': '1', '--cx': '58%', '--cy': '80%' }}>
                    {' '}
                    <figure className="ill-wrap">
                      {' '}
                      <svg
                        className="ill"
                        viewBox="0 0 320 200"
                        role="img"
                        aria-label="The same message after More info, with its Run anyway button marked."
                      >
                        {' '}
                        <rect className="ill-desk" width="320" height="200" rx="10" />{' '}
                        <rect
                          className="ill-win sq"
                          x="22"
                          y="18"
                          width="276"
                          height="164"
                          rx="4"
                        />{' '}
                        <text className="ill-title big" x="40" y="52">
                          Windows protected your PC
                        </text>{' '}
                        <rect className="ill-line" x="40" y="66" width="220" height="4" rx="2" />{' '}
                        <text className="ill-small" x="40" y="92">
                          App: Fuselane_&#8230;_setup.exe
                        </text>{' '}
                        <text className="ill-small" x="40" y="106">
                          Publisher: Unknown publisher
                        </text>{' '}
                        <rect
                          className="ill-target"
                          x="120"
                          y="146"
                          width="80"
                          height="22"
                          rx="3"
                        />{' '}
                        <text className="ill-btn-text on" x="160" y="161" textAnchor="middle">
                          {' '}
                          Run anyway{' '}
                        </text>{' '}
                        <rect
                          className="ill-btn sq"
                          x="206"
                          y="146"
                          width="78"
                          height="22"
                          rx="3"
                        />{' '}
                        <text className="ill-btn-text" x="245" y="161" textAnchor="middle">
                          {' '}
                          Don&#8217;t run{' '}
                        </text>{' '}
                      </svg>{' '}
                      <span className="ill-cursor" aria-hidden="true">
                        <Icon name="cursor" />
                      </span>{' '}
                    </figure>{' '}
                    <span>
                      Choose <b>Run anyway</b>.
                    </span>{' '}
                  </li>{' '}
                </ol>{' '}
                <p>Fuselane installs for your account only and updates itself after that.</p>{' '}
              </div>{' '}
            </details>{' '}
          </div>{' '}
        </section>{' '}
        <section
          className="tab-panel"
          id="panel-linux"
          role="tabpanel"
          aria-labelledby="tab-linux"
          hidden
        >
          {' '}
          <div className="panel file-row spot">
            {' '}
            <div>
              {' '}
              <h2>AppImage</h2> <p className="sub">x64, runs on most distributions</p>{' '}
            </div>{' '}
            <p className="fname num">
              <span data-size="linux-x64.AppImage"></span>
            </p>{' '}
            <a className="btn btn-primary btn-small" data-file="linux-x64.AppImage">
              <Icon name="download" />
              Download
            </a>{' '}
          </div>{' '}
          <div className="panel file-row spot">
            {' '}
            <div>
              {' '}
              <h2>Debian and Ubuntu</h2> <p className="sub">.deb, Ubuntu 22.04 or newer</p>{' '}
            </div>{' '}
            <p className="fname num">
              <span data-size="linux-x64.deb"></span>
            </p>{' '}
            <span className="cta">
              <a className="btn btn-small" data-file="linux-x64.deb">
                x64
              </a>
              <a className="btn btn-small" data-file="linux-arm64.deb">
                arm64
              </a>
            </span>{' '}
          </div>{' '}
          <div className="panel file-row spot">
            {' '}
            <div>
              {' '}
              <h2>Fedora and openSUSE</h2> <p className="sub">.rpm</p>{' '}
            </div>{' '}
            <p className="fname num">
              <span data-size="linux-x64.rpm"></span>
            </p>{' '}
            <span className="cta">
              <a className="btn btn-small" data-file="linux-x64.rpm">
                x64
              </a>
              <a className="btn btn-small" data-file="linux-arm64.rpm">
                arm64
              </a>
            </span>{' '}
          </div>{' '}
        </section>{' '}
        <section className="more section" aria-labelledby="more-title">
          {' '}
          <h2 id="more-title" className="reveal">
            More ways to use it
          </h2>{' '}
          <div className="more-grid">
            {' '}
            <article className="panel more-card spot reveal">
              {' '}
              <span className="icon-tile">
                <Icon name="terminal" />
              </span>{' '}
              <h3>Command line</h3>{' '}
              <p>
                {' '}
                The same engine for scripts and servers:{' '}
                <code className="num">fuselane get &lt;link&gt;</code>.{' '}
              </p>{' '}
              <p className="row">
                {' '}
                <a className="btn btn-small" data-cli="macos-universal.tar.gz">
                  macOS
                </a>{' '}
                <a className="btn btn-small" data-cli="windows-x64.exe">
                  Windows
                </a>{' '}
                <a className="btn btn-small" data-cli="linux-x64.tar.gz">
                  Linux x64
                </a>{' '}
                <a className="btn btn-small" data-cli="linux-arm64.tar.gz">
                  Linux arm64
                </a>{' '}
              </p>{' '}
            </article>{' '}
            <article className="panel more-card spot reveal" style={{ '--i': '1' }}>
              {' '}
              <span className="icon-tile">
                <Icon name="puzzle" />
              </span>{' '}
              <h3>Browser extension</h3>{' '}
              <p>
                {' '}
                Sends big downloads from Chrome, Edge and Brave to the app. It is in review on the{' '}
                Chrome Web Store; the app sets itself up for it, nothing to install by hand.{' '}
              </p>{' '}
            </article>{' '}
            <article className="panel more-card verify spot reveal" style={{ '--i': '2' }}>
              {' '}
              <span className="icon-tile">
                <Icon name="seal" />
              </span>{' '}
              <h3>Check a download yourself</h3>{' '}
              <p>
                {' '}
                Put SHA256SUMS next to the file you downloaded, then run this in the same folder{' '}
                (macOS and Linux):{' '}
              </p>{' '}
              <div className="cmd">
                {' '}
                <code id="cmd-verify">shasum -a 256 -c SHA256SUMS --ignore-missing</code>
                <button className="copy" data-copy="cmd-verify" type="button">
                  Copy
                </button>{' '}
              </div>{' '}
            </article>{' '}
          </div>{' '}
        </section>{' '}
      </main>
      <PageScript name="download" />
    </>
  )
}
