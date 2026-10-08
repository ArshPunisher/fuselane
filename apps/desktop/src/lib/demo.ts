// Demo engine for the browser (pnpm dev, Playwright, contact sheets). It mimics the
// real service's behaviour and messages closely enough to exercise every UI state,
// and the UI labels it "Demo data" so it is never mistaken for real transfers.
//
// URL parameters: empty=1 (no sample jobs), speed=N (time multiplier),
// seed=N, freeze=SECONDS (advance to that instant, then stop: for screenshots),
// drop=0 (the phone never drops out).
import type { Backend } from './backend'
import type { JobStatus, JobView, Live, LiveNet, NetView, UiError, UiEvent } from './types'

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
  { name: 'en0', label: 'Wi-Fi', kind: 'wifi', usable: true, addrs: ['192.168.1.24'] },
  { name: 'en7', label: 'iPhone USB', kind: 'tether', usable: true, addrs: ['172.20.10.2'] },
  { name: 'en5', label: 'Ethernet', kind: 'ethernet', usable: true, addrs: ['10.0.0.31'] },
  { name: 'utun4', label: 'utun4', kind: 'vpn', usable: false, addrs: ['100.64.0.7'] },
]
const BASE_RATE = [7.4 * MB, 3.2 * MB, 10.8 * MB]

interface SimJob extends JobView {
  fill: Float32Array
  owner: Int8Array
  inflight: number[]
  bytes: number[]
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

export function createDemoBackend(params: URLSearchParams): Backend {
  const random = rng(Number(params.get('seed') ?? 7))
  const speed = Math.max(0.1, Math.min(50, Number(params.get('speed') ?? 1)))
  const drops = params.get('drop') !== '0'
  const freeze = params.has('freeze') ? Number(params.get('freeze')) : null
  const jobs: SimJob[] = []
  let nextId = 1
  let clock = 0
  let listener: ((e: UiEvent) => void) | null = null

  const now = () => Math.floor(Date.now() / 1000)

  function make(
    name: string,
    total: number,
    status: JobStatus,
    done = 0,
    error: string | null = null,
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
      finalPath: status === 'completed' ? `~/Downloads/${name}` : null,
      createdAt: now() - (100 - nextId) * 60,
      fill,
      owner,
      inflight: [-1, -1, -1],
      bytes: [0, 0, 0],
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
    make('Blender-5.1-macos-arm64.dmg', 412 * MB, 'completed', 1)
    make(
      'nightly-build-2026-10-07.zip',
      96 * MB,
      'failed',
      0.21,
      'This link stopped working (the server said 403). Get a fresh link to the same file and try again.',
    )
    make('dataset-shard-0042.tar.zst', 2.4 * 1024 * MB, 'paused', 0.38)
    make('ubuntu-26.04-desktop-amd64.iso', 1.1 * 1024 * MB, 'running', 0)
  }

  const view = (j: SimJob): JobView => {
    const { fill: _f, owner: _o, inflight: _i, bytes: _b, ...rest } = j
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
      BASE_RATE.forEach((base, lane) => {
        if (!laneUp(lane, clock)) {
          j.inflight[lane] = -1
          return
        }
        const r = Math.max(0.2 * MB, base * (1 + 0.22 * wobble(lane + 11, clock))) * share
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
        listener?.({ type: 'live', ...live(j) }) // the final picture, as the engine sends
        emitJobs()
      }
    }
  }

  function live(j: SimJob): Live {
    const running = jobs.filter((x) => x.status === 'running').length
    const share = 1 / Math.max(1, running)
    const networks: LiveNet[] = BASE_RATE.map((base, lane) => {
      const n = NETWORKS[lane]!
      const up = laneUp(lane, clock)
      return {
        name: n.name,
        label: n.label,
        kind: n.kind,
        bytes: Math.round(j.bytes[lane]!),
        rate: up ? Math.max(0.2 * MB, base * (1 + 0.22 * wobble(lane + 11, clock))) * share : 0,
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
    for (const j of jobs) if (j.status === 'running') listener?.({ type: 'live', ...live(j) })
  }

  const find = (id: number) => {
    const j = jobs.find((x) => x.id === id)
    if (!j)
      throw err('not-found', `There's no download ${id}.`, 'It may have been removed already.')
    return j
  }

  return {
    demo: true,
    appInfo: async () => ({ version: '0.0.0', defaultDir: '~/Downloads' }),
    listJobs: async () => jobs.map(view),
    listNetworks: async () => NETWORKS.map((n) => ({ ...n })),
    add: async (raw, dir) => {
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
      const j = make(nameFromUrl(url), (180 + random() * 700) * MB, 'running')
      j.url = url.href
      if (dir?.trim()) j.dir = dir.trim()
      emitJobs()
      return j.id
    },
    pause: async (id) => {
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
    subscribe: async (onEvent) => {
      listener = onEvent
      emitJobs()
      if (freeze !== null) {
        for (let t = 0; t < freeze; t += 0.2) step(0.2)
        for (const j of jobs) if (j.status === 'running') onEvent({ type: 'live', ...live(j) })
        return
      }
      setInterval(tick, 200)
    },
  }
}
