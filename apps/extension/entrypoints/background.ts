import { checkReply, normalizeRules, type Rules } from '@fuselane/capture'
import { handOff, offerFor, offerLink, type Item } from '../lib/handoff.ts'
import { getSession } from '../lib/session.ts'
import { appStatus, browserId, mustPush, settle, type AppState } from '../lib/status.ts'
import { cleanLimit, route } from '../lib/route.ts'
import { nameFor } from '../lib/fetcher.ts'
import { KEY, newJob, readJobs, tidy, type SelfJob } from '../lib/selfjobs.ts'
import { createRunner, type Runner, type RunnerEvent } from '../lib/runner.ts'

// Who downloads what (REVIEW-2026-10-11 D): the app when it's installed (started
// if closed), else the extension itself up to its limit, else the browser.

const HOST = 'app.fuselane.host'
const GET_APP = 'https://fuselane.app/download/?ref=extension'
/** An offer may need the app to start first (the host waits up to 20 s for it). */
const START_WAIT_MS = 25_000
const RUNNING_WAIT_MS = 8_000

const ask = (message: unknown) => browser.runtime.sendNativeMessage(HOST, message as object)

async function rules(): Promise<Rules> {
  const { rules } = await browser.storage.local.get('rules')
  // Cleaned: stored rules may come from an older version or a hand edit.
  return normalizeRules(rules)
}

async function limit(): Promise<number> {
  return cleanLimit((await browser.storage.local.get('selfLimit')).selfLimit)
}

const siteAccess = () => browser.permissions.contains({ origins: ['<all_urls>'] })

const hello = () => ({
  browser: browserId(navigator.userAgent, 'brave' in navigator),
  version: browser.runtime.getManifest().version,
})

// A running app is remembered briefly, so each download doesn't wait for a ping.
// "Not there" is never remembered: the app may have just been installed or opened.
let known: { state: AppState; at: number } | null = null
async function appState(): Promise<AppState> {
  if (known?.state === 'connected' && Date.now() - known.at < 30_000) return known.state
  const s = await appStatus(ask, hello())
  known = { state: s.state, at: Date.now() }
  return s.state
}

const downloads = {
  pause: (id: number) => browser.downloads.pause(id),
  resume: (id: number) => browser.downloads.resume(id),
  cancel: (id: number) => browser.downloads.cancel(id),
  erase: async (id: number) => {
    await browser.downloads.erase({ id })
  },
}

function item(d: Browser.downloads.DownloadItem): Item {
  return {
    id: d.id,
    state: d.state,
    url: d.url,
    finalUrl: d.finalUrl,
    referrer: d.referrer,
    filename: d.filename,
    mime: d.mime,
    size: d.totalBytes > 0 ? d.totalBytes : d.fileSize,
  }
}

// ---- The browser's own download bar ---------------------------------------
// While Fuselane is on, Chrome and Edge don't flash their bar for downloads it
// takes (downloads.ui). Firefox has no such switch.
const setUi = (browser.downloads as { setUiOptions?: (o: { enabled: boolean }) => Promise<void> })
  .setUiOptions
async function browserBar(show: boolean) {
  await setUi?.call(browser.downloads, { enabled: show }).catch(() => {})
}
/** The switch as last read, so a new download can be judged without waiting. */
let switchOn: boolean | null = null
async function followSwitch() {
  switchOn = (await rules()).enabled
  await browserBar(!switchOn)
}
/** Shows the bar for a download the browser keeps, then hides it again. */
async function showBarFor(ms = 8000) {
  if (!(await rules()).enabled) return
  await browserBar(true)
  setTimeout(() => void followSwitch(), ms)
}

// ---- Links we let the browser download on purpose -------------------------
const letThrough = new Set<string>()
async function browserDownload(url: string) {
  letThrough.add(url)
  setTimeout(() => letThrough.delete(url), 60_000)
  await showBarFor()
  await browser.downloads.download({ url }).catch(() => {})
}

// ---- The extension's own downloads ----------------------------------------
let queue: Promise<unknown> = Promise.resolve()
/** Every change to the list goes through here, one at a time. */
function update(fn: (jobs: SelfJob[]) => SelfJob[] | void): Promise<SelfJob[]> {
  const next = queue.then(async () => {
    const jobs = readJobs((await browser.storage.local.get(KEY))[KEY])
    const out = tidy(fn(jobs) ?? jobs)
    await browser.storage.local.set({ [KEY]: out })
    return out
  })
  queue = next.catch(() => {})
  return next
}
const patch = (id: string, change: Partial<SelfJob>) =>
  update((jobs) => jobs.map((j) => (j.id === id ? { ...j, ...change } : j)))

interface RunnerLike {
  start(job: SelfJob, limit: number): void
  pause(id: string): void
  cancel(id: string): void
  saved(id: string): void
}

// Chrome runs the downloads in an offscreen page (its service worker can stop at
// any time); Firefox's background page does them itself.
type Offscreen = {
  createDocument(o: { url: string; reasons: string[]; justification: string }): Promise<void>
}
const offscreen = (browser as unknown as { offscreen?: Offscreen }).offscreen
let local: Runner | null = null
async function runner(): Promise<RunnerLike> {
  if (!offscreen) {
    local ??= createRunner((e) => void onRunnerEvent(e))
    return local
  }
  const contexts = await (
    browser.runtime as unknown as { getContexts?: (f: object) => Promise<unknown[]> }
  )
    .getContexts?.({ contextTypes: ['OFFSCREEN_DOCUMENT'] })
    .catch(() => [])
  if (!contexts?.length) {
    await offscreen
      .createDocument({
        url: '/offscreen.html',
        reasons: ['BLOBS'],
        justification:
          'Saves downloads the extension makes when the Fuselane app is not installed.',
      })
      .catch(() => {})
  }
  const send = (m: object) =>
    void browser.runtime.sendMessage({ target: 'offscreen', ...m }).catch(() => {})
  return {
    start: (job, l) => send({ cmd: 'start', job, limit: l }),
    pause: (id) => send({ cmd: 'pause', id }),
    cancel: (id) => send({ cmd: 'cancel', id }),
    saved: (id) => send({ cmd: 'saved', id }),
  }
}

async function startSelf(url: string, pageUrl: string | null, name: string, size: number | null) {
  const job = newJob(url, pageUrl, name, size)
  await update((jobs) => [job, ...jobs])
  ;(await runner()).start(job, await limit())
}

/** Browser download ids of the extension's finished files, while they save. */
const saving = new Map<number, string>()

async function onRunnerEvent(e: RunnerEvent) {
  const jobs = readJobs((await browser.storage.local.get(KEY))[KEY])
  const job = jobs.find((j) => j.id === e.id)
  if (!job) return
  switch (e.type) {
    case 'self.progress':
      await patch(e.id, {
        status: 'running',
        written: e.written,
        rate: e.rate,
        parts: e.parts,
        total: e.total,
        finalUrl: e.finalUrl,
        ...(e.name ? { name: e.name } : {}),
      })
      break
    case 'self.paused':
      await patch(e.id, { status: 'paused', rate: 0, written: e.written, parts: e.parts })
      break
    case 'self.failed':
      // Nothing came at all: the link may need the page's session or a form.
      // The browser can still get it, so it gets it.
      if (job.written === 0) {
        await patch(e.id, {
          status: 'failed',
          rate: 0,
          error: `${e.error} The browser is downloading it instead.`,
        })
        await browserDownload(job.url)
      } else {
        await patch(e.id, { status: 'failed', rate: 0, error: e.error })
      }
      break
    case 'self.too-big':
      await patch(e.id, { status: 'too-big', rate: 0, total: e.size })
      await askAboutBig(job.url, job.name, e.size)
      break
    case 'self.ready': {
      await patch(e.id, { status: 'saving', rate: 0 })
      try {
        const savedId = await browser.downloads.download({
          url: e.blobUrl,
          filename: job.name,
          conflictAction: 'uniquify',
        })
        saving.set(savedId, e.id)
        await patch(e.id, { savedId })
      } catch {
        await patch(e.id, {
          status: 'failed',
          error: "The file couldn't be saved to your Downloads folder.",
        })
      }
      break
    }
  }
}

/** Offers a browser download to the app (its own copy already stopped). True when taken. */
async function offerItem(it: Item): Promise<boolean> {
  const session = await getSession(it.finalUrl || it.url).catch(() => null)
  const reply = await Promise.race([
    ask(offerFor(it, 'auto', session)),
    new Promise((r) => setTimeout(() => r(null), RUNNING_WAIT_MS)),
  ]).catch(() => null)
  const checked = checkReply(reply)
  return checked.ok && checked.value.type === 'download.accepted'
}

/** "Too big for the extension": Get the app, or let the browser have it. */
async function askAboutBig(url: string, name: string, size: number) {
  const q = new URLSearchParams({ url, name, size: String(size) })
  await browser.windows
    .create({ url: `/decide.html?${q}`, type: 'popup', width: 440, height: 360, focused: true })
    .catch(() => browserDownload(url))
}

// ---- Clicks on file links (once the person allowed Fuselane on all sites) --
const GRAB_ID = 'fuselane-grab'
type Scripting = {
  getRegisteredContentScripts?: (f?: { ids?: string[] }) => Promise<{ id: string }[]>
  registerContentScripts?: (s: object[]) => Promise<void>
  unregisterContentScripts?: (f: { ids: string[] }) => Promise<void>
}
async function followAccess() {
  const scripting = browser.scripting as unknown as Scripting
  const on = await siteAccess()
  const have =
    (await scripting.getRegisteredContentScripts?.({ ids: [GRAB_ID] }).catch(() => [])) ?? []
  if (on && !have.length) {
    await scripting
      .registerContentScripts?.([
        {
          id: GRAB_ID,
          matches: ['<all_urls>'],
          js: ['/content-scripts/grab.js'],
          runAt: 'document_start',
          allFrames: true,
        },
      ])
      .catch(() => {})
  } else if (!on && have.length) {
    await scripting.unregisterContentScripts?.({ ids: [GRAB_ID] }).catch(() => {})
  }
}

/** A file link clicked in a page: straight to Fuselane, the browser never starts it. */
async function grab(url: string, pageUrl: string | null, name: string | null) {
  const r = route(
    { url, filename: name ?? undefined },
    {
      rules: await rules(),
      app: await appState(),
      siteAccess: await siteAccess(),
      limit: await limit(),
    },
  )
  if (r.to === 'app') {
    const took = await offerLink(
      url,
      pageUrl ?? undefined,
      ask,
      getSession,
      () => browserDownload(url),
      START_WAIT_MS,
    )
    if (!took) known = null
  } else if (r.to === 'self' || r.to === 'too-big') {
    // Size unknown until it starts: the extension stops past its limit and asks.
    await startSelf(url, pageUrl, nameFor(url, name), null)
  } else {
    await browserDownload(url)
  }
}

// ---- The popup's buttons for the extension's own downloads ---------------
async function selfAction(id: string, action: string) {
  const jobs = readJobs((await browser.storage.local.get(KEY))[KEY])
  const job = jobs.find((j) => j.id === id)
  if (!job) return
  const r = await runner()
  switch (action) {
    case 'pause':
      r.pause(id)
      break
    case 'resume':
      await patch(id, { status: 'queued', error: null })
      r.start({ ...job, status: 'queued' }, await limit())
      break
    case 'cancel':
      r.cancel(id)
      await update((all) => all.filter((j) => j.id !== id))
      break
    case 'remove':
      await update((all) => all.filter((j) => j.id !== id))
      break
    case 'open':
      if (job.savedId !== null) await browser.downloads.open(job.savedId).catch(() => {})
      break
    case 'reveal':
      if (job.savedId !== null) await browser.downloads.show(job.savedId)
      break
    case 'delete':
      if (job.savedId !== null) {
        await browser.downloads.removeFile(job.savedId).catch(() => {})
        await browser.downloads.erase({ id: job.savedId }).catch(() => {})
      }
      await update((all) => all.filter((j) => j.id !== id))
      break
  }
}

/** Says hello to the app (its Browser extension page shows this browser) and syncs the switch. */
async function checkIn() {
  const s = await appStatus(ask, hello())
  known = { state: s.state, at: Date.now() }
  const got = await browser.storage.local.get(['rules', 'handoffPending', 'handoffSynced'])
  const now = await settle(s, await rules(), mustPush(got), ask)
  await browser.storage.local.set({
    // With the app, every download goes to it: no size rule any more.
    rules: { ...now.rules, minBytes: 0 },
    handoffPending: now.pending,
    handoffSynced: got.handoffSynced === true || (s.state === 'connected' && !now.pending),
  })
}

/** After the browser restarts, downloads that were running wait to be resumed. */
async function afterRestart() {
  await update((jobs) =>
    jobs.map((j) =>
      ['running', 'queued', 'saving'].includes(j.status) ? { ...j, status: 'paused', rate: 0 } : j,
    ),
  )
}

export default defineBackground(() => {
  browser.runtime.onStartup.addListener(() => {
    void checkIn()
    void followSwitch()
    void followAccess()
    void afterRestart()
  })

  browser.runtime.onInstalled.addListener(() => {
    void checkIn()
    void followSwitch()
    void followAccess()
    browser.contextMenus.create({
      id: 'fuselane-download',
      title: 'Download with Fuselane',
      contexts: ['link', 'video', 'audio'],
    })
  })

  browser.storage.onChanged.addListener((changes, area) => {
    if (area === 'local' && changes.rules) void followSwitch()
  })
  browser.permissions.onAdded.addListener(() => void followAccess())
  browser.permissions.onRemoved.addListener(() => void followAccess())

  browser.downloads.onCreated.addListener(async (d) => {
    // Our own saves and links we let through on purpose stay with the browser.
    if (
      d.byExtensionId === browser.runtime.id ||
      letThrough.has(d.url) ||
      d.url.startsWith('blob:')
    )
      return
    // The app is running and Fuselane is on: like IDM, the browser's copy stops
    // at once (a pause can't stop a small file that's already on its way), and the
    // link goes to the app. If the app can't take it, the browser gets it again.
    if (
      d.state === 'in_progress' &&
      switchOn === true &&
      known?.state === 'connected' &&
      Date.now() - known.at < 30_000
    ) {
      void downloads
        .cancel(d.id)
        .then(() => downloads.erase(d.id))
        .catch(() => {})
      const r = route(item(d), {
        rules: await rules(),
        app: 'connected',
        siteAccess: false,
        limit: 0,
      })
      if (r.to === 'browser') return void browserDownload(d.finalUrl || d.url)
      const took = await offerItem(item(d))
      if (!took) {
        known = null
        await browserDownload(d.finalUrl || d.url)
      }
      return
    }
    // Otherwise hold it before deciding: deciding takes a moment, and a fast
    // download would otherwise be finished (or flash the browser's bar) by then.
    const held =
      d.state === 'in_progress'
        ? downloads.pause(d.id).then(
            () => true,
            () => false,
          )
        : null
    const s = {
      rules: await rules(),
      app: await appState(),
      siteAccess: await siteAccess(),
      limit: await limit(),
    }
    switchOn = s.rules.enabled
    const r = route(item(d), s)
    if (r.to === 'browser') {
      if (held && (await held)) await downloads.resume(d.id).catch(() => {})
      await showBarFor()
      return
    }
    if (r.to === 'app') {
      const wait = s.app === 'connected' ? RUNNING_WAIT_MS : START_WAIT_MS
      const outcome = await handOff(
        item(d),
        { ...s.rules, minBytes: 0 },
        downloads,
        ask,
        wait,
        getSession,
      )
      if (outcome === 'returned') {
        known = null // the app may have closed: ask again next time
        await showBarFor()
      }
      return
    }
    // The extension takes it: the browser's copy goes, without a trace.
    await downloads.cancel(d.id).catch(() => {})
    await downloads.erase(d.id).catch(() => {})
    const url = d.finalUrl || d.url
    const name = nameFor(url, null, d.filename)
    if (r.to === 'too-big') await askAboutBig(url, name, r.size)
    else await startSelf(url, d.referrer || null, name, item(d).size ?? null)
  })

  // A file the extension saved finished (or failed) in the browser.
  browser.downloads.onChanged.addListener(async (delta) => {
    if (!delta.state?.current) return
    // The worker may have stopped mid-save and lost the map; the job keeps the id.
    const id =
      saving.get(delta.id) ??
      readJobs((await browser.storage.local.get(KEY))[KEY]).find((j) => j.savedId === delta.id)?.id
    if (!id) return
    if (delta.state.current === 'complete') {
      saving.delete(delta.id)
      await patch(id, { status: 'completed' })
      ;(await runner()).saved(id)
    } else if (delta.state.current === 'interrupted') {
      saving.delete(delta.id)
      await patch(id, {
        status: 'failed',
        error: "The file couldn't be saved to your Downloads folder.",
      })
    }
  })

  browser.runtime.onMessage.addListener((m: unknown, sender) => {
    if (sender.id !== browser.runtime.id) return
    const msg = (m ?? {}) as Record<string, unknown>
    if (msg.target === 'offscreen') return // for the offscreen page, not us
    const type = typeof msg.type === 'string' ? msg.type : ''
    if (type.startsWith('self.') && type !== 'self.action') {
      void onRunnerEvent(msg as unknown as RunnerEvent)
    } else if (type === 'grab' && typeof msg.url === 'string') {
      void grab(
        msg.url,
        typeof msg.page === 'string' ? msg.page : null,
        typeof msg.name === 'string' ? msg.name : null,
      )
    } else if (
      type === 'self.action' &&
      typeof msg.id === 'string' &&
      typeof msg.action === 'string'
    ) {
      void selfAction(msg.id, msg.action)
    } else if (type === 'decide' && typeof msg.url === 'string') {
      if (msg.choice === 'browser') void browserDownload(msg.url)
      else void browser.tabs.create({ url: GET_APP })
    } else if (type === 'app-changed') {
      known = null
    }
  })

  browser.contextMenus.onClicked.addListener(async (info) => {
    const url = info.linkUrl ?? info.srcUrl
    if (info.menuItemId !== 'fuselane-download' || !url) return
    await grab(url, info.pageUrl ?? null, null)
  })
})
