// Demo engine for the browser (pnpm dev, Playwright, contact sheets). It mimics the
// real service's behaviour and messages closely enough to exercise every UI state,
// and the UI labels it "Demo data" so it is never mistaken for real transfers.
//
// URL parameters: empty=1 (no sample jobs), speed=N (time multiplier),
// seed=N, freeze=SECONDS (advance to that instant, then stop: for screenshots),
// drop=0 (the phone never drops out), portal=en0 (that network shows a sign-in page).
import type { Backend } from './backend'
import { createDemoTorrents } from './demoTorrents'
import { createDemoSends } from './demoSends'
import { createDemoNearby } from './demoNearby'
import type {
  AllowanceView,
  Automation,
  AutomationView,
  BatchResult,
  JobStatus,
  JobView,
  LimitsView,
  Live,
  LiveNet,
  NetPref,
  NetView,
  PreviewView,
  ReportView,
  UiError,
  UiEvent,
} from './types'

const MB = 1024 * 1024
const TICKS = 180

function rng(seed: number) {
  let s = seed >>> 0 || 1
  return () => {
    s ^= s << 13
    s ^= s >>> 17
    s ^= s << 5
    return (s >>> 0) / 4294967296
  }
}

const NETWORKS: NetView[] = [
  {
    name: 'en0',
    label: 'Wi-Fi',
    kind: 'wifi',
    usable: true,
    addrs: ['192.168.1.24'],
    reach: 'online',
  },
  {
    name: 'en7',
    label: 'iPhone USB',
    kind: 'tether',
    usable: true,
    addrs: ['172.20.10.2'],
    reach: 'online',
  },
  {
    name: 'en5',
    label: 'Ethernet',
    kind: 'ethernet',
    usable: true,
    addrs: ['10.0.0.31'],
    reach: 'online',
  },
  { name: 'utun4', label: 'utun4', kind: 'vpn', usable: false, addrs: ['100.64.0.7'], reach: null },
]
const BASE_RATE = [7.4 * MB, 3.2 * MB, 10.8 * MB]

interface SimJob extends JobView {
  fill: Float32Array
  owner: Int8Array
  inflight: number[]
  bytes: number[]
  /** Seconds spent running, for the "what each network saved" report. */
  ran: number
}

function err(code: string, message: string, hint: string | null): UiError {
  return { code, message, hint }
}

function nameFromUrl(url: URL): string {
  const seg = url.pathname.split('/').filter(Boolean).pop()
  try {
    return seg ? decodeURIComponent(seg) : url.hostname
  } catch {
    return seg ?? url.hostname
  }
}

/** As groups.rs: "3 files from example.org", or "3 downloads" from several sites. */
function groupName(links: string[]): string {
  const hosts = links.map((l) => {
    try {
      return new URL(l).host
    } catch {
      return ''
    }
  })
  const same = hosts.every((h) => h && h === hosts[0])
  return same ? `${links.length} files from ${hosts[0]}` : `${links.length} downloads`
}

/** The same estimate as savings.rs: without network k, T × bytes_k / (total − bytes_k) more. */
function savings(secs: number, nets: [string, number][]): ReportView {
  const total = nets.reduce((a, [, b]) => a + b, 0)
  return {
    secs,
    nets: nets.map(([label, bytes]) => ({
      label,
      bytes,
      savedSecs: secs >= 3 && total > bytes ? (secs * bytes) / (total - bytes) : null,
    })),
  }
}

export function createDemoBackend(params: URLSearchParams): Backend {
  const random = rng(Number(params.get('seed') ?? 7))
  const speed = Math.max(0.1, Math.min(50, Number(params.get('speed') ?? 1)))
  const drops = params.get('drop') !== '0'
  const freeze = params.has('freeze') ? Number(params.get('freeze')) : null
  const jobs: SimJob[] = []
  let nextId = 1
  let clock = 0
  let updateCancelled = false
  let listener: ((e: UiEvent) => void) | null = null
  const torrents = createDemoTorrents(params, () => (e) => listener?.(e))
  const sends = createDemoSends(params, () => (e) => listener?.(e))
  const nearby = createDemoNearby(params, () => (e) => listener?.(e))
  let limits: LimitsView = { global: 0, networks: [], slow: false, slowRate: 1024 * 1024 }
  let prefs: NetPref[] = []
  let perNetDns = false
  let allowances: AllowanceView[] = NETWORKS.filter((n) => n.usable).map((n) => ({
    name: n.name,
    allowance: n.name === 'en7' && params.get('allowance') === 'reached' ? 5 * 1024 ** 3 : null,
    resetDay: 1,
    used: n.name === 'en7' ? 5.2 * 1024 ** 3 : 0,
    resetsOn: '2026-11-01',
    reached: n.name === 'en7' && params.get('allowance') === 'reached',
  }))

  const now = () => Math.floor(Date.now() / 1000)

  function make(
    name: string,
    total: number,
    status: JobStatus,
    done = 0,
    error: string | null = null,
    errorAction: JobView['errorAction'] = null,
  ): SimJob {
    const fill = new Float32Array(TICKS)
    const full = Math.floor(done * TICKS)
    for (let i = 0; i < full; i++) fill[i] = 1
    const owner = new Int8Array(TICKS).fill(-1)
    for (let i = 0; i < full; i++) owner[i] = i % 3
    const job: SimJob = {
      id: nextId++,
      url: `https://downloads.example.org/files/${encodeURIComponent(name)}`,
      name,
      dir: '~/Downloads',
      status,
      resumable: status === 'paused' || status === 'failed',
      written: Math.round(total * done),
      total,
      error,
      errorAction,
      finalPath: status === 'completed' ? `~/Downloads/${name}` : null,
      createdAt: now() - (100 - nextId) * 60,
      position: nextId,
      verify: false,
      speedLimit: 0,
      retryIn: status === 'failed' && errorAction === 'retry' ? 95 : null,
      startAt: null,
      mirrors: [],
      mirrorNotes: [],
      focused: false,
      checksumFrom: null,
      verified: false,
      report: null,
      readyBy: null,
      readyState: null,
      groupId: null,
      groupName: null,
      fill,
      owner,
      inflight: [-1, -1, -1],
      bytes: [0, 0, 0],
      ran: 0,
    }
    jobs.unshift(job)
    return job
  }

  // many=N adds N finished downloads with awkward names (stress tests).
  const many = Math.min(5000, Number(params.get('many') ?? 0))
  const awkward = [
    'a'.repeat(240) + '.bin',
    'تقرير-السنة-المالية-٢٠٢٦.pdf',
    '📦 release (final) [v2] {copy}.zip',
    'no-extension',
    '.hidden',
    'zero-bytes.txt',
  ]
  for (let i = 0; i < many; i++) {
    const name = awkward[i % awkward.length] ?? 'file'
    const j = make(
      `${i}-${name}`,
      name === 'zero-bytes.txt' ? 0 : (i % 7) * 13 * MB,
      i % 9 === 0 ? 'failed' : 'completed',
      1,
      i % 9 === 0 ? "The server answered with status 500, so the download couldn't start." : null,
    )
    if (i % 11 === 0) j.total = null
  }

  if (params.get('empty') !== '1') {
    Object.assign(make('Blender-5.1-macos-arm64.dmg', 412 * MB, 'completed', 1), {
      verify: true,
      verified: true,
      checksumFrom: 'SHA256SUMS',
      // 30 s over three networks: what each one saved, as the app works it out.
      report: savings(30, [
        ['Wi-Fi', 152 * MB],
        ['iPhone USB', 66 * MB],
        ['Ethernet', 194 * MB],
      ]),
    })
    make(
      'nightly-build-2026-10-07.zip',
      96 * MB,
      'failed',
      0.21,
      'This link stopped working (the server said 403). Get a fresh link to the same file and try again.',
      'fix-link',
    )
    make(
      'mirror-snapshot-2026-10.tar',
      640 * MB,
      'failed-final',
      0.55,
      'The file on the server changed during the download, so it was stopped to avoid a mixed file. Start it again.',
      'start-over',
    )
    make('dataset-shard-0042.tar.zst', 2.4 * 1024 * MB, 'paused', 0.38)
    if (params.get('offline') === '1') {
      make(
        'podcast-episode-212.mp3',
        80 * MB,
        'failed',
        0.4,
        'Every network failed. Last problem: no network could reach the server.',
        'retry',
      )
    }
    if (params.get('allowance') === 'reached') {
      make(
        'conference-talk-4k.mp4',
        1.8 * 1024 * MB,
        'paused',
        0.62,
        'Paused: every network reached its data allowance. It continues after the allowance resets, or raise it in Networks.',
        'allowance',
      )
    }
    make('ubuntu-26.04-desktop-amd64.iso', 1.1 * 1024 * MB, 'running', 0)
  }

  const view = (j: SimJob): JobView => {
    const { fill: _f, owner: _o, inflight: _i, bytes: _b, ran: _r, ...rest } = j
    return { ...rest }
  }
  const emitJobs = () => listener?.({ type: 'jobs', jobs: jobs.map(view) })

  function laneUp(lane: number, t: number) {
    // The phone drops out for a few seconds to show recovery.
    return !(drops && lane === 1 && t % 40 > 14 && t % 40 < 20)
  }

  function wobble(seed: number, t: number) {
    let v = 0
    for (let i = 1; i <= 3; i++) v += Math.sin(t * (0.6 * i + seed * 0.13) + seed * 1.7) / (i * 1.8)
    return v
  }

  function nextTick(j: SimJob) {
    for (let i = 0; i < TICKS; i++) if (j.fill[i]! < 1 && !j.inflight.includes(i)) return i
    return -1
  }

  function step(dt: number) {
    clock += dt
    const running = jobs.filter((j) => j.status === 'running')
    for (const j of running) {
      const total = j.total ?? 1
      const perTick = total / TICKS
      const share = 1 / Math.max(1, running.length)
      const cap = capFor(j, share)
      j.ran += dt
      BASE_RATE.forEach((base, lane) => {
        if (!laneUp(lane, clock)) {
          j.inflight[lane] = -1
          return
        }
        const r = Math.max(0.2 * MB, base * (1 + 0.22 * wobble(lane + 11, clock))) * share * cap
        if (j.inflight[lane]! < 0) j.inflight[lane] = nextTick(j)
        const tick = j.inflight[lane]!
        if (tick < 0) return
        const add = Math.min(r * dt, (1 - j.fill[tick]!) * perTick)
        j.fill[tick] = Math.min(1, j.fill[tick]! + add / perTick)
        j.bytes[lane] = j.bytes[lane]! + add
        if (j.fill[tick]! >= 1) {
          j.owner[tick] = lane
          j.inflight[lane] = nextTick(j)
        }
      })
      let w = 0
      for (let i = 0; i < TICKS; i++) w += j.fill[i]! * perTick
      j.written = Math.min(total, Math.round(w))
      if (j.fill.every((f) => f >= 1)) {
        j.status = 'completed'
        j.written = total
        j.finalPath = `${j.dir}/${j.name}`
        j.inflight = [-1, -1, -1]
        j.report = savings(
          j.ran,
          j.bytes.map((b, i) => [NETWORKS[i]!.label, b] as [string, number]),
        )
        listener?.({ type: 'live', ...live(j) }) // the final picture, as the engine sends
        if (j.focused) endFocus(j)
        emitJobs()
      }
    }
  }

  // "Do this one now": the held downloads carry on when the focused one ends.
  const held = new Set<number>()
  function endFocus(j: SimJob) {
    j.focused = false
    for (const id of held) {
      const h = jobs.find((x) => x.id === id)
      if (h?.status === 'paused') {
        h.status = 'running'
        h.resumable = false
        h.error = null
        h.errorAction = null
      }
    }
    held.clear()
  }

  /** A download's own limit scales every lane down to fit it (wobble included). */
  function capFor(j: SimJob, share: number): number {
    const full = BASE_RATE.reduce((a, b) => a + b, 0) * share * 1.25
    return j.speedLimit > 0 ? Math.min(1, j.speedLimit / full) : 1
  }

  function live(j: SimJob): Live {
    const running = jobs.filter((x) => x.status === 'running').length
    const share = 1 / Math.max(1, running)
    const cap = capFor(j, share)
    const networks: LiveNet[] = BASE_RATE.map((base, lane) => {
      const n = NETWORKS[lane]!
      const up = laneUp(lane, clock)
      return {
        name: n.name,
        label: n.label,
        kind: n.kind,
        bytes: Math.round(j.bytes[lane]!),
        rate: up
          ? Math.max(0.2 * MB, base * (1 + 0.22 * wobble(lane + 11, clock))) * share * cap
          : 0,
        streams: up ? [8, 4, 12][lane]! : 0,
        dead: !up,
      }
    })
    const ticks: number[] = []
    for (let i = 0; i < TICKS; i++) {
      const holder = j.inflight.indexOf(i)
      ticks.push(Math.round(j.fill[i]! * 100), j.owner[i]! + 1, holder + 1)
    }
    return {
      id: j.id,
      written: j.written,
      total: j.total,
      rate: networks.reduce((a, n) => a + n.rate, 0),
      networks,
      ticks,
      retries: Math.floor(clock / 9),
      hedges: Math.floor(clock / 13),
    }
  }

  function tick() {
    step(0.2 * speed)
    if (torrents.step(0.2 * speed)) torrents.send()
    sends.step(0.2 * speed)
    for (const j of jobs) if (j.status === 'running') listener?.({ type: 'live', ...live(j) })
  }

  const find = (id: number) => {
    const j = jobs.find((x) => x.id === id)
    if (!j)
      throw err('not-found', `There's no download ${id}.`, 'It may have been removed already.')
    return j
  }

  /** Same rules as the service: finished, and (in the demo) never deleted. */
  function finishedFile(id: number) {
    const j = find(id)
    if (j.status !== 'completed') {
      throw err(
        'not-finished',
        "This download hasn't finished yet, so there's no file to show.",
        null,
      )
    }
    if (params.get('missing') === '1') {
      throw err(
        'file-missing',
        `The file is no longer at ${j.finalPath}.`,
        'It may have been moved, renamed or deleted.',
      )
    }
  }

  let maxRunning = 3
  let findSums = true
  let nextGroup = 1
  let longMinutes = 5
  const windowPrefs = { startAtLogin: false, closeToTray: false, watchClipboard: false }
  let automation: Automation = {
    schedule: {
      enabled: false,
      start: 60,
      stop: 420,
      days: [true, true, true, true, true, true, true],
    },
    whenDone: 'nothing',
    keepAwake: true,
    sortByType: false,
    nameTaken: 'ask',
    afterDownload: 'nothing',
    lowBattery: 'keep-going',
  }
  const automationView = (): AutomationView => {
    const s = automation.schedule
    const hhmm = (m: number) =>
      `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
    return {
      settings: automation,
      allowedNow: !s.enabled,
      next: s.enabled ? `Starts at ${hhmm(s.start)}.` : null,
    }
  }
  const backend: Backend = {
    demo: true,
    appInfo: async () => ({
      version: '0.0.0',
      defaultDir: '~/Downloads',
      updatedFrom: params.get('updated'),
    }),
    openReleaseNotes: async () => {},
    listJobs: async () => jobs.map(view),
    listNetworks: async () =>
      NETWORKS.map((n) => ({
        ...n,
        reach: n.name === params.get('portal') ? ('portal' as const) : n.reach,
      })),
    openSignIn: async () => {},
    add: async (raw, dir, options) => {
      const text = raw.trim()
      if (!text)
        throw err('bad-link', 'Paste a link to download.', 'Links start with http:// or https://.')
      if (text.length > 8192)
        throw err(
          'bad-link',
          'That link is too long (over 8 KB).',
          'Copy the link again from its page.',
        )
      let url: URL
      try {
        url = new URL(text)
      } catch {
        throw err(
          'bad-link',
          `"${text.slice(0, 80)}" isn't a valid link.`,
          'Links start with http:// or https://.',
        )
      }
      if (url.protocol !== 'http:' && url.protocol !== 'https:') {
        throw err(
          'bad-link',
          `${url.protocol.replace(':', '')}: links aren't supported. Use an http:// or https:// link.`,
          'Links start with http:// or https://.',
        )
      }
      if (!url.hostname)
        throw err('bad-link', 'The link has no host name.', 'Links start with http:// or https://.')
      if (dir && dir.trim() && !dir.trim().startsWith('~') && !dir.trim().startsWith('/')) {
        throw err(
          'folder-missing',
          `The folder "${dir.trim()}" doesn't exist.`,
          'Pick another folder to save into.',
        )
      }
      const sha = options?.sha256?.trim()
      if (sha && !/^[0-9a-fA-F]{64}$/.test(sha))
        throw err(
          'bad-checksum',
          'A SHA-256 is 64 hex digits (0-9, a-f).',
          'Paste the SHA-256 from the download page: 64 letters and digits.',
        )
      const name = options?.name?.trim()
      if (name && name.length > 255)
        throw err(
          'bad-name',
          'That file name is too long (over 255 bytes).',
          'Pick a shorter name.',
        )
      const existing = jobs.find((x) => x.url === url.href)
      if (existing && !options?.allowDuplicate)
        throw err(
          'duplicate',
          `You already added this link (${existing.name}).`,
          'Download it again anyway, or open the one in your list.',
        )
      const j = make(
        name || nameFromUrl(url),
        (180 + random() * 700) * MB,
        options?.later || options?.startAt ? 'paused' : 'running',
      )
      j.startAt = options?.startAt ?? null
      const ms = (options?.mirrors ?? []).map((m) => m.trim()).filter(Boolean)
      for (const m of ms)
        if (!/^https?:\/\//i.test(m))
          throw err(
            'bad-mirror',
            `A mirror link isn't usable: "${m}" isn't a web link.`,
            'Mirrors are http:// or https:// links to the same file.',
          )
      j.mirrors = ms.map((m) => new URL(m).hostname)
      j.url = url.href
      j.verify = Boolean(sha)
      if (dir?.trim()) j.dir = dir.trim()
      emitJobs()
      return j.id
    },
    exportLinks: async () =>
      params.get('pick') === 'cancel' ? null : new Set(jobs.map((j) => j.url)).size,
    importLinks: async () =>
      params.get('pick') === 'cancel'
        ? null
        : backend.addBatch(
            '# Fuselane downloads\nhttps://example.com/one.iso\nhttps://example.com/two.zip',
            null,
            true,
          ),
    addBatch: async (text, dir, later, group) => {
      const found = [...new Set(text.match(/https?:\/\/[^\s"<>]+/gi) ?? [])].map((l) =>
        l.replace(/[.,;)']+$/, ''),
      )
      const links: string[] = []
      for (const l of found) {
        const letters = /\[([a-z])-([a-z])\]/i.exec(l)
        if (letters) {
          const [x, y] = [letters[1]!.charCodeAt(0), letters[2]!.charCodeAt(0)]
          for (let c = Math.min(x, y); c <= Math.max(x, y); c++)
            links.push(l.replace(letters[0], String.fromCharCode(c)))
          continue
        }
        const m = /\[(\d+)-(\d+)\]/.exec(l)
        if (!m) {
          links.push(l)
          continue
        }
        const [a, b] = [Number(m[1]), Number(m[2])]
        if (Math.abs(b - a) >= 1000)
          throw err(
            'too-many',
            'That pattern makes more than 1000 links. Use a smaller range.',
            null,
          )
        const width = m[1]!.startsWith('0') ? m[1]!.length : 0
        for (let n = Math.min(a, b); n <= Math.max(a, b); n++)
          links.push(l.replace(m[0], String(n).padStart(width, '0')))
      }
      if (!links.length)
        throw err(
          'bad-link',
          'There are no http:// or https:// links in that text.',
          'Paste one link per line, or a pattern like https://example.com/part[01-10].zip.',
        )
      const result: BatchResult = { added: [], skipped: [], group: null }
      for (const l of [...new Set(links)]) {
        try {
          result.added.push(await backend.add(l, dir, { later: later ?? false }))
        } catch (e) {
          result.skipped.push({ url: l, reason: (e as UiError).message })
        }
      }
      if (group !== null && group !== undefined && result.added.length >= 2) {
        const name = group.trim() || groupName(links)
        if (name.length > 80) throw err('bad-group-name', 'That group name is too long.', null)
        result.group = nextGroup++
        for (const id of result.added)
          Object.assign(find(id), { groupId: result.group, groupName: name })
        emitJobs()
      }
      return result
    },
    automation: async () => automationView(),
    setAutomation: async (settings) => {
      const s = settings.schedule
      if (s.start < 0 || s.start >= 1440 || s.stop < 0 || s.stop >= 1440)
        throw err('bad-value', 'Times must be between 00:00 and 23:59.', null)
      if (s.enabled && s.start === s.stop)
        throw err(
          'bad-value',
          'The schedule starts and stops at the same time. Pick a stop time after the start.',
          null,
        )
      if (s.enabled && !s.days.some(Boolean))
        throw err('bad-value', 'Pick at least one day for the schedule.', null)
      automation = settings
      return automationView()
    },
    cancelWhenDone: async () => {
      listener?.({ type: 'whenDoneCancelled' })
    },
    windowPrefs: async () => ({ ...windowPrefs }),
    setStartAtLogin: async (on) => (windowPrefs.startAtLogin = on),
    setCloseToTray: async (on) => (windowPrefs.closeToTray = on),
    setWatchClipboard: async (on) => (windowPrefs.watchClipboard = on),
    maxRunning: async () => maxRunning,
    setMaxRunning: async (n) => {
      if (!Number.isInteger(n) || n < 1 || n > 8)
        throw err('bad-value', 'Pick between 1 and 8 downloads at once.', null)
      maxRunning = n
      return n
    },
    setJobLimit: async (id, rate) => {
      if (!Number.isFinite(rate) || rate < 0 || rate > 100 * 1024 * MB)
        throw err(
          'bad-limit',
          'That limit is too high to be a real speed.',
          'Use a speed in KB/s or MB/s, or leave it empty for no limit.',
        )
      find(id).speedLimit = rate
      emitJobs()
    },
    reorder: async (ids) => {
      const rest = jobs.filter((j) => !ids.includes(j.id)).sort((a, b) => a.position - b.position)
      const order = [...ids.map((id) => jobs.find((j) => j.id === id)).filter(Boolean), ...rest]
      order.forEach((j, i) => {
        if (j) j.position = i + 1
      })
      emitJobs()
    },
    alreadyHave: async (name, size) => {
      const j = jobs.find(
        (x) =>
          x.status === 'completed' &&
          x.name.toLowerCase() === name.trim().toLowerCase() &&
          size !== null &&
          x.total === size,
      )
      return j
        ? {
            id: j.id,
            name: j.name,
            path: `${j.dir}/${j.name}`,
            size: size!,
            finishedAt: now() - 86400,
          }
        : null
    },
    setReadyBy: async (id, at) => {
      const j = find(id)
      const nowS = Math.floor(Date.now() / 1000)
      if (at !== null && (at <= nowS || at > nowS + 366 * 86400))
        throw err('bad-value', 'Pick a time in the next year.', null)
      j.readyBy = at
      if (at === null) j.readyState = null
      else {
        // As deadline.rs: the remaining bytes at the networks' combined speed.
        const left = (j.total ?? 0) - j.written
        const rate = BASE_RATE.reduce((a, b) => a + b, 0)
        j.readyState = nowS + left / rate > at ? 'at-risk' : 'on-track'
      }
      emitJobs()
    },
    filesOnPage: async (url) => {
      await new Promise((r) => setTimeout(r, 200))
      let base: URL
      try {
        base = new URL(url.trim())
      } catch {
        throw err('bad-link', "That isn't a valid link.", 'Links start with http:// or https://.')
      }
      if (base.pathname.includes('nofiles'))
        throw err(
          'no-files',
          "That page doesn't link to any files.",
          'Paste the link of the file itself, or a page that lists downloads.',
        )
      const names = [
        'ubuntu-26.04-desktop-amd64.iso',
        'ubuntu-26.04-live-server-amd64.iso',
        'release-notes.pdf',
        'wallpapers.zip',
        'install-guide.pdf',
        'intro-video.mp4',
        'source.tar.gz',
      ]
      return {
        title: 'Ubuntu 26.04 downloads',
        files: names.map((n) => ({ url: new URL(n, base).toString(), name: n })),
      }
    },
    renameGroup: async (id, name) => {
      const n = name.trim()
      if (!n) throw err('bad-group-name', 'Give the group a name.', null)
      if (n.length > 80) throw err('bad-group-name', 'That group name is too long.', null)
      for (const j of jobs) if (j.groupId === id) j.groupName = n
      emitJobs()
    },
    ungroup: async (id) => {
      for (const j of jobs)
        if (j.groupId === id) Object.assign(j, { groupId: null, groupName: null })
      emitJobs()
    },
    pauseGroup: async (id) => {
      for (const j of jobs.filter((x) => x.groupId === id))
        if (j.status === 'running' || j.status === 'queued') await backend.pause(j.id)
    },
    resumeGroup: async (id) => {
      for (const j of jobs.filter((x) => x.groupId === id))
        if (j.resumable) await backend.resume(j.id)
    },
    nearbyHandoff: async (id, fingerprint) => {
      const j = find(id)
      if (j.status !== 'paused' && j.status !== 'failed')
        throw err('not-paused', 'Pause the download first, then send it.', null)
      const name = (j.name.split('/').pop() ?? j.name) + '.fuselane'
      return backend.nearbySend(fingerprint, [`/Users/demo/Downloads/${name}`])
    },
    focus: async (id) => {
      const j = find(id)
      if (j.status === 'completed')
        throw err('not-resumable', "This download can't run, so it can't go first.", null)
      for (const o of jobs) {
        if (o.focused && o.id !== id) o.focused = false
        if (o.id !== id && o.status === 'running') {
          held.add(o.id)
          o.status = 'paused'
          o.resumable = true
          o.inflight = [-1, -1, -1]
          o.error = `Waiting: ${j.name} goes first. This carries on after it.`
          o.errorAction = 'focus'
        }
      }
      j.focused = true
      j.status = 'running'
      j.resumable = false
      j.error = null
      j.errorAction = null
      j.startAt = null
      emitJobs()
    },
    unfocus: async () => {
      const j = jobs.find((x) => x.focused)
      if (j) endFocus(j)
      emitJobs()
    },
    pause: async (id) => {
      held.delete(id)
      const j = find(id)
      if (j.status === 'running' || j.status === 'queued') {
        j.status = 'paused'
        j.resumable = true
        j.inflight = [-1, -1, -1]
        listener?.({ type: 'live', ...live(j) }) // the final picture, as the engine sends
        emitJobs()
      } else if (j.status !== 'paused') {
        throw err('not-running', "This download isn't running, so there's nothing to pause.", null)
      }
    },
    resume: async (id) => {
      const j = find(id)
      j.startAt = null
      if (j.status === 'running') return
      if (!j.resumable)
        throw err(
          'not-resumable',
          `download ${id} is ${j.status} and can't be resumed. Start it again.`,
          'Add the link again to download it from the start.',
        )
      j.status = 'running'
      j.error = null
      j.resumable = false
      j.retryIn = null
      emitJobs()
    },
    reveal: async (id) => finishedFile(id),
    openFile: async (id) => finishedFile(id),
    preview: async (raw): Promise<PreviewView> => {
      await new Promise((r) => setTimeout(r, 150))
      let url: URL
      try {
        url = new URL(raw.trim())
      } catch {
        throw err(
          'bad-link',
          `"${raw.trim().slice(0, 80)}" isn't a valid link.`,
          'Links start with http:// or https://.',
        )
      }
      if (url.pathname.includes('missing')) {
        throw err(
          'preview-failed',
          "The server says this file doesn't exist (404). Check the link.",
          null,
        )
      }
      const filename = nameFromUrl(url)
      return {
        filename,
        total: url.hostname.includes('unknown')
          ? null
          : (jobs.find((j) => j.name === filename)?.total ?? 734003200),
        splittable: !url.hostname.includes('noranges'),
      }
    },
    pickFolder: async () => (params.get('pick') === 'cancel' ? null : '/Users/demo/Movies'),
    checkUpdate: async () => {
      if (params.get('update') === 'offline')
        throw err('update-check', "Couldn't check for updates: you're offline.", null)
      return ['1', 'bad', 'slow'].includes(params.get('update') ?? '')
        ? { version: '0.1.0-beta.8', notes: 'Faster resume.', size: 38.6 * MB }
        : null
    },
    installUpdate: async () => {
      // Like the app: progress over the networks, then installing (B8.4).
      const total = 38.6 * MB
      const slow = params.get('update') === 'slow'
      let done = 0
      updateCancelled = false
      while (done < total) {
        await new Promise((r) => setTimeout(r, slow ? 250 : 40))
        if (updateCancelled) throw err('update-cancelled', 'Update cancelled.', null)
        if (params.get('update') === 'bad' && done > total * 0.57)
          throw err(
            'update-failed',
            "Couldn't download the update: every network dropped at 22.0 MB.",
            'Try again. It continues from where it stopped, and your downloads are safe.',
          )
        done = Math.min(total, done + (slow ? 0.6 : 3.1) * MB)
        listener?.({
          type: 'update',
          progress: {
            phase: 'downloading',
            done,
            total,
            rate: slow ? 2.4 * MB : 6.1 * MB,
            networks: 3,
          },
        })
      }
      listener?.({
        type: 'update',
        progress: { phase: 'installing', done: total, total, rate: 0, networks: 0 },
      })
      // The real app restarts here; the demo just stays on "installing".
      await new Promise(() => {})
    },
    cancelUpdate: async () => {
      updateCancelled = true
    },
    // Taken when a download of that name is in the list (the demo has no folder).
    nameTaken: async (_dir, name) => jobs.some((j) => j.name === name.trim()),
    getLimits: async () => structuredClone(limits),
    perNetworkDns: async () => perNetDns,
    findChecksums: async () => findSums,
    longMinutes: async () => longMinutes,
    setLongMinutes: async (m) => {
      if (!Number.isInteger(m) || m < 1 || m > 600)
        throw err('bad-value', 'Pick between 1 and 600 minutes.', null)
      return (longMinutes = m)
    },
    setFindChecksums: async (on) => (findSums = on),
    setPerNetworkDns: async (on) => (perNetDns = on),
    allowances: async () => structuredClone(allowances),
    setAllowance: async (req) => {
      const bad = (m: string) =>
        err('bad-allowance', m, 'Use an amount like 5 GB and a reset day from 1 to 28.')
      if (!Number.isInteger(req.resetDay) || req.resetDay < 1 || req.resetDay > 28)
        throw bad('The reset day must be from 1 to 28.')
      if (!Number.isFinite(req.bytes) || req.bytes < 0 || req.bytes > 100 * 1024 ** 4)
        throw bad('That allowance is too big to be real.')
      allowances = allowances.map((a) =>
        a.name === req.name
          ? {
              ...a,
              allowance: req.bytes > 0 ? req.bytes : null,
              resetDay: req.resetDay,
              resetsOn: `2026-11-${String(req.resetDay).padStart(2, '0')}`,
              reached: req.bytes > 0 && a.used >= req.bytes,
            }
          : a,
      )
      return structuredClone(allowances)
    },
    setSlow: async (on) => {
      limits = { ...limits, slow: on }
      return structuredClone(limits)
    },
    networkPrefs: async () => structuredClone(prefs),
    setNetworkPref: async (pref) => {
      const label = pref.label?.trim() || null
      if (label && label.length > 40)
        throw err('bad-network-name', 'That name is too long.', 'Use up to 40 characters.')
      if (
        pref.lane &&
        !['tide', 'volt', 'iris', 'rose', 'mint', 'sky', 'lilac', 'steel'].includes(pref.lane)
      )
        throw err('bad-network-color', "That colour isn't one of Fuselane's network colours.", null)
      prefs = prefs.filter((p) => p.name !== pref.name)
      const useFor = pref.useFor ?? 'always'
      if (!['always', 'long', 'never'].includes(useFor))
        throw err('bad-value', "That isn't a way to use a network.", null)
      if (label || pref.lane || useFor !== 'always')
        prefs.push({ name: pref.name, label, lane: pref.lane, useFor })
      return structuredClone(prefs)
    },
    diagnostics: async () =>
      [
        'Fuselane 0.0.0 on demo browser',
        '',
        'Checks:',
        '  ok pinning: interface pinning works',
        '',
        'Networks:',
        ...NETWORKS.map(
          (n) => `  ${n.name} (${n.kind})${n.usable ? ', used' : ''}: 1 IPv4, 0 IPv6`,
        ),
        '',
        `Speed limits: overall ${limits.global} B/s, ${limits.networks.length} per-network`,
        '',
        'Recent downloads (newest first):',
        ...jobs
          .slice(0, 20)
          .map((j) => `  #${j.id} https ${j.status}${j.errorAction ? ` (${j.errorAction})` : ''}`),
      ].join('\n'),
    setLimits: async (next) => {
      // The same rules as the service.
      const max = 100 * 1024 ** 3
      const bad = (m: string) =>
        err('bad-limit', m, 'Use a speed in KB/s or MB/s, or leave it empty for no limit.')
      if (!Number.isFinite(next.global) || next.global < 0 || next.global > max)
        throw bad('That overall limit is too high to be a real speed.')
      const names = new Set<string>()
      for (const n of next.networks) {
        if (!n.name || n.name !== n.name.trim())
          throw bad('A network limit has no valid network name.')
        if (!Number.isFinite(n.rate) || n.rate < 0 || n.rate > max)
          throw bad('That network limit is too high to be a real speed.')
        if (names.has(n.name)) throw bad('A network is listed twice.')
        names.add(n.name)
      }
      limits = {
        global: Math.round(next.global),
        networks: next.networks.filter((n) => n.rate > 0),
        slow: next.slow,
        slowRate: next.slowRate || 1024 * 1024,
      }
      return structuredClone(limits)
    },
    fixLink: async (id, raw) => {
      let url: URL
      try {
        url = new URL(raw.trim())
      } catch {
        throw err(
          'bad-link',
          `"${raw.trim().slice(0, 80)}" isn't a valid link.`,
          'Links start with http:// or https://.',
        )
      }
      if (url.protocol !== 'http:' && url.protocol !== 'https:') {
        throw err(
          'bad-link',
          `${url.protocol.replace(':', '')}: links aren't supported. Use an http:// or https:// link.`,
          'Links start with http:// or https://.',
        )
      }
      const j = find(id)
      if (!j.resumable)
        throw err(
          'not-resumable',
          "This download can't continue from a new link.",
          'Start it again instead.',
        )
      j.url = url.href
      j.status = 'running'
      j.error = null
      j.errorAction = null
      j.resumable = false
      emitJobs()
    },
    startOver: async (id) => {
      const j = find(id)
      jobs.splice(jobs.indexOf(j), 1)
      const fresh = make(j.name, j.total ?? 100 * MB, 'running')
      fresh.url = j.url
      emitJobs()
      return fresh.id
    },
    trashFile: async (id) => {
      const j = find(id)
      if (j.status !== 'completed')
        throw err('not-finished', "This download hasn't finished yet, so there's no file.", null)
      jobs.splice(jobs.indexOf(j), 1)
      emitJobs()
    },
    remove: async (id) => {
      find(id)
      jobs.splice(
        jobs.findIndex((x) => x.id === id),
        1,
      )
      emitJobs()
    },
    ...torrents.methods,
    ...sends.methods,
    ...nearby.methods,
    subscribe: async (onEvent) => {
      listener = onEvent
      // Tests stand in for the OS handing over a magnet or .torrent (demo only).
      ;(window as unknown as { __demoOpen?: (t: string) => void }).__demoOpen = (target) =>
        onEvent({ type: 'open', target })
      // And for backend events the demo engine never produces by itself.
      ;(window as unknown as { __demoEvent?: (e: UiEvent) => void }).__demoEvent = onEvent
      emitJobs()
      torrents.send()
      sends.send()
      if (freeze !== null) {
        for (let t = 0; t < freeze; t += 0.2) {
          step(0.2)
          torrents.step(0.2)
        }
        torrents.send()
        for (const j of jobs) if (j.status === 'running') onEvent({ type: 'live', ...live(j) })
        return
      }
      setInterval(tick, 200)
      // Like the service's 5-second tick: start what is due (B8.5).
      setInterval(() => {
        const due = jobs.filter(
          (j) => j.status === 'paused' && j.startAt !== null && j.startAt * 1000 <= Date.now(),
        )
        for (const j of due) void backend.resume(j.id)
      }, 1000)
    },
  }
  return backend
}
