import { formatSize, normalizeRules } from '@fuselane/capture'
import { appStatus, browserId, sendHandoff, type Status } from '../../lib/status.ts'
import { findOnPage, mediaList, worthPeeking, type MediaItem } from '../../lib/media.ts'
import { feedList, findFeeds, type FoundFeed } from '../../lib/feeds.ts'
import { KEY, readJobs, type SelfJob } from '../../lib/selfjobs.ts'
import { cleanLimit } from '../../lib/route.ts'
import { DIAL_STOPS, dialFraction, mbps, speedTest } from '../../lib/speed.ts'
import { fillIcons, icon, type IconName } from './icons.ts'
import './style.css'

// The popup (REVIEW-2026-10-11 D): the Fuselane switch, the downloads (the
// app's, or the extension's own when there's no app) with their buttons and
// the combined speed, this page's video and files, and a speed test.

const HOST = 'app.fuselane.host'
const DOWNLOAD_PAGE = 'https://fuselane.app/download/?ref=extension'

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!
const chip = $<HTMLSpanElement>('#chip')
const status = $<HTMLParagraphElement>('#status')
const cards = $<HTMLElement>('#cards')

const ask = (m: unknown) => browser.runtime.sendNativeMessage(HOST, m as object)
const hello = () => ({
  browser: browserId(navigator.userAgent, 'brave' in navigator),
  version: browser.runtime.getManifest().version,
})

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> = {},
  ...kids: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const e = Object.assign(document.createElement(tag), props)
  e.append(...kids)
  return e
}

// ---- A connection to the app while the popup is open ----------------------
// One native host for the popup's life: the downloads list refreshes several
// times a second without starting a new host each time.
let port: Browser.runtime.Port | null = null
const waiting: ((v: unknown) => void)[] = []
function openPort() {
  try {
    port = browser.runtime.connectNative(HOST)
    port.onMessage.addListener((m) => waiting.shift()?.(m))
    port.onDisconnect.addListener(() => {
      port = null
      for (const w of waiting.splice(0)) w(null)
    })
  } catch {
    port = null
  }
}
function request(m: object): Promise<unknown> {
  if (!port) return ask(m).catch(() => null)
  return new Promise((resolve) => {
    waiting.push(resolve)
    port!.postMessage(m)
  })
}

// ---- The Fuselane switch --------------------------------------------------
let app: Status = { state: 'not-installed', text: '' }
/** Resolves once the app's state is known (the page tab waits for it). */
let knowApp: (s: Status) => void = () => {}
const appKnown = new Promise<Status>((r) => (knowApp = r))

async function setOn(on: boolean) {
  const { rules } = await browser.storage.local.get('rules')
  const next = { ...normalizeRules(rules), enabled: on, minBytes: 0 }
  // The app keeps it too, so every browser agrees; if it's closed, it waits.
  const delivered = app.state === 'connected' && (await sendHandoff(ask, next))
  await browser.storage.local.set({
    rules: next,
    handoffPending: !delivered,
    ...(delivered ? { handoffSynced: true } : {}),
  })
  showOn(on)
}

function showOn(on: boolean) {
  $('#off').hidden = on
  $('#on').hidden = !on
  for (const e of document.querySelectorAll<HTMLElement>('.on-only')) e.hidden = !on
  $<HTMLButtonElement>('#turn-on').setAttribute('aria-checked', String(on))
}

$<HTMLButtonElement>('#power').addEventListener('click', () => void setOn(false))
$<HTMLButtonElement>('#turn-on').addEventListener('click', () => void setOn(true))

for (const link of [$<HTMLAnchorElement>('#settings'), $<HTMLAnchorElement>('#more-settings')]) {
  link.addEventListener('click', (e) => {
    e.preventDefault()
    void browser.runtime.openOptionsPage()
    window.close()
  })
}

// ---- Cards: no app, or no site access yet ---------------------------------
/** Downloads too big for the extension, asked about in this popup (D). */
async function bigCards(): Promise<HTMLElement[]> {
  const v = (await browser.storage.local.get('bigAsks')).bigAsks
  const asks = (Array.isArray(v) ? v : []) as { url: string; name: string; size: number }[]
  return asks
    .slice()
    .reverse()
    .map((ask) => {
      const done = (choice: 'app' | 'browser') => {
        void browser.runtime
          .sendMessage({ type: 'decide', choice, url: ask.url })
          .finally(() => window.close())
      }
      const get = el(
        'button',
        { type: 'button', className: 'btn btn-primary btn-block' },
        icon('download-simple'),
        'Get the free app',
      )
      get.addEventListener('click', () => done('app'))
      const here = el(
        'button',
        { type: 'button', className: 'btn btn-block' },
        icon('globe'),
        'Download in the browser',
      )
      here.addEventListener('click', () => done('browser'))
      const c = card(
        'desktop',
        'This one needs the Fuselane app',
        `${ask.name}${ask.size > 0 ? ` is ${formatSize(ask.size)}, ` : ' is '}more than the extension downloads by itself. The free app takes any size and splits it across Wi‑Fi, Ethernet and your phone at once.`,
        el('div', { className: 'card-actions' }, get, here),
      )
      c.classList.add('card-ask')
      return c
    })
}

/** A link already in Fuselane, paused or stopped: Resume it, or another copy. */
async function sameCard(): Promise<HTMLElement | null> {
  const same = (await browser.storage.local.get('sameAsk')).sameAsk as
    | {
        id: number
        name: string
        status: string
        written: number
        total: number | null
        at: number
      }
    | undefined
  if (!same || Date.now() - same.at > 30 * 60_000) return null
  const pct = same.total ? ` at ${Math.floor((same.written / same.total) * 100)}%` : ''
  const where = same.status === 'paused' ? `paused${pct}` : `stopped${pct}`
  const finish = async (action: 'resume' | 'again') => {
    resume.disabled = again.disabled = true
    const reply = (await ask({ v: 1, type: 'job.action', id: same.id, action }).catch(
      () => null,
    )) as { error?: string } | null
    if (!reply || reply.error) {
      // Gone from the list since, or the app closed: say so, nothing to choose.
      c.querySelector('.card-actions')!.replaceChildren(
        el(
          'p',
          { className: 'muted small', role: 'status' },
          reply
            ? 'That download isn’t in Fuselane any more.'
            : 'Fuselane didn’t answer. Try again.',
        ),
      )
      if (reply) await browser.storage.local.remove('sameAsk')
      else setTimeout(() => void showCards(), 3000)
      return
    }
    await browser.storage.local.remove('sameAsk')
    c.remove()
  }
  const resume = el(
    'button',
    { type: 'button', className: 'btn btn-primary btn-block' },
    icon('play'),
    'Resume it',
  )
  resume.addEventListener('click', () => void finish('resume'))
  const again = el(
    'button',
    { type: 'button', className: 'btn btn-block' },
    icon('download-simple'),
    'Download it again',
  )
  again.addEventListener('click', () => void finish('again'))
  const c = card(
    'download-simple',
    'Already in Fuselane',
    `${same.name} is in your downloads, ${where}. Carry on with it, or download a new copy?`,
    el('div', { className: 'card-actions' }, resume, again),
  )
  c.classList.add('card-ask')
  return c
}

async function showCards() {
  cards.replaceChildren()
  const same = await sameCard()
  if (same) cards.append(same)
  const big = await bigCards()
  cards.append(...big)
  const access = await browser.permissions.contains({ origins: ['<all_urls>'] })
  // A question about a big file already offers the app.
  if (app.state === 'not-installed' && !big.length) {
    const limit = cleanLimit((await browser.storage.local.get('selfLimit')).selfLimit)
    cards.append(
      card(
        'desktop',
        'Get the free Fuselane app',
        `It splits every download across Wi‑Fi, Ethernet and your phone. Until then, the extension downloads files up to ${formatSize(limit)}.`,
        el(
          'a',
          {
            className: 'btn btn-primary btn-block',
            href: DOWNLOAD_PAGE,
            target: '_blank',
            rel: 'noopener',
          },
          icon('download-simple'),
          'Get the app',
        ),
      ),
    )
  }
  if (!access) {
    const allow = el(
      'button',
      { type: 'button', className: 'btn btn-block' },
      icon('lock-open'),
      'Allow Fuselane on all sites',
    )
    allow.addEventListener('click', async () => {
      const ok = await browser.permissions.request({ origins: ['<all_urls>'] }).catch(() => false)
      if (ok) void showCards()
    })
    cards.append(
      card(
        'lock-open',
        'Catch downloads before the browser',
        'Fuselane takes file links as you click them, so the browser never starts them. It only reads links you click.',
        allow,
      ),
    )
  }
  cards.hidden = !cards.childElementCount
}

function card(ic: IconName, title: string, text: string, action: Node): HTMLElement {
  return el(
    'div',
    { className: 'card' },
    el(
      'div',
      { className: 'card-head' },
      el('span', { className: 'card-ic' }, icon(ic)),
      el(
        'div',
        {},
        el('p', { className: 'card-title' }, title),
        el('p', { className: 'muted small' }, text),
      ),
    ),
    action,
  )
}

// ---- Tabs -----------------------------------------------------------------
const TABS = ['downloads', 'page', 'speed'] as const
type Tab = (typeof TABS)[number]
function show(tab: Tab, focus = false) {
  for (const t of TABS) {
    const b = $<HTMLButtonElement>(`#tab-${t}`)
    const on = t === tab
    b.setAttribute('aria-selected', String(on))
    b.tabIndex = on ? 0 : -1
    $(`#panel-${t}`).hidden = !on
    if (on && focus) b.focus()
  }
  try {
    localStorage.setItem('tab', tab)
  } catch {
    // a remembered tab is only a convenience
  }
}
for (const t of TABS) $<HTMLButtonElement>(`#tab-${t}`).addEventListener('click', () => show(t))
$('.tabs').addEventListener('keydown', (e) => {
  const i = TABS.findIndex((t) => $(`#tab-${t}`).getAttribute('aria-selected') === 'true')
  if (e.key === 'ArrowRight') show(TABS[(i + 1) % TABS.length]!, true)
  else if (e.key === 'ArrowLeft') show(TABS[(i + TABS.length - 1) % TABS.length]!, true)
  else return
  e.preventDefault()
})

// ---- Downloads ------------------------------------------------------------
interface Row {
  id: string
  name: string
  status: string
  written: number
  total: number | null
  rate: number
  error: string | null
  file: boolean
  /** The networks carrying it, with their bytes so far (from the app). */
  lanes?: { label: string; bytes: number }[]
}

/** A network's lane colour, the same one wherever it shows. */
const LANE_COLOURS = ['tide', 'volt', 'iris', 'rose', 'mint', 'sky', 'lilac']
const laneOf = new Map<string, string>()
function laneColour(label: string): string {
  if (!laneOf.has(label)) laneOf.set(label, LANE_COLOURS[laneOf.size % LANE_COLOURS.length]!)
  return `var(--lane-${laneOf.get(label)})`
}

/** What kind of file, by its name: its badge on the tile. */
function fileIcon(name: string): IconName {
  const ext = name.toLowerCase().split('.').pop() ?? ''
  if (['iso', 'img', 'dmg', 'appimage', 'deb', 'rpm', 'pkg', 'msi', 'exe'].includes(ext))
    return 'disc'
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar', 'zst', 'xz', 'bz2'].includes(ext)) return 'file-zip'
  if (['mp4', 'mkv', 'mov', 'webm', 'avi', 'm4v'].includes(ext)) return 'film-strip'
  if (['mp3', 'flac', 'm4a', 'wav', 'ogg', 'aac'].includes(ext)) return 'music-note'
  return 'file'
}

const speed = (bps: number) => `${formatSize(Math.max(0, bps))}/s`
const ACTIVE = new Set(['running', 'queued', 'paused', 'saving'])

function meta(r: Row): string {
  const of = r.total ? `${formatSize(r.written)} of ${formatSize(r.total)}` : formatSize(r.written)
  switch (r.status) {
    case 'running':
      // The percentage (bold) and speed (accent) are drawn around this (render).
      return of
    case 'queued':
      return r.written ? `Waiting, ${of}` : 'Waiting'
    case 'paused':
      return `Paused, ${of}`
    case 'saving':
      return 'Saving to your Downloads folder…'
    case 'completed':
      return `Done, ${formatSize(r.total ?? r.written)}`
    case 'too-big':
      return 'Too big for the extension: get the app, or let the browser download it.'
    default:
      return r.error ? `Failed: ${r.error}` : 'Failed'
  }
}

function buttons(r: Row): [IconName, string, string][] {
  if (r.status === 'running' || r.status === 'queued')
    return [
      ['pause', 'pause', 'Pause'],
      ['x', 'cancel', 'Cancel'],
    ]
  if (r.status === 'paused' || r.status === 'failed')
    return [
      ['play', 'resume', 'Resume'],
      ['x', 'cancel', 'Cancel'],
    ]
  if (r.status === 'completed' && r.file)
    return [
      ['arrow-square-out', 'open', 'Open'],
      ['folder-open', 'reveal', 'Show in folder'],
      ['x', 'remove', 'Remove from the list'],
    ]
  return [['x', 'remove', 'Remove from the list']]
}

let act: (id: string, action: string) => Promise<void> = async () => {}

/** "Cancel this download?" in its row: Keep it, or cancel and delete the part. */
function askToCancel(li: HTMLElement, actions: HTMLElement, id: string) {
  if (li.querySelector('.job-confirm')) return
  const keep = el('button', { type: 'button', className: 'btn btn-small' }, 'Keep it')
  const drop = el(
    'button',
    { type: 'button', className: 'btn btn-small btn-danger' },
    'Cancel download',
  )
  const box = el(
    'div',
    { className: 'job-confirm', role: 'group' },
    el('span', {}, 'Cancel it? What’s downloaded is deleted.'),
    el('span', { className: 'job-confirm-actions' }, keep, drop),
  )
  box.setAttribute('aria-label', 'Cancel this download?')
  actions.hidden = true
  li.classList.add('asking')
  li.append(box)
  keep.addEventListener('click', () => {
    box.remove()
    li.classList.remove('asking')
    actions.hidden = false
  })
  drop.addEventListener('click', async () => {
    keep.disabled = drop.disabled = true
    await act(id, 'cancel').catch(() => {})
    // Gone, the row goes with it. Still here a moment later: it didn't take,
    // so the choice is offered again.
    setTimeout(() => {
      if (box.isConnected) keep.disabled = drop.disabled = false
    }, 3000)
  })
  drop.focus()
}
const rows = new Map<string, { li: HTMLLIElement; state: string }>()
/** Pause pressed: shown at once, until the download says it has stopped. */
const pausing = new Map<string, number>()

/** The speed card: all downloads together, and each network's share. */
function summary(running: number, rate: number, networks: { label: string; rate: number }[]) {
  const card = $('#summary')
  card.hidden = running === 0
  if (!running) return
  const [num, unit] = speed(rate).split(' ')
  $('#sum-rate').textContent = num ?? '0'
  $('#sum-unit').textContent = unit ?? 'B/s'
  $('#sum-what').textContent = running === 1 ? '1 downloading' : `${running} downloading`
  const nets = networks.filter((n) => n.rate > 0)
  const lanes = $('#sum-lanes')
  const keys = $('#sum-keys')
  lanes.hidden = keys.hidden = nets.length === 0
  lanes.replaceChildren(
    ...nets.map((n) => {
      const seg = el('span')
      seg.style.flex = String(n.rate)
      seg.style.background = laneColour(n.label)
      return seg
    }),
  )
  keys.replaceChildren(
    ...nets.map((n) => {
      const k = el('span', { className: 'lane-key' }, el('b', {}, n.label), ` ${speed(n.rate)}`)
      k.style.setProperty('--k', laneColour(n.label))
      return k
    }),
  )
}

function render(list: Row[], rate: number, networks: { label: string; rate: number }[] = []) {
  const ul = $<HTMLUListElement>('#jobs')
  const doneUl = $<HTMLUListElement>('#jobs-done')
  const active = list.filter((r) => ACTIVE.has(r.status))
  const running = list.filter((r) => r.status === 'running').length
  summary(running, rate, networks)
  const line = $('#speed-line')
  line.textContent = running ? '' : active.length ? `${active.length} waiting or paused` : ''
  const count = $('#tab-count')
  count.hidden = !active.length
  count.textContent = String(active.length)
  $('#jobs-empty').hidden = list.length > 0
  const finished = list.filter((r) => r.status === 'completed')
  $('#done-label').hidden = finished.length === 0 || finished.length === list.length
  const seen = new Set<string>()
  const placed = new Map<HTMLUListElement, number>()
  for (const r of list) {
    seen.add(r.id)
    const into = r.status === 'completed' ? doneUl : ul
    const key = `${r.status}|${r.file}`
    let row = rows.get(r.id)
    if (!row || row.state !== key) {
      const li = el('li', { className: 'job' })
      li.dataset.status = r.status
      const actions = el('span', { className: 'job-actions' })
      for (const [ic, action, label] of buttons(r)) {
        const b = el('button', { type: 'button', className: 'icon-btn', title: label }, icon(ic))
        b.setAttribute('aria-label', `${label}: ${r.name}`)
        b.addEventListener('click', async () => {
          // Cancelling throws away what's downloaded: ask first, in the row.
          if (action === 'cancel') return askToCancel(li, actions, r.id)
          b.disabled = true
          if (action === 'pause') {
            // Its connections take a moment to close: say so now.
            pausing.set(r.id, Date.now())
            li.querySelector('.job-meta')!.textContent = 'Pausing…'
            for (const x of actions.querySelectorAll('button')) x.disabled = true
          }
          await act(r.id, action)
          if (action !== 'pause') b.disabled = false
        })
        actions.append(b)
      }
      const badge = r.status === 'completed' ? 'check' : fileIcon(r.name)
      li.append(
        el('span', { className: 'job-ic' }, icon(badge)),
        el('span', { className: 'job-name', title: r.name }, r.name),
        actions,
        el('span', { className: 'job-meta mono' }),
        el('span', { className: 'bar' }),
      )
      if (row) row.li.remove()
      row = { li, state: key }
      rows.set(r.id, row)
    }
    const { li } = row
    const asked = pausing.get(r.id)
    const stillPausing =
      asked !== undefined && r.status === 'running' && Date.now() - asked < 20_000
    if (!stillPausing && asked !== undefined) {
      // Still running after all (the pause didn't take): its buttons work again.
      pausing.delete(r.id)
      for (const x of li.querySelectorAll('button')) x.disabled = false
    }
    const pct = r.total
      ? Math.min(100, (r.written / r.total) * 100)
      : r.status === 'completed'
        ? 100
        : 0
    const metaEl = li.querySelector('.job-meta')!
    const bold =
      r.total && r.status !== 'completed'
        ? el('b', { className: 'pct' }, `${Math.floor(pct)}%`)
        : null
    if (stillPausing) metaEl.textContent = 'Pausing…'
    else if (r.status === 'running')
      // No speed yet: it's checking what it had and reconnecting (a few seconds).
      metaEl.replaceChildren(
        ...(bold ? [bold, ' '] : []),
        `${meta(r)} `,
        r.rate > 0 ? el('span', { className: 'rate' }, speed(r.rate)) : 'connecting…',
      )
    else metaEl.replaceChildren(...(bold ? [bold, ' '] : []), meta(r))
    li.querySelector('.job-name')!.textContent = r.name
    // The bar, coloured by the networks carrying it (one colour without the app).
    const bar = li.querySelector<HTMLElement>('.bar')!
    const lanes = (r.lanes ?? []).filter((l) => l.bytes > 0)
    const sum = lanes.reduce((n, l) => n + l.bytes, 0)
    const parts =
      r.status === 'running' && lanes.length > 1 && sum > 0
        ? lanes.map((l) => ({ w: (pct * l.bytes) / sum, c: laneColour(l.label) }))
        : [{ w: pct, c: '' }]
    if (bar.childElementCount !== parts.length)
      bar.replaceChildren(...parts.map(() => el('span', { className: 'bar-fill' })))
    parts.forEach((p, i) => {
      const seg = bar.children[i] as HTMLElement
      seg.style.width = `${p.w}%`
      seg.style.background = p.c
    })
    const at = placed.get(into) ?? 0
    if (into.children[at] !== li) into.insertBefore(li, into.children[at] ?? null)
    placed.set(into, at + 1)
  }
  for (const [id, row] of rows) {
    if (!seen.has(id)) {
      row.li.remove()
      rows.delete(id)
    }
  }
}

/** With the app: its list, a few times a second, over the popup's own host. */
async function followApp() {
  act = async (id, action) => {
    const reply = (await request({ v: 1, type: 'job.action', id: Number(id), action })) as {
      error?: string
    } | null
    if (reply?.error) $('#speed-line').textContent = reply.error
    await pollApp()
  }
  await pollApp()
  setInterval(() => void pollApp(), 700)
}
async function pollApp() {
  const reply = (await request({ v: 1, type: 'jobs.list' })) as {
    jobs?: Row[]
    rate?: number
    networks?: { label: string; rate: number }[]
  } | null
  if (!reply?.jobs) return
  render(
    reply.jobs.map((j) => ({ ...j, id: String(j.id) })),
    reply.rate ?? 0,
    reply.networks ?? [],
  )
}

/** Without the app: the extension's own downloads, as they change. */
async function followSelf() {
  act = async (id, action) => {
    await browser.runtime.sendMessage({ type: 'self.action', id, action })
  }
  const draw = (jobs: SelfJob[]) =>
    render(
      jobs.map((j) => ({
        id: j.id,
        name: j.name,
        status: j.status,
        written: j.written,
        total: j.total,
        rate: j.rate,
        error: j.error,
        file: j.savedId !== null,
      })),
      jobs.reduce((n, j) => n + (j.status === 'running' ? j.rate : 0), 0),
    )
  draw(readJobs((await browser.storage.local.get(KEY))[KEY]))
  browser.storage.onChanged.addListener((c, area) => {
    if (area === 'local' && c[KEY]) draw(readJobs(c[KEY].newValue))
  })
}

// ---- This page ------------------------------------------------------------
const KIND: Record<MediaItem['kind'], { word: string; icon: IconName }> = {
  video: { word: 'Video', icon: 'video-camera' },
  audio: { word: 'Audio', icon: 'music-note' },
  link: { word: 'File', icon: 'file' },
}

async function onThisPage() {
  const list = $<HTMLUListElement>('#media')
  const said = $<HTMLParagraphElement>('#media-status')
  const video = $<HTMLButtonElement>('#video')
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true })
  if (!tab?.id || !tab.url || !/^https?:/.test(tab.url)) {
    $('#page-card').hidden = true
    said.textContent = "This page isn't a website, so there's nothing to get here."
    return
  }
  const pageUrl = tab.url
  $('#page-title').textContent = tab.title || new URL(pageUrl).hostname
  $('#page-site').textContent = new URL(pageUrl).hostname.replace(/^www\./, '')
  // A video page (YouTube and many more): the app finds the video and its
  // qualities. It opens Fuselane if it's closed, so this can take a moment.
  video.addEventListener('click', async () => {
    video.disabled = true
    said.textContent =
      app.state === 'connected' ? 'Opening the video in Fuselane…' : 'Starting Fuselane…'
    const reply = (await ask({ v: 1, type: 'page.video', url: pageUrl }).catch(() => null)) as {
      type?: string
    } | null
    if (reply?.type === 'page.video.opened') {
      window.close()
    } else {
      video.disabled = false
      said.textContent =
        app.state === 'not-installed'
          ? 'Getting videos needs the Fuselane app.'
          : "Fuselane didn't answer. Open it and try again."
    }
  })
  let items: MediaItem[] = []
  let feeds: FoundFeed[] = []
  try {
    const [res] = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: findOnPage,
    })
    items = mediaList((res?.result as Parameters<typeof mediaList>[0]) ?? [], pageUrl)
    const [f] = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: findFeeds,
    })
    feeds = feedList((f?.result as FoundFeed[]) ?? [], pageUrl)
  } catch {
    // a page the browser doesn't let extensions read
  }
  // A video page and the app running: it starts reading the page now, so the
  // qualities are ready (or nearly) when "Get this page's video" is pressed (B3).
  // Never starts the app: a closed app isn't opened just for a look.
  if (worthPeeking(pageUrl, items))
    void appKnown.then((a) => {
      if (a.state === 'connected')
        void ask({ v: 1, type: 'page.peek', url: pageUrl }).catch(() => {})
    })
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
      if (reply?.type === 'page.feed.opened') window.close()
      else {
        b.disabled = false
        said.textContent = "Fuselane didn't answer. Open it and try again."
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
      await browser.runtime.sendMessage({ type: 'grab', url: m.url, page: pageUrl, name: null })
      button.replaceChildren(icon('check'))
      said.textContent = `${m.name} is on its way. It shows under Downloads.`
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
          el('span', { className: 'm-kind' }, m.label ? `${kind.word}, ${m.label}` : kind.word),
        ),
        button,
      ),
    )
  }
}

// ---- Speed test -----------------------------------------------------------
// Evenly spaced numbers to 1 Gbps (lib/speed.ts), so everyday speeds use most
// of the dial and every number has room.
// The dial: 240°, from lower left round to lower right.
const DIAL = { cx: 125, cy: 100, r: 84, start: 150, sweep: 240 }
function dialPoint(deg: number, r: number): [number, number] {
  const a = (deg * Math.PI) / 180
  return [DIAL.cx + r * Math.cos(a), DIAL.cy + r * Math.sin(a)]
}
function dialArc(): string {
  const [x1, y1] = dialPoint(DIAL.start, DIAL.r)
  const [x2, y2] = dialPoint(DIAL.start + DIAL.sweep, DIAL.r)
  return `M${x1.toFixed(1)} ${y1.toFixed(1)} A${DIAL.r} ${DIAL.r} 0 1 1 ${x2.toFixed(1)} ${y2.toFixed(1)}`
}
function needle(bps: number) {
  const f = dialFraction((Math.max(0, bps) * 8) / 1e6)
  const fill = $<HTMLElement>('#gauge-fill')
  fill.style.strokeDasharray = `${f * 100} 100`
  // An empty arc would still draw its rounded end as a dot.
  fill.style.opacity = f > 0.005 ? '1' : '0'
  for (const t of document.querySelectorAll<SVGTextElement>('#gauge-ticks text'))
    t.classList.toggle('on', Number(t.dataset.f) <= f + 1e-9 && f > 0)
  $('#gauge-value').textContent = mbps(bps)
}
function ticks() {
  $('#gauge-track').setAttribute('d', dialArc())
  $('#gauge-fill').setAttribute('d', dialArc())
  const g = $<HTMLElement>('#gauge-ticks')
  for (const m of DIAL_STOPS) {
    const f = dialFraction(m)
    const [x, y] = dialPoint(DIAL.start + DIAL.sweep * f, DIAL.r - 22)
    const t = document.createElementNS('http://www.w3.org/2000/svg', 'text')
    t.setAttribute('x', x.toFixed(1))
    t.setAttribute('y', (y + 3.5).toFixed(1))
    t.setAttribute('class', 'gauge-tick')
    t.dataset.f = String(f)
    t.textContent = m >= 1000 ? '1G' : String(m)
    g.append(t)
  }
}

function results(entries: [string, string][]) {
  const dl = $<HTMLDListElement>('#speed-results')
  dl.replaceChildren(...entries.map(([k, v]) => el('div', {}, el('dt', {}, k), el('dd', {}, v))))
  dl.hidden = !entries.length
}

let testing: AbortController | null = null
async function runSpeedTest() {
  const go = $<HTMLButtonElement>('#speed-go')
  const step = $('#speed-step')
  if (testing) {
    testing.abort()
    return
  }
  results([])
  go.replaceChildren(icon('x'), 'Stop')
  if (app.state === 'connected') {
    // The app tests every network on its own, then all of them together.
    testing = new AbortController()
    const stop = testing
    await request({ v: 1, type: 'speedtest.start' })
    $('#speed-note').textContent = 'Fuselane tests each network, then all of them together.'
    while (!stop.signal.aborted) {
      const r = (await request({ v: 1, type: 'speedtest.state' })) as {
        state?: {
          running: boolean
          phase?: string | null
          live?: { bps: number } | null
          run?: {
            results: { label: string; downBps: number | null; upBps: number | null }[]
            togetherBps: number | null
          } | null
        }
      } | null
      const s = r?.state
      if (!s) break
      needle(s.live?.bps ?? 0)
      step.textContent = s.phase ?? (s.running ? 'Testing…' : 'Done.')
      if (s.run) {
        results([
          ...s.run.results.map((n): [string, string] => [
            n.label,
            `${mbps(n.downBps)} down, ${mbps(n.upBps)} up Mbps`,
          ]),
          ...(s.run.togetherBps
            ? [['All together', `${mbps(s.run.togetherBps)} Mbps`] as [string, string]]
            : []),
        ])
      }
      if (!s.running) break
      await new Promise((r) => setTimeout(r, 250))
    }
  } else {
    testing = new AbortController()
    $('#speed-note').textContent =
      'Without the app, the browser can only test the connection it uses.'
    const words = { ping: 'Checking the delay…', down: 'Download speed…', up: 'Upload speed…' }
    try {
      const r = await speedTest((x) => {
        needle(x.bps)
        step.textContent = words[x.step]
      }, testing.signal)
      needle(r.downBps ?? 0)
      step.textContent = 'Done.'
      results([
        ['Download', `${mbps(r.downBps)} Mbps`],
        ['Upload', `${mbps(r.upBps)} Mbps`],
        ['Delay', r.pingMs ? `${Math.round(r.pingMs)} ms` : 'unknown'],
      ])
    } catch {
      step.textContent = testing.signal.aborted
        ? 'Stopped.'
        : "The test couldn't reach the speed server."
    }
  }
  testing = null
  go.replaceChildren(icon('gauge'), 'Test again')
}
$<HTMLButtonElement>('#speed-go').addEventListener('click', () => void runSpeedTest())

// ---- Start ----------------------------------------------------------------
async function load() {
  const { rules } = await browser.storage.local.get('rules')
  showOn(normalizeRules(rules).enabled)
  let tab: Tab = 'downloads'
  try {
    const t = localStorage.getItem('tab') as Tab | null
    if (t && TABS.includes(t)) tab = t
  } catch {
    // ignore
  }
  // Opened by a download that just started: straight to Downloads.
  const started = (await browser.storage.local.get('showDownloads')).showDownloads
  if (typeof started === 'number' && Date.now() - started < 10_000) {
    tab = 'downloads'
    void browser.storage.local.remove('showDownloads')
  }
  show(tab)
  // A question waiting (a file too big for the extension) shows at once,
  // before the app's state is known.
  cards.prepend(...(await bigCards()))
  const same = await sameCard()
  if (same) cards.prepend(same)
  app = await appStatus(ask, hello())
  knowApp(app)
  void browser.runtime.sendMessage({ type: 'app-changed' }).catch(() => {})
  status.textContent = app.text
  chip.dataset.state = app.state
  chip.textContent =
    app.state === 'connected' ? 'Connected' : app.state === 'not-running' ? 'App closed' : 'No app'
  chip.title = app.text
  await showCards()
  if (app.state === 'connected') {
    openPort()
    await followApp()
  } else {
    if (app.state === 'not-running') {
      $('#jobs-empty-text').textContent =
        'Fuselane is closed. Your next download opens it and shows up here.'
    }
    await followSelf()
  }
}

fillIcons()
ticks()
needle(0)
void load()
void onThisPage()
