// Shapes the backend sends (apps/desktop/src-tauri/src/service.rs). Keep in step.

export type JobStatus =
  'queued' | 'running' | 'paused' | 'failed' | 'failed-final' | 'completed' | 'cancelled'

/** The fix the UI offers for a download's error. */
export type ErrorAction =
  | 'fix-link'
  | 'retry'
  | 'start-over'
  | 'free-space'
  | 'allowance'
  | 'schedule'
  | 'unpack'
  | 'focus'
  | 'battery'
  | 'handoff'
  | null

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
  errorAction: ErrorAction
  finalPath: string | null
  createdAt: number
  position: number
  verify: boolean
  /** This download's own speed limit in bytes per second; 0 = none. */
  speedLimit: number
  /** Seconds until Fuselane tries this failed download again by itself. */
  retryIn: number | null
  /** When it starts by itself (unix seconds), while it waits paused. */
  startAt: number | null
  /** Hosts of its mirrors, and why any of them wasn't used. */
  mirrors: string[]
  mirrorNotes: string[]
  /** It has every network to itself ("Do this one now"). */
  focused: boolean
  /** Where its checksum was found by itself ("SHA256SUMS"), if it was. */
  checksumFrom: string | null
  /** Finished and matched its SHA-256. */
  verified: boolean
  /** What each network carried and saved in the finishing run. */
  report: ReportView | null
  /** When it should be finished (unix seconds), and how that looks. */
  readyBy: number | null
  readyState: 'on-track' | 'at-risk' | 'missed' | null
  /** The group it was added in, and its name. */
  groupId: number | null
  groupName: string | null
}

export interface ReportView {
  /** How long the finishing run took. */
  secs: number
  nets: { label: string; bytes: number; savedSecs: number | null }[]
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
  | { type: 'update'; progress: UpdateProgress }
  | { type: 'nearby'; view: NearbyView }
  | { type: 'netCheck'; view: NetCheckView }

/** A device on the network (Nearby, B8.11). */
export interface DeviceView {
  fingerprint: string
  alias: string
  /** mobile | desktop | web | headless | server */
  kind: string
  model: string | null
  trusted: boolean
  /** Another Fuselane, verified by its certificate. */
  fuselane: boolean
}

export interface TrustedDevice {
  fingerprint: string
  alias: string
  /** yyyy-mm-dd */
  since: string
}

/** Someone asking to send files here. */
export interface NearbyRequest {
  id: number
  alias: string
  kind: string
  model: string | null
  files: string[]
  total: number
  verified: boolean
  /** A text message instead of files: the text. */
  text: string | null
}

export interface NearbyTransfer {
  id: string
  direction: 'out' | 'in'
  device: string
  name: string
  size: number
  done: number
  state: 'asking' | 'sending' | 'receiving' | 'done' | 'declined' | 'failed' | 'cancelled'
  error: string | null
  path: string | null
  /** A text message instead of a file: the text. */
  text: string | null
}

/** The phone page while it's on (B8.12). */
export interface PhoneView {
  url: string
  /** The link as a QR code (SVG made by the app). */
  qr: string
  offers: { id: string; name: string; size: number }[]
  /** Text offered to the phone (B10.7). */
  text: string | null
}

export interface NearbyView {
  on: boolean
  me: string
  /** Seconds left of "Everyone"; null when only trusted devices can see this computer. */
  everyoneFor: number | null
  devices: DeviceView[]
  trusted: TrustedDevice[]
  transfers: NearbyTransfer[]
  request: NearbyRequest | null
  problem: string | null
  phone: PhoneView | null
  /** Folders kept in sync with trusted computers (B10.3). */
  syncs: SyncView[]
}

export interface SyncView {
  id: number
  folder: string
  name: string
  device: string
  state: 'up-to-date' | 'sending' | 'waiting' | 'problem'
  files: number
  pending: number
  note: string | null
}

/** A file this computer is sending with Fuse Send. */
export interface ShareView {
  /** The info-hash once ready; a temporary id while preparing. */
  id: string
  name: string
  size: number
  /** The link to give the receiver, once ready. */
  link: string | null
  /** `sent`: stopped by itself after one full copy went out. */
  state: 'preparing' | 'sharing' | 'sent' | 'changed' | 'failed'
  /** How far preparing has got, 0 to 1. */
  prepared: number
  /** Bytes sent to receivers so far. */
  sent: number
  peers: number
  /** Stop sharing once a full copy has been sent. */
  once: boolean
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

export type NameTaken = 'ask' | 'keep-both' | 'replace'
export type AfterDownload = 'nothing' | 'open' | 'unpack'

export interface Automation {
  schedule: Schedule
  whenDone: WhenDone
  keepAwake: boolean
  sortByType: boolean
  /** When a file of the same name is already there (B8.6). */
  nameTaken: NameTaken
  /** What happens to each finished download (B8.7). */
  afterDownload: AfterDownload
  /** On battery and low (B9.10). */
  lowBattery: LowBattery
}

export type LowBattery = 'keep-going' | 'leave-out-phone' | 'pause'

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
  /** Start by itself at this time (unix seconds). */
  startAt?: number | null
  /** The name is taken: true replaces the old file, false keeps both; unset follows Settings. */
  replace?: boolean | null
  /** Other links to the same file. */
  mirrors?: string[]
}

export interface BatchResult {
  added: number[]
  skipped: { url: string; reason: string }[]
  /** The group they were put in, if one was asked for and two or more were added. */
  group: number | null
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
  /** Size of the download, when the server says. */
  size: number | null
}

/** The app's own update downloading or installing (B8.4). */
export interface UpdateProgress {
  phase: 'downloading' | 'installing'
  done: number
  total: number | null
  rate: number
  /** Networks carrying it; 1 when the fallback download is used. */
  networks: number
}

/** A network's name and colour as the user chose them (by device name). */
export interface NetPref {
  name: string
  label: string | null
  lane: string | null
  /** When it helps: always, only for long downloads, or never. */
  useFor: NetUse
  /** Only used between these times every day (minutes after midnight). */
  hours: { start: number; stop: number } | null
}

export type NetUse = 'always' | 'long' | 'never'

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

/** One peer connected to a torrent. */
export interface PeerView {
  addr: string
  /** The app it says it is, when it tells. */
  client: string | null
  /** Device name of our network it's on; null when it connected to us. */
  network: string | null
  /** Bytes per second from it and to it. */
  down: number
  up: number
  received: number
  sent: number
  kind: string
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

export type FilePriority = 'skip' | 'low' | 'normal' | 'high'

export interface TorrentFileView {
  index: number
  path: string
  size: number
  selected: boolean
  /** Bytes of it that have arrived and passed their check. */
  done: number
  priority: FilePriority
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

/** A finished download with the same name and size, still on disk. */
export interface HaveView {
  id: number
  name: string
  path: string
  size: number
  /** When it finished (unix seconds). */
  finishedAt: number
}

/** The files a web page links to (B9.3). */
export interface PageFiles {
  title: string | null
  files: { url: string; name: string }[]
}

/** One network's numbers from a network check (B10.1). */
export interface NetResult {
  name: string
  label: string
  kind: string
  /** Bytes per second. */
  downBps: number | null
  idleMs: number | null
  jitterMs: number | null
  /** 0..1 */
  loss: number | null
  loadedMs: number | null
  /** Bufferbloat, A+ to F. */
  grade: string | null
  dnsMs: number | null
  problem: string | null
}

export interface CheckRun {
  at: number
  results: NetResult[]
  togetherBps: number | null
}

export interface Outage {
  name: string
  label: string
  kind: 'offline' | 'sign-in'
  from: number
  to: number | null
}

export interface NetCheckView {
  running: boolean
  phase: string | null
  current: CheckRun | null
  history: CheckRun[]
  outages: Outage[]
}

/** A video page's choices, from yt-dlp (B10.4). */
export interface MediaInfo {
  title: string
  site: string
  duration: number | null
  options: { id: string; label: string; detail: string; size: number | null }[]
  /** HD needs ffmpeg to join video and audio. */
  hdNeedsFfmpeg: boolean
}

export interface MediaTools {
  ytDlp: string | null
  ffmpeg: string | null
}

/** Bytes per network per day (B10.5). */
/** Remote control for aria2 tools (8.7, ADR 0013). */
export interface RemoteView {
  on: boolean
  /** Devices on the local network may connect too. */
  lan: boolean
  port: number
  secret: string
  /** Addresses to give an aria2 tool, this computer's first. */
  urls: string[]
  /** Why it isn't running although it's on. */
  problem: string | null
}

export interface UsageHistory {
  days: { day: string; nets: Record<string, number> }[]
  labels: Record<string, string>
}
