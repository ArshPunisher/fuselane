// The one door to the backend. In the Tauri window it calls Rust commands; in a
// plain browser (pnpm dev, Playwright) it uses the demo engine in ./demo.ts,
// and the UI says so.
import type {
  AddOptions,
  AllowanceRequest,
  AllowanceView,
  AppInfo,
  WindowPrefs,
  Automation,
  AutomationView,
  BatchResult,
  JobView,
  ListingView,
  LimitsView,
  NetPref,
  NetView,
  PeerView,
  PreviewView,
  ReceiveView,
  SeedSettings,
  ShareView,
  TorrentFileView,
  TorrentView,
  UiError,
  UiEvent,
  UpdateInfo,
} from './types'

export interface Backend {
  readonly demo: boolean
  appInfo(): Promise<AppInfo>
  listNetworks(): Promise<NetView[]>
  add(url: string, dir: string | null, options?: AddOptions): Promise<number>
  /** Adds every link in pasted text (patterns like file[01-20].zip expanded). */
  addBatch(text: string, dir: string | null, later?: boolean): Promise<BatchResult>
  /** Saves every link to a file the person picks; how many, or null if cancelled. */
  exportLinks(): Promise<number | null>
  /** Adds the links in a file the person picks, paused; null if cancelled. */
  importLinks(): Promise<BatchResult | null>
  /** How many downloads run at once (1–8). */
  maxRunning(): Promise<number>
  setMaxRunning(n: number): Promise<number>
  /** One download's own speed limit in bytes/s (0 removes it); applies at once. */
  setJobLimit(id: number, rate: number): Promise<void>
  /** Puts these downloads first in the queue, in this order. */
  reorder(ids: number[]): Promise<void>
  automation(): Promise<AutomationView>
  setAutomation(settings: Automation): Promise<AutomationView>
  /** Stops a pending sleep, shut-down or quit. */
  cancelWhenDone(): Promise<void>
  windowPrefs(): Promise<WindowPrefs>
  setStartAtLogin(on: boolean): Promise<boolean>
  setCloseToTray(on: boolean): Promise<boolean>
  setWatchClipboard(on: boolean): Promise<boolean>
  pause(id: number): Promise<void>
  resume(id: number): Promise<void>
  remove(id: number): Promise<void>
  /** Shows a finished file in the file manager. */
  reveal(id: number): Promise<void>
  /** Moves a finished download's file to the Trash and removes it from the list. */
  trashFile(id: number): Promise<void>
  openFile(id: number): Promise<void>
  /** A folder the user picked, or null if they cancelled. */
  pickFolder(): Promise<string | null>
  getLimits(): Promise<LimitsView>
  perNetworkDns(): Promise<boolean>
  setPerNetworkDns(on: boolean): Promise<boolean>
  allowances(): Promise<AllowanceView[]>
  setAllowance(req: AllowanceRequest): Promise<AllowanceView[]>
  setSlow(on: boolean): Promise<LimitsView>
  networkPrefs(): Promise<NetPref[]>
  setNetworkPref(pref: NetPref): Promise<NetPref[]>
  /** A privacy-safe report for bug reports. */
  diagnostics(): Promise<string>
  /** A newer signed version, or null when up to date. */
  checkUpdate(): Promise<UpdateInfo | null>
  /** Pauses downloads, installs the update and restarts the app. */
  installUpdate(): Promise<void>
  cancelUpdate(): Promise<void>
  /** Is a file of this name already in the folder a new download would save to? */
  nameTaken(dir: string | null, name: string): Promise<boolean>
  /** Opens this version's release notes in the browser. */
  openReleaseNotes(): Promise<void>
  /** Saves limits (validated by the backend); returns what was saved. */
  setLimits(limits: LimitsView): Promise<LimitsView>
  /** Continues a stopped download from a new link to the same file. */
  fixLink(id: number, url: string): Promise<void>
  /** Discards a download that can't continue and starts it fresh; the new id. */
  startOver(id: number): Promise<number>
  /** Name and size of what a link points at, without downloading it. */
  preview(url: string): Promise<PreviewView>
  subscribe(onEvent: (e: UiEvent) => void): Promise<void>
  listJobs(): Promise<JobView[]>
  /** Opens a web page so a hotel or café network shows its sign-in page. */
  openSignIn(): Promise<void>
  /** A .torrent file the user picked, or null if they cancelled. */
  pickTorrent(): Promise<string | null>
  /** Reads a magnet's file list (from peers; can take a while). Writes nothing. */
  inspectMagnet(magnet: string, dir: string | null): Promise<ListingView>
  inspectTorrentFile(path: string, dir: string | null): Promise<ListingView>
  /** Starts an inspected torrent with the chosen files; its id. */
  addTorrent(token: string, files: number[]): Promise<string>
  listTorrents(): Promise<TorrentView[]>
  torrentFiles(id: string): Promise<TorrentFileView[]>
  pauseTorrent(id: string): Promise<void>
  resumeTorrent(id: string): Promise<void>
  selectTorrentFiles(id: string, files: number[]): Promise<void>
  removeTorrent(id: string, deleteFiles: boolean): Promise<void>
  revealTorrent(id: string): Promise<void>
  seedSettings(): Promise<SeedSettings>
  /** Saves sharing settings (validated by the backend); returns what was saved. */
  setSeedSettings(s: SeedSettings): Promise<SeedSettings>
  /** The peers connected to a torrent now, fastest first. */
  torrentPeers(id: string): Promise<PeerView[]>
  /** How complete each of `cells` slices of a torrent is (0 to 100). */
  torrentPieces(id: string, cells: number): Promise<number[]>
  /** Stops sharing a finished torrent now; its files stay. */
  stopSharing(id: string): Promise<void>
  /** A dropped .torrent's contents (the page can't see its path). */
  inspectTorrentBytes(bytes: Uint8Array, dir: string | null): Promise<ListingView>
  /** Fuse Send: a file the user picked to send, or null if they cancelled. */
  pickSendFile(): Promise<string | null>
  /** Starts sharing a file; progress and the link arrive as `sends` events. */
  sendFile(path: string): Promise<string>
  sendsState(): Promise<[ShareView[], ReceiveView[]]>
  /** Stops sharing; the file itself stays. */
  stopSend(id: string): Promise<void>
  /** Stop sharing by itself once a full copy has been sent. */
  sendOnce(id: string, on: boolean): Promise<void>
  /** Starts receiving from a link into `dir` (or the download folder). */
  receiveLink(link: string, dir: string | null): Promise<string>
  /** Removes a finished or failed receive from the list; the file stays. */
  dismissReceive(id: string): Promise<void>
  /** Shows a received file in the file manager. */
  revealReceived(id: string): Promise<void>
}

/** Turns anything thrown across IPC into a UiError the UI can show. */
export function toUiError(e: unknown): UiError {
  if (e && typeof e === 'object' && 'message' in e && 'code' in e) {
    const o = e as { code: unknown; message: unknown; hint?: unknown }
    return {
      code: String(o.code),
      message: String(o.message),
      hint: typeof o.hint === 'string' ? o.hint : null,
    }
  }
  const text = typeof e === 'string' ? e : e instanceof Error ? e.message : 'Something went wrong.'
  return {
    code: 'unexpected',
    message: text || 'Something went wrong.',
    hint: 'Try again. If it keeps happening, restart Fuselane.',
  }
}

async function tauriBackend(): Promise<Backend> {
  const { invoke, Channel } = await import('@tauri-apps/api/core')
  const call = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    try {
      return await invoke<T>(cmd, args)
    } catch (e) {
      throw toUiError(e)
    }
  }
  return {
    demo: false,
    appInfo: () => call('app_info'),
    listJobs: () => call('list_jobs'),
    listNetworks: () => call('list_networks'),
    add: (url, dir, options) => call('add_download', { url, dir, options: options ?? null }),
    exportLinks: () => call('export_links'),
    importLinks: () => call('import_links'),
    addBatch: (text, dir, later) => call('add_batch', { text, dir, later: later ?? false }),
    maxRunning: () => call('max_running'),
    setMaxRunning: (n) => call('set_max_running', { n }),
    reorder: (ids) => call('reorder', { ids }),
    setJobLimit: (id, rate) => call('set_job_limit', { id, rate }),
    automation: () => call('automation'),
    setAutomation: (settings) => call('set_automation', { settings }),
    cancelWhenDone: () => call('cancel_when_done'),
    windowPrefs: () => call('window_prefs'),
    setStartAtLogin: (on) => call('set_start_at_login', { on }),
    setCloseToTray: (on) => call('set_close_to_tray', { on }),
    setWatchClipboard: (on) => call('set_watch_clipboard', { on }),
    pause: (id) => call('pause', { id }),
    resume: (id) => call('resume', { id }),
    remove: (id) => call('remove', { id }),
    reveal: (id) => call('reveal', { id }),
    trashFile: (id) => call('trash_file', { id }),
    openFile: (id) => call('open_file', { id }),
    pickFolder: () => call('pick_folder'),
    getLimits: () => call('get_limits'),
    perNetworkDns: () => call('per_network_dns'),
    setPerNetworkDns: (on) => call('set_per_network_dns', { on }),
    allowances: () => call('allowances'),
    setAllowance: (req) => call('set_allowance', { req }),
    setSlow: (on) => call('set_slow', { on }),
    networkPrefs: () => call('network_prefs'),
    setNetworkPref: (pref) => call('set_network_pref', { pref }),
    diagnostics: () => call('diagnostics'),
    checkUpdate: () => call('check_update'),
    installUpdate: () => call('install_update'),
    cancelUpdate: () => call('cancel_update'),
    nameTaken: (dir, name) => call('name_taken', { dir, name }),
    openReleaseNotes: () => call('open_release_notes'),
    setLimits: (limits) => call('set_limits', { limits }),
    fixLink: (id, url) => call('fix_link', { id, url }),
    startOver: (id) => call('start_over', { id }),
    preview: (url) => call('preview', { url }),
    pickTorrent: () => call('pick_torrent'),
    openSignIn: () => call('open_sign_in'),
    inspectMagnet: (magnet, dir) => call('torrent_inspect_magnet', { magnet, dir }),
    inspectTorrentFile: (path, dir) => call('torrent_inspect_file', { path, dir }),
    inspectTorrentBytes: (bytes, dir) =>
      call('torrent_inspect_bytes', { bytes: Array.from(bytes), dir }),
    addTorrent: (token, files) => call('torrent_add', { token, files }),
    listTorrents: () => call('torrent_list'),
    torrentFiles: (id) => call('torrent_files', { id }),
    pauseTorrent: (id) => call('torrent_pause', { id }),
    resumeTorrent: (id) => call('torrent_resume', { id }),
    selectTorrentFiles: (id, files) => call('torrent_select', { id, files }),
    removeTorrent: (id, deleteFiles) => call('torrent_remove', { id, deleteFiles }),
    revealTorrent: (id) => call('torrent_reveal', { id }),
    seedSettings: () => call('torrent_seed_settings'),
    setSeedSettings: (settings) => call('set_torrent_seed_settings', { settings }),
    stopSharing: (id) => call('torrent_stop_sharing', { id }),
    torrentPeers: (id) => call('torrent_peers', { id }),
    torrentPieces: (id, cells) => call('torrent_pieces', { id, cells }),
    pickSendFile: () => call('send_pick'),
    sendFile: (path) => call('send_file', { path }),
    sendsState: () => call('sends_state'),
    stopSend: (id) => call('stop_send', { id }),
    sendOnce: (id, on) => call('send_once', { id, on }),
    receiveLink: (link, dir) => call('receive_link', { link, dir }),
    dismissReceive: (id) => call('dismiss_receive', { id }),
    revealReceived: (id) => call('reveal_received', { id }),
    subscribe: async (onEvent) => {
      const channel = new Channel<UiEvent>()
      channel.onmessage = onEvent
      await call('subscribe', { channel })
    },
  }
}

export async function connect(): Promise<Backend> {
  if ('__TAURI_INTERNALS__' in window) return tauriBackend()
  const { createDemoBackend } = await import('./demo')
  return createDemoBackend(new URLSearchParams(location.search))
}
