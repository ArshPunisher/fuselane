// Simulated torrents for the browser demo, mirroring torrents.rs: inspect first,
// pick files, then download with each network credited for verified bytes.
// URL parameters: torrents=0 (none at start), torrents=1 (the sample even with empty=1),
// magnet=slow (inspect never ends).
import type {
  ListingView,
  PeerView,
  SeedSettings,
  TorrentFileView,
  TorrentView,
  UiError,
  UiEvent,
} from './types'

const MB = 1024 * 1024
const NETS = ['en0', 'en7', 'en5']
/** Sample peers: app, address, which of our networks, share of the speed. */
const DEMO_PEERS: [string | null, string, string, number][] = [
  ['qBittorrent 5.0.4', '84.17.52.10:51413', 'en5', 0.34],
  ['Transmission 4.0.6', '193.32.126.214:6881', 'en0', 0.27],
  ['Deluge 2.1.1', '45.134.212.88:58237', 'en7', 0.18],
  ['libtorrent 2.0.10', '[2a02:8109:1140::7]:6889', 'en5', 0.12],
  [null, '102.165.48.31:41975', 'en0', 0.09],
]
const SHARE = [0.38, 0.17, 0.45]

interface SimTorrent {
  view: TorrentView
  files: TorrentFileView[]
  wanted: number
}

function err(code: string, message: string, hint: string | null): UiError {
  return { code, message, hint }
}

function hash(seed: string): string {
  let h = 0x811c9dc5
  let out = ''
  for (let round = 0; round < 5; round++) {
    for (const c of seed + round) h = Math.imul(h ^ c.charCodeAt(0), 0x01000193) >>> 0
    out += h.toString(16).padStart(8, '0')
  }
  return out
}

const SAMPLES: Record<string, { name: string; files: [string, number][] }> = {
  movie: {
    name: 'Sprite Fright (2021) 4K',
    files: [
      ['Sprite Fright 4K.mkv', 2.81 * 1024 * MB],
      ['Sprite Fright 1080p.mkv', 912 * MB],
      ['Subtitles/English.srt', 41_233],
      ['Subtitles/Deutsch.srt', 43_910],
      ['poster.jpg', 1.4 * MB],
    ],
  },
  distro: {
    name: 'debian-13.1.0-amd64-DVD-1.iso',
    files: [['debian-13.1.0-amd64-DVD-1.iso', 3.94 * 1024 * MB]],
  },
}

export function createDemoTorrents(params: URLSearchParams, emit: () => (e: UiEvent) => void) {
  const list: SimTorrent[] = []
  let seed: SeedSettings = {
    enabled: params.get('share') === '1',
    ratio: 1,
    minutes: 60,
  }
  const pending = new Map<string, ListingView>()

  function listing(kind: keyof typeof SAMPLES, dir: string | null): ListingView {
    const s = SAMPLES[kind]
    if (!s) throw err('invalid-torrent', "Couldn't read the torrent.", null)
    const files = s.files.map(([path, size], index) => ({
      index,
      path,
      size: Math.round(size),
      selected: true,
    }))
    const base = dir || '~/Downloads'
    const l: ListingView = {
      token: hash(s.name),
      name: s.name,
      folder: files.length > 1 ? `${base}/${s.name}` : base,
      total: files.reduce((a, f) => a + f.size, 0),
      files,
    }
    pending.set(l.token, l)
    return l
  }

  function start(l: ListingView, chosen: number[], status: TorrentView['status'], done = 0) {
    const files = l.files.map((f) => ({ ...f, selected: chosen.includes(f.index) }))
    const wanted = files.filter((f) => f.selected).reduce((a, f) => a + f.size, 0)
    const t: SimTorrent = {
      files,
      wanted,
      view: {
        id: l.token,
        name: l.name,
        folder: l.folder,
        status,
        done: Math.round(wanted * done),
        total: wanted,
        uploaded: 0,
        rate: 0,
        error: null,
        fileCount: files.length,
        selectedCount: chosen.length,
        networks: [],
        addedAt: Math.floor(Date.now() / 1000),
      },
    }
    credit(t, 0)
    list.unshift(t)
    return t
  }

  function credit(t: SimTorrent, rate: number) {
    const done = t.view.done
    let given = 0
    t.view.networks = NETS.map((name, i) => {
      const c = i === NETS.length - 1 ? done - given : Math.floor(done * (SHARE[i] ?? 0))
      given += c
      const live = t.view.status === 'downloading'
      return {
        name,
        peers: live ? ([4, 1, 6][i] ?? 0) : 0,
        received: Math.round(c * 1.03),
        rate: live ? Math.round(rate * (SHARE[i] ?? 0)) : 0,
        credited: c,
      }
    })
    // Like the service: the torrent's speed is the sum of its networks (B8.1).
    t.view.rate = t.view.networks.reduce((a, n) => a + n.rate, 0) || rate
  }

  const want = params.get('torrents')
  if (want === '1' || (want !== '0' && params.get('empty') !== '1')) {
    const movie = listing('movie', null)
    start(movie, [0, 2], 'downloading', 0.41)
    pending.clear()
  }

  const find = (id: string) => {
    const t = list.find((x) => x.view.id === id)
    if (!t) throw err('not-found', "That torrent isn't in your list any more.", null)
    return t
  }
  const send = () => emit()({ type: 'torrents', torrents: list.map((t) => ({ ...t.view })) })
  const delay = (ms: number) => new Promise((r) => setTimeout(r, ms))

  return {
    /** Advances downloads by dt seconds; returns whether anything moved. */
    step(dt: number): boolean {
      let moved = false
      for (const t of list) {
        if (t.view.status !== 'downloading') continue
        const rate = 14.2 * MB
        t.view.done = Math.min(t.view.total, t.view.done + rate * dt)
        if (t.view.done >= t.view.total) {
          t.view.status = seed.enabled ? 'seeding' : 'completed'
          credit(t, 0)
        } else credit(t, rate)
        moved = true
      }
      for (const t of list) {
        if (t.view.status !== 'seeding') continue
        t.view.uploaded = Math.min(t.view.total * seed.ratio, t.view.uploaded + 3.1 * MB * dt)
        if (t.view.uploaded >= t.view.total * seed.ratio) t.view.status = 'completed'
        moved = true
      }
      return moved
    },
    send,
    methods: {
      pickTorrent: async () => '~/Downloads/sprite-fright.torrent',
      inspectMagnet: async (magnet: string, dir: string | null) => {
        const m = magnet.trim().toLowerCase()
        if (!m.startsWith('magnet:?') || !m.includes('xt=urn:btih:'))
          throw err(
            'not-a-magnet',
            'That isn\'t a magnet link. It should start with "magnet:?xt=urn:btih:".',
            'Copy the whole magnet link and paste it again.',
          )
        if (params.get('magnet') === 'slow') return new Promise<ListingView>(() => {})
        await delay(900)
        return listing('distro', dir)
      },
      inspectTorrentFile: async (path: string, dir: string | null) => {
        await delay(150)
        if (path.endsWith('bad.torrent'))
          throw err(
            'invalid-torrent',
            "Couldn't read the torrent: not a valid torrent file.",
            'The file may be damaged. Download the .torrent again.',
          )
        if (path.endsWith('unsafe.torrent'))
          throw err(
            'unsafe-torrent',
            'This torrent isn\'t safe to save: "Readme.txt" and "README.TXT" differ only by upper/lower case and would overwrite each other.',
            'Get the torrent from somewhere you trust.',
          )
        return listing('movie', dir)
      },
      addTorrent: async (token: string, files: number[]) => {
        const l = pending.get(token)
        if (!l) throw err('expired', "That torrent's details were cleared. Add it again.", null)
        if (!files.length)
          throw err('nothing-selected', 'Pick at least one file to download.', null)
        if (list.some((t) => t.view.id === token))
          throw err('already-added', 'This torrent is already in your list.', null)
        pending.delete(token)
        start(l, files, 'downloading')
        send()
        return token
      },
      listTorrents: async () => list.map((t) => ({ ...t.view })),
      torrentFiles: async (id: string) => find(id).files.map((f) => ({ ...f })),
      torrentPeers: async (id: string): Promise<PeerView[]> => {
        const t = find(id)
        if (t.view.status !== 'downloading' && t.view.status !== 'seeding') return []
        const clock = Date.now() / 1000
        return DEMO_PEERS.map(([client, addr, net, share], i) => {
          const wobble = 0.75 + 0.25 * Math.sin(clock * 0.9 + i * 1.7)
          const down =
            t.view.status === 'downloading' ? Math.round(t.view.rate * share * wobble) : 0
          return {
            addr,
            client,
            network: net,
            down,
            up: Math.round(down * 0.08),
            received: Math.round(t.view.done * share),
            sent: Math.round(t.view.done * share * 0.05),
            kind: i === 3 ? 'utp' : 'tcp',
          }
        }).sort((a, b) => b.down - a.down)
      },
      torrentPieces: async (id: string, cells: number) => {
        const t = find(id)
        const n = Math.max(1, Math.min(2048, cells))
        const frac = t.view.total ? t.view.done / t.view.total : 0
        // Pieces arrive out of order: a fixed shuffle decides which come first.
        return Array.from({ length: n }, (_, i) => {
          // Pieces come in runs (several peers each sending a stretch), not evenly.
          const run = Math.floor(i / 6)
          const order = (Math.sin(run * 12.9898 + 78.233) * 43758.5453) % 1
          const k = order < 0 ? order + 1 : order
          if (k < frac - 0.02) return 100
          if (k < frac) return 50
          return 0
        })
      },
      pauseTorrent: async (id: string) => {
        const t = find(id)
        if (t.view.status === 'completed') throw err('finished', 'This torrent has finished.', null)
        t.view.status = 'paused'
        credit(t, 0)
        send()
      },
      resumeTorrent: async (id: string) => {
        const t = find(id)
        if (t.view.status === 'completed') throw err('finished', 'This torrent has finished.', null)
        t.view.status = 'downloading'
        send()
      },
      selectTorrentFiles: async (id: string, files: number[]) => {
        const t = find(id)
        if (!files.length)
          throw err('nothing-selected', 'Pick at least one file to download.', null)
        for (const f of t.files) f.selected = files.includes(f.index)
        t.wanted = t.files.filter((f) => f.selected).reduce((a, f) => a + f.size, 0)
        t.view.total = t.wanted
        t.view.done = Math.min(t.view.done, t.wanted)
        t.view.selectedCount = files.length
        if (t.view.status === 'completed' && t.view.done < t.view.total)
          t.view.status = 'downloading'
        send()
      },
      removeTorrent: async (id: string, _deleteFiles: boolean) => {
        find(id)
        list.splice(
          list.findIndex((t) => t.view.id === id),
          1,
        )
        send()
      },
      revealTorrent: async (id: string) => {
        find(id)
      },
      seedSettings: async () => ({ ...seed }),
      setSeedSettings: async (s: SeedSettings) => {
        if (!Number.isFinite(s.ratio) || s.ratio < 0.1 || s.ratio > 10)
          throw err(
            'bad-ratio',
            'The sharing ratio must be between 0.1 and 10.',
            '1 means upload as much as you downloaded.',
          )
        if (!Number.isInteger(s.minutes) || s.minutes < 1 || s.minutes > 10080)
          throw err(
            'bad-minutes',
            'Sharing time must be between 1 minute and 7 days (10080 minutes).',
            null,
          )
        seed = { ...s }
        if (!seed.enabled)
          for (const t of list) if (t.view.status === 'seeding') t.view.status = 'completed'
        send()
        return { ...seed }
      },
      stopSharing: async (id: string) => {
        const t = find(id)
        if (t.view.status !== 'seeding')
          throw err('not-sharing', "This torrent isn't sharing.", null)
        t.view.status = 'completed'
        credit(t, 0)
        send()
      },
      inspectTorrentBytes: async (bytes: Uint8Array, dir: string | null) => {
        await delay(150)
        // A real torrent file is a bencoded dictionary: it starts with "d".
        if (bytes[0] !== 0x64)
          throw err(
            'invalid-torrent',
            "Couldn't read the torrent: not a valid torrent file.",
            'The file may be damaged. Download the .torrent again.',
          )
        return listing('movie', dir)
      },
    },
  }
}
