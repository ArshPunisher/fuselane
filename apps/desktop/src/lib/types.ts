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
  position: number
  verify: boolean
  /** This download's own speed limit in bytes per second; 0 = none. */
  speedLimit: number
}

export interface NetView {
  name: string
  label: string
  kind: string
  usable: boolean
  addrs: string[]
  /** Whether it really reaches the internet; 'portal' means a sign-in page is in the way. */
  reach: 'online' | 'portal' | 'offline' | null
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

export type UiEvent =
  | { type: 'jobs'; jobs: JobView[] }
  | ({ type: 'live' } & Live)
  | { type: 'torrents'; torrents: TorrentView[] }
  | { type: 'open'; target: string }
  | { type: 'networks'; networks: NetView[] }
  | { type: 'whenDone'; action: WhenDone; seconds: number }
  | { type: 'whenDoneCancelled' }
  | { type: 'automation'; view: AutomationView }
  | { type: 'sends'; shares: ShareView[]; receives: ReceiveView[] }

/** A file this computer is sending with Fuse Send. */
export interface ShareView {
  /** The info-hash once ready; a temporary id while preparing. */
  id: string
  name: string
  size: number
  /** The link to give the receiver, once ready. */
  link: string | null
  state: 'preparing' | 'sharing' | 'changed' | 'failed'
  /** How far preparing has got, 0 to 1. */
  prepared: number
  /** Bytes sent to receivers so far. */
  sent: number
  peers: number
  error: string | null
}

/** A file arriving from someone's Fuse Send link. */
export interface ReceiveView {
  id: string
  name: string
  size: number
  done: number
  state: 'finding' | 'receiving' | 'checking' | 'done' | 'failed'
  /** Where it was saved, once done. */
  path: string | null
  error: string | null
}

export type WhenDone = 'nothing' | 'sleep' | 'shut-down' | 'quit'

export interface Schedule {
  enabled: boolean
  /** Minutes after midnight. */
  start: number
  stop: number
  /** Monday first. */
  days: boolean[]
}

export interface Automation {
  schedule: Schedule
  whenDone: WhenDone
  keepAwake: boolean
  sortByType: boolean
}

export interface AutomationView {
  settings: Automation
  allowedNow: boolean
  /** "Starts at 01:00" / "Pauses at 07:00" when a schedule is on. */
  next: string | null
}

export interface UiError {
  code: string
  message: string
  hint: string | null
}

/** Choices in the New download dialog besides the link and folder. */
export interface AddOptions {
  name?: string | null
  sha256?: string | null
  /** Add a link that is already in the list (after the person confirms). */
  allowDuplicate?: boolean
  /** Add it paused, to start later. */
  later?: boolean
}

export interface BatchResult {
  added: number[]
  skipped: { url: string; reason: string }[]
}

export interface WindowPrefs {
  startAtLogin: boolean
  closeToTray: boolean
  /** Offer download links copied anywhere (opt-in). */
  watchClipboard: boolean
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

export type TorrentStatus =
  'checking' | 'downloading' | 'paused' | 'seeding' | 'completed' | 'failed'

/** A torrent in the list (apps/desktop/src-tauri/src/torrents.rs). */
export interface TorrentView {
  /** The info hash: stable across restarts. */
  id: string
  name: string
  folder: string
  status: TorrentStatus
  done: number
  total: number
  uploaded: number
  rate: number
  error: string | null
  fileCount: number
  selectedCount: number
  networks: TorrentNetView[]
  addedAt: number
}

export interface TorrentNetView {
  name: string
  peers: number
  received: number
  /** Bytes per second received on this network just now. */
  rate: number
  /** Verified bytes credited to this network; all networks sum to `done`. */
  credited: number
}

export interface TorrentFileView {
  index: number
  path: string
  size: number
  selected: boolean
}

/** What a torrent holds before it starts; the user picks files from this. */
export interface ListingView {
  token: string
  name: string
  folder: string
  total: number
  files: TorrentFileView[]
}

/** Sharing back after a download: off by default; stops at whichever limit comes first. */
export interface SeedSettings {
  enabled: boolean
  /** Stop after uploading this many times the download's size (0.1 to 10). */
  ratio: number
  /** Stop after sharing this many minutes (1 to 10080). */
  minutes: number
}
