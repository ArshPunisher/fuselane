// Shapes the backend sends (apps/desktop/src-tauri/src/service.rs). Keep in step.

export type JobStatus =
  'queued' | 'running' | 'paused' | 'failed' | 'failed-final' | 'completed' | 'cancelled'

export interface JobView {
  id: number
  url: string
  name: string
  dir: string
  status: JobStatus
  resumable: boolean
  written: number
  total: number | null
  error: string | null
  /** The fix to offer for `error`. */
  errorAction: 'fix-link' | 'retry' | 'start-over' | 'free-space' | 'allowance' | null
  finalPath: string | null
  createdAt: number
}

export interface NetView {
  name: string
  label: string
  kind: string
  usable: boolean
  addrs: string[]
}

export interface LiveNet {
  name: string
  label: string
  kind: string
  bytes: number
  rate: number
  streams: number
  dead: boolean
}

export interface Live {
  id: number
  written: number
  total: number | null
  rate: number
  networks: LiveNet[]
  /** Triples per tick: fill 0..100, owner (network index + 1, 0 none), in-flight (same). */
  ticks: number[]
  retries: number
  hedges: number
}

export type UiEvent = { type: 'jobs'; jobs: JobView[] } | ({ type: 'live' } & Live)

export interface UiError {
  code: string
  message: string
  hint: string | null
}

export interface AppInfo {
  version: string
  defaultDir: string
  /** Set on the first launch after an update. */
  updatedFrom: string | null
}

export interface PreviewView {
  filename: string
  total: number | null
  splittable: boolean
}

/** Speed limits in bytes per second; 0 means no limit. */
export interface LimitsView {
  global: number
  networks: NetLimit[]
  /** Slow mode: a temporary overall cap that leaves `global` untouched. */
  slow: boolean
  /** The slow-mode cap in bytes per second. */
  slowRate: number
}

export interface NetLimit {
  name: string
  rate: number
}

export interface UpdateInfo {
  version: string
  notes: string | null
}

/** A network's name and colour as the user chose them (by device name). */
export interface NetPref {
  name: string
  label: string | null
  lane: string | null
}

/** A network's monthly data allowance and its usage this period. */
export interface AllowanceView {
  name: string
  allowance: number | null
  resetDay: number
  used: number
  /** Next reset, YYYY-MM-DD. */
  resetsOn: string
  reached: boolean
}

/** Sets (bytes > 0) or removes (bytes = 0) an allowance. */
export interface AllowanceRequest {
  name: string
  bytes: number
  resetDay: number
}
