import { create } from 'zustand'
import { connect, toUiError, type Backend } from './backend'
import type { AppInfo, JobView, LimitsView, Live, NetView, UiError } from './types'

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
  live: Record<number, Live>
  history: Record<number, History>
  networks: NetView[]
  limits: LimitsView
  selected: number | null
  view: View
  adding: boolean
  /** A link handed to the dialog by paste or drop. */
  draft: string
  toast: (UiError & { at: number }) | null
  theme: Theme
  start(): Promise<void>
  select(id: number | null): void
  setView(v: View): void
  setAdding(open: boolean, draft?: string): void
  setTheme(t: Theme): void
  dismissToast(): void
  refreshNetworks(): Promise<void>
  /** Saves limits; true when the backend accepted them. */
  saveLimits(next: LimitsView): Promise<boolean>
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
  live: {},
  history: {},
  networks: [],
  limits: { global: 0, networks: [] },
  selected: null,
  view: 'transfers',
  adding: false,
  draft: '',
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
        backend.getLimits().catch(() => ({ global: 0, networks: [] })),
      ])
      set({ info, networks, limits })
      await backend.subscribe((e) => {
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
  select: (id) => set({ selected: id }),
  setView: (view) => set({ view }),
  setAdding: (adding, draft) => set({ adding, draft: draft ?? '' }),
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
