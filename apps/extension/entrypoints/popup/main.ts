import { formatSize, normalizeRules } from '@fuselane/capture'
import {
  appStatus,
  browserId,
  mustPush,
  sendHandoff,
  settle,
  type Status,
} from '../../lib/status.ts'
import { offerLink } from '../../lib/handoff.ts'
import { findOnPage, mediaList, type MediaItem } from '../../lib/media.ts'
import { getSession } from '../../lib/session.ts'
import { feedList, findFeeds, type FoundFeed } from '../../lib/feeds.ts'
import { fillIcons, icon, type IconName } from './icons.ts'
import './style.css'

const HOST = 'app.fuselane.host'
const DOWNLOAD_PAGE = 'https://fuselane.app/download/?ref=extension'

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!
const status = $<HTMLParagraphElement>('#status')
const chip = $<HTMLSpanElement>('#chip')
const card = $<HTMLElement>('#app-card')
const enabled = $<HTMLInputElement>('#enabled')
const summary = $<HTMLParagraphElement>('#handoff-summary')

const ask = (m: unknown) => browser.runtime.sendNativeMessage(HOST, m as object)
const hello = () => ({
  browser: browserId(navigator.userAgent, 'brave' in navigator),
  version: browser.runtime.getManifest().version,
})

/** What the switch's line says, in the app's terms. */
function describe(on: boolean, minBytes: number, state: Status['state']): string {
  if (!on) return 'Off: every download stays in the browser.'
  const what = minBytes > 0 ? `Downloads over ${formatSize(minBytes)}` : 'Every download'
  return state === 'connected'
    ? `${what} go to Fuselane.`
    : `${what} go to Fuselane when it's running.`
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> = {},
  ...kids: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const e = Object.assign(document.createElement(tag), props)
  e.append(...kids)
  return e
}

/** The card for an app that's missing or closed: what's wrong and the one fix. */
function showCard(s: Status) {
  card.replaceChildren()
  card.dataset.state = s.state
  if (s.state === 'connected') {
    card.hidden = true
    return
  }
  const missing = s.state === 'not-installed'
  const go = el(
    'a',
    {
      className: 'btn btn-primary btn-block',
      href: missing ? DOWNLOAD_PAGE : 'fuselane://open',
      target: '_blank',
      rel: 'noopener',
    },
    icon(missing ? 'download-simple' : 'power'),
    missing ? 'Get the Fuselane app' : 'Open Fuselane',
  )
  if (!missing) {
    // The OS starts the app for its own link; look again once it's had a moment.
    go.addEventListener('click', () => setTimeout(() => void load(), 2500))
  }
  card.append(
    el(
      'div',
      { className: 'card-head' },
      el('span', { className: 'card-ic' }, icon('desktop')),
      el(
        'div',
        {},
        el('p', { className: 'card-title' }, missing ? 'Get the free app' : 'Fuselane is closed'),
        el(
          'p',
          { className: 'muted small' },
          missing
            ? 'The extension hands downloads to the Fuselane app, which splits them across Wi‑Fi, Ethernet and your phone at once. Until then, downloads stay in the browser.'
            : 'Open it and big downloads go to it again. Until then, they stay in the browser.',
        ),
      ),
    ),
    go,
  )
  if (missing)
    card.append(el('p', { className: 'muted small center' }, 'Free for Windows, macOS and Linux.'))
  card.hidden = false
}

async function load() {
  const stored = await browser.storage.local.get(['rules', 'handoffPending', 'handoffSynced'])
  const s = await appStatus(ask, hello())
  // The app decides where downloads go, unless we have a change it hasn't seen.
  const { rules, pending } = await settle(s, normalizeRules(stored.rules), mustPush(stored), ask)
  await browser.storage.local.set({
    rules,
    handoffPending: pending,
    handoffSynced: stored.handoffSynced === true || (s.state === 'connected' && !pending),
  })
  enabled.checked = rules.enabled
  summary.textContent = describe(rules.enabled, rules.minBytes, s.state)
  status.textContent = s.text
  chip.dataset.state = s.state
  chip.textContent =
    s.state === 'connected' ? 'Connected' : s.state === 'not-running' ? 'App closed' : 'No app'
  chip.title = s.text
  showCard(s)
  return s
}

enabled.addEventListener('change', async () => {
  const { rules } = await browser.storage.local.get('rules')
  const next = { ...normalizeRules(rules), enabled: enabled.checked }
  // Tell the app too, so its Browser extension page and other browsers agree; if
  // it's closed, the change waits and goes with the next ping.
  const delivered = await sendHandoff(ask, next)
  await browser.storage.local.set({
    rules: next,
    handoffPending: !delivered,
    ...(delivered ? { handoffSynced: true } : {}),
  })
  summary.textContent = describe(
    next.enabled,
    next.minBytes,
    delivered ? 'connected' : 'not-running',
  )
})

for (const link of [$<HTMLAnchorElement>('#settings'), $<HTMLAnchorElement>('#more-settings')]) {
  link.addEventListener('click', (e) => {
    e.preventDefault()
    void browser.runtime.openOptionsPage()
    window.close()
  })
}

const KIND: Record<MediaItem['kind'], { word: string; icon: IconName }> = {
  video: { word: 'Video', icon: 'video-camera' },
  audio: { word: 'Audio', icon: 'music-note' },
  link: { word: 'File', icon: 'file' },
}

/** Lists the tab's videos, audio and file links (activeTab: only now, only this tab). */
async function onThisPage() {
  const section = $<HTMLElement>('#page')
  const list = $<HTMLUListElement>('#media')
  const said = $<HTMLParagraphElement>('#media-status')
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true })
  if (!tab?.id || !tab.url || !/^https?:/.test(tab.url)) return
  let items: MediaItem[] = []
  try {
    const [res] = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: findOnPage,
    })
    items = mediaList((res?.result as Parameters<typeof mediaList>[0]) ?? [], tab.url)
  } catch {
    return // a page the browser doesn't let extensions read
  }
  section.hidden = false
  // A video page (YouTube and many more): the app finds the real video and its
  // qualities, then downloads it over every network.
  const video = $<HTMLButtonElement>('#video')
  video.addEventListener('click', async () => {
    video.disabled = true
    const reply = (await ask({ v: 1, type: 'page.video', url: tab.url }).catch(() => null)) as {
      type?: string
    } | null
    if (reply?.type === 'page.video.opened') {
      window.close()
    } else {
      video.disabled = false
      said.textContent =
        "Fuselane isn't running, or this version can't get videos yet. Open it and try again."
    }
  })
  // A site with a feed (podcasts, blogs, releases): follow it in Fuselane, and
  // new files download by themselves.
  let feeds: FoundFeed[] = []
  try {
    const [res] = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: findFeeds,
    })
    feeds = feedList((res?.result as FoundFeed[]) ?? [], tab.url)
  } catch {
    feeds = []
  }
  const feedBox = $<HTMLDivElement>('#feeds')
  for (const f of feeds) {
    const b = el(
      'button',
      { type: 'button', className: 'btn btn-block', title: f.href },
      icon('rss'),
      feeds.length > 1 && f.title
        ? `Follow “${f.title}” in Fuselane`
        : "Follow this site's feed in Fuselane",
    )
    b.addEventListener('click', async () => {
      b.disabled = true
      const reply = (await ask({ v: 1, type: 'page.feed', url: f.href }).catch(() => null)) as {
        type?: string
      } | null
      if (reply?.type === 'page.feed.opened') {
        window.close()
      } else {
        b.disabled = false
        said.textContent =
          "Fuselane isn't running, or this version can't follow feeds yet. Open it and try again."
      }
    })
    feedBox.append(b)
  }
  if (!items.length) {
    said.textContent = 'No videos or file links here.'
    return
  }
  for (const m of items) {
    const kind = KIND[m.kind]
    const button = el(
      'button',
      { type: 'button', className: 'icon-btn', title: 'Download' },
      icon('download-simple'),
    )
    button.setAttribute('aria-label', `Download ${m.name}`)
    button.addEventListener('click', async () => {
      button.disabled = true
      const took = await offerLink(m.url, tab.url, ask, getSession, (u) =>
        browser.downloads.download({ url: u }),
      )
      button.replaceChildren(icon('check'))
      button.dataset.done = took ? 'app' : 'browser'
      said.textContent = took
        ? `${m.name} is downloading in Fuselane.`
        : `Fuselane isn't running, so the browser is downloading ${m.name}.`
    })
    list.append(
      el(
        'li',
        {},
        el('span', { className: 'm-ic' }, icon(kind.icon)),
        el(
          'span',
          { className: 'm-text' },
          el('span', { className: 'm-name', title: m.url }, m.name),
          el('span', { className: 'm-kind' }, m.label ? `${kind.word} · ${m.label}` : kind.word),
        ),
        button,
      ),
    )
  }
}

fillIcons()
void load()
void onThisPage()
