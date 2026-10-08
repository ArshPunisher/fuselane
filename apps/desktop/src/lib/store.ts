import { create } from 'zustand'
import { connect, toUiError, type Backend } from './backend'
import { setNetPrefs } from './lanes'
import type {
  AppInfo,
  JobView,
  LimitsView,
  Live,
  NetPref,
  TorrentView,
  NetView,
  UiError,
  UpdateInfo,
} from './types'

/** Rate history for the Stream graph: newest last, about five samples a second. */
export interface History {
  names: string[]
  rates: number[][]
}
const HISTORY = 300

export type View = 'transfers' | 'networks' | 'settings'
export type Theme = 'system' | 'light' | 'dark'

interface State {
  backend: Backend | null
  ready: boolean
  info: AppInfo | null
  jobs: JobView[]
  torrents: TorrentView[]
  /** The torrent shown in the detail pane (torrents and downloads share it). */
  selectedTorrent: string | null
  live: Record<number, Live>
  history: Record<number, History>
  networks: NetView[]
  limits: LimitsView
  netPrefs: NetPref[]
  update: UpdateInfo | null
  updateDismissed: boolean
  selected: number | null
  view: View
  adding: boolean
  /** A link handed to the dialog by paste or drop. */
  draft: string
  /** A dropped .torrent's contents, handed to the dialog. */
  draftTorrent: Uint8Array | null
  toast: (UiError & { at: number }) | null
  theme: Theme
  start(): Promise<void>
  select(id: number | null): void
  selectTorrent(id: string | null): void
  setView(v: View): void
  setAdding(open: boolean, draft?: string): void
  /** Opens the dialog with a dropped .torrent file (checked for size first). */
  dropTorrent(file: File): Promise<void>
  setTheme(t: Theme): void
  dismissToast(): void
  refreshNetworks(): Promise<void>
  /** Saves limits; true when the backend accepted them. */
  saveLimits(next: LimitsView): Promise<boolean>
  setSlow(on: boolean): Promise<void>
  /** Saves a network's name and colour; true when accepted. */
  saveNetPref(pref: NetPref): Promise<boolean>
  /** Checks the feed; quiet=true never shows errors (the launch check). */
  checkUpdate(quiet: boolean): Promise<'available' | 'current' | 'error'>
  dismissUpdate(): void
  /** Runs an action; failures become a toast. Returns false on failure. */
  act(f: (b: Backend) => Promise<unknown>): Promise<boolean>
}

function savedTheme(): Theme {
  try {
    const t = localStorage.getItem('fuselane.theme')
    return t === 'light' || t === 'dark' ? t : 'system'
  } catch {
    return 'system'
  }
}

export function applyTheme(t: Theme) {
  if (t === 'system') delete document.documentElement.dataset.theme
  else document.documentElement.dataset.theme = t
}

export const useApp = create<State>((set, get) => ({
  backend: null,
  ready: false,
  info: null,
  jobs: [],
  torrents: [],
  selectedTorrent: null,
  live: {},
  history: {},
  networks: [],
  limits: { global: 0, networks: [], slow: false, slowRate: 1024 * 1024 },
  netPrefs: [],
  update: null,
  updateDismissed: false,
  selected: null,
  view: 'transfers',
  adding: false,
  draft: '',
  draftTorrent: null,
  toast: null,
  theme: savedTheme(),

  async start() {
    if (get().backend) return
    applyTheme(get().theme)
    try {
      const backend = await connect()
      set({ backend })
      const [info, networks, limits] = await Promise.all([
        backend.appInfo(),
        backend.listNetworks().catch(() => []),
        backend
          .getLimits()
          .catch(() => ({ global: 0, networks: [], slow: false, slowRate: 1024 * 1024 })),
      ])
      const netPrefs = await backend.networkPrefs().catch(() => [])
      setNetPrefs(netPrefs)
      set({ info, networks, limits, netPrefs })
      // The first list fills the page until the first event; it never overwrites a newer event.
      let heard = false
      backend
        .listTorrents()
        .then((torrents) => {
          if (!heard) set({ torrents })
        })
        .catch(() => {})
      await backend.subscribe((e) => {
        if (e.type === 'torrents') {
          heard = true
          const ids = new Set(e.torrents.map((t) => t.id))
          set((s) => ({
            torrents: e.torrents,
            selectedTorrent:
              s.selectedTorrent !== null && !ids.has(s.selectedTorrent) ? null : s.selectedTorrent,
          }))
          return
        }
        if (e.type === 'jobs') {
          const ids = new Set(e.jobs.map((j) => j.id))
          set((s) => ({
            jobs: e.jobs,
            ready: true,
            selected: s.selected !== null && !ids.has(s.selected) ? null : s.selected,
          }))
          return
        }
        const { type: _t, ...live } = e
        set((s) => {
          const prev = s.history[live.id]
          const names = live.networks.map((n) => n.name)
          const rates =
            prev && prev.names.join() === names.join() ? prev.rates.slice(-HISTORY + 1) : []
          rates.push(live.networks.map((n) => n.rate))
          return {
            live: { ...s.live, [live.id]: live },
            history: { ...s.history, [live.id]: { names, rates } },
          }
        })
      })
    } catch (e) {
      set({ ready: true, toast: { ...toUiError(e), at: Date.now() } })
    }
  },
  select: (id) => set({ selected: id, selectedTorrent: null }),
  selectTorrent: (id) => set({ selectedTorrent: id, selected: null }),
  setView: (view) => set({ view }),
  setAdding: (adding, draft) =>
    set((s) => ({ adding, draft: draft ?? '', draftTorrent: adding ? s.draftTorrent : null })),
  async dropTorrent(file) {
    const MAX = 8 * 1024 * 1024
    if (file.size > MAX) {
      set({
        toast: {
          code: 'torrent-too-big',
          message: `That file is ${Math.ceil(file.size / 1024 / 1024)} MiB; a .torrent file is at most 8 MiB.`,
          hint: 'Drop the .torrent file, not the download itself.',
          at: Date.now(),
        },
      })
      return
    }
    try {
      const bytes = new Uint8Array(await file.arrayBuffer())
      set({ draftTorrent: bytes, draft: '', adding: true })
    } catch (e) {
      set({ toast: { ...toUiError(e), at: Date.now() } })
    }
  },
  setTheme(theme) {
    try {
      localStorage.setItem('fuselane.theme', theme)
    } catch {
      /* private mode: the choice lasts for this session */
    }
    applyTheme(theme)
    set({ theme })
  },
  dismissToast: () => set({ toast: null }),
  async checkUpdate(quiet) {
    const b = get().backend
    if (!b) return 'error'
    try {
      const update = await b.checkUpdate()
      set({ update, updateDismissed: false })
      return update ? 'available' : 'current'
    } catch (e) {
      if (!quiet) set({ toast: { ...toUiError(e), at: Date.now() } })
      return 'error'
    }
  },
  dismissUpdate: () => set({ updateDismissed: true }),
  async setSlow(on) {
    const b = get().backend
    if (!b) return
    try {
      set({ limits: await b.setSlow(on) })
    } catch (e) {
      set({ toast: { ...toUiError(e), at: Date.now() } })
    }
  },
  async saveNetPref(pref) {
    const b = get().backend
    if (!b) return false
    try {
      const netPrefs = await b.setNetworkPref(pref)
      setNetPrefs(netPrefs)
      set({ netPrefs })
      return true
    } catch (e) {
      set({ toast: { ...toUiError(e), at: Date.now() } })
      return false
    }
  },
  async saveLimits(next) {
    const b = get().backend
    if (!b) return false
    try {
      set({ limits: await b.setLimits(next) })
      return true
    } catch (e) {
      set({ toast: { ...toUiError(e), at: Date.now() } })
      return false
    }
  },
  async refreshNetworks() {
    const b = get().backend
    if (!b) return
    try {
      set({ networks: await b.listNetworks() })
    } catch (e) {
      set({ toast: { ...toUiError(e), at: Date.now() } })
    }
  },
  async act(f) {
    const b = get().backend
    if (!b) return false
    try {
      await f(b)
      return true
    } catch (e) {
      set({ toast: { ...toUiError(e), at: Date.now() } })
      return false
    }
  },
}))
