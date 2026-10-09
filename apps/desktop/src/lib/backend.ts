// The one door to the backend. In the Tauri window it calls Rust commands; in a
// plain browser (pnpm dev, Playwright) it uses the demo engine in ./demo.ts,
// and the UI says so.
import type {
  HaveView,
  NetCheckView,
  PageFiles,
  NearbyView,
  FilePriority,
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

/** A drag of files over the window (desktop app only). */
export interface FileDrop {
  type: 'over' | 'drop' | 'leave'
  paths: string[]
  x: number
  y: number
}

export interface Backend {
  readonly demo: boolean
  appInfo(): Promise<AppInfo>
  listNetworks(): Promise<NetView[]>
  add(url: string, dir: string | null, options?: AddOptions): Promise<number>
  /** Adds every link in pasted text (patterns like file[01-20].zip expanded). */
  /** `group`: a name (empty for one made up) to keep them together, or null. */
  addBatch(
    text: string,
    dir: string | null,
    later?: boolean,
    group?: string | null,
  ): Promise<BatchResult>
  /** Reads a web page and lists the files it links to; nothing is downloaded. */
  filesOnPage(url: string): Promise<PageFiles>
  renameGroup(id: number, name: string): Promise<void>
  ungroup(id: number): Promise<void>
  pauseGroup(id: number): Promise<void>
  resumeGroup(id: number): Promise<void>
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
  /** Gives one download every network; the others wait and carry on after it. */
  /** A finished download with this name and size that's still on disk. */
  alreadyHave(name: string, size: number | null): Promise<HaveView | null>
  /** When a download should be finished (unix seconds), or null to clear it. */
  setReadyBy(id: number, at: number | null): Promise<void>
  focus(id: number): Promise<void>
  unfocus(): Promise<void>
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
  /** Look for a published SHA-256 next to each new download (on by default). */
  /** "Long" for networks used only for long downloads, in minutes. */
  longMinutes(): Promise<number>
  setLongMinutes(minutes: number): Promise<number>
  findChecksums(): Promise<boolean>
  setFindChecksums(on: boolean): Promise<boolean>
  setPerNetworkDns(on: boolean): Promise<boolean>
  allowances(): Promise<AllowanceView[]>
  setAllowance(req: AllowanceRequest): Promise<AllowanceView[]>
  setSlow(on: boolean): Promise<LimitsView>
  networkPrefs(): Promise<NetPref[]>
  setNetworkPref(pref: NetPref): Promise<NetPref[]>
  /** A privacy-safe report for bug reports. */
  diagnostics(): Promise<string>
  /** Network check (B10.1): results so far, start, stop, and the report. */
  netCheckState(): Promise<NetCheckView>
  netCheckStart(): Promise<NetCheckView>
  netCheckCancel(): Promise<void>
  /** Writes the report for an internet provider, opens it, returns its path. */
  netCheckReport(): Promise<string>
  /** A crash report from last time, offered once; null when there was none. */
  unseenCrash(): Promise<string | null>
  /** Opens a filled-in GitHub issue for the person to read and submit. */
  reportProblem(crash: string | null): Promise<void>
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
  /** High fetches first, Low waits for the rest, Skip leaves the file out. */
  setFilePriority(id: string, file: number, priority: FilePriority): Promise<void>
  /** Plays a file while it downloads; returns the local stream link. */
  playTorrentFile(id: string, file: number): Promise<string>
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
  // Nearby (B8.11)
  nearbyStart(): Promise<NearbyView>
  nearbyState(): Promise<NearbyView>
  nearbySetEveryone(on: boolean): Promise<NearbyView>
  nearbyPick(): Promise<string[]>
  nearbySend(fingerprint: string, paths: string[]): Promise<NearbyView>
  /** Hands a paused download to another Fuselane, which carries on with it. */
  nearbyHandoff(id: number, fingerprint: string): Promise<NearbyView>
  nearbyAnswer(id: number, accept: boolean, trust: boolean): Promise<NearbyView>
  nearbyForget(fingerprint: string): Promise<NearbyView>
  nearbyCancel(id: string): Promise<void>
  nearbyClear(id: string): Promise<NearbyView>
  nearbyReveal(id: string): Promise<void>
  nearbyPhone(on: boolean): Promise<NearbyView>
  /**
   * Files dragged over or dropped on the window, with their paths and where
   * (CSS pixels). Only the desktop app has paths; the demo never calls back.
   */
  onFileDrop(cb: (e: FileDrop) => void): () => void
  nearbyPhoneOffer(add: string[], remove: string | null): Promise<NearbyView>
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
    addBatch: (text, dir, later, group) =>
      call('add_batch', { text, dir, later: later ?? false, group: group ?? null }),
    filesOnPage: (url) => call('files_on_page', { url }),
    renameGroup: (id, name) => call('rename_group', { id, name }),
    ungroup: (id) => call('ungroup', { id }),
    pauseGroup: (id) => call('pause_group', { id }),
    resumeGroup: (id) => call('resume_group', { id }),
    maxRunning: () => call('max_running'),
    setMaxRunning: (n) => call('set_max_running', { n }),
    reorder: (ids) => call('reorder', { ids }),
    focus: (id) => call('focus', { id }),
    setReadyBy: (id, at) => call('set_ready_by', { id, at }),
    alreadyHave: (name, size) => call('already_have', { name, size }),
    unfocus: () => call('unfocus'),
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
    findChecksums: () => call('find_checksums'),
    longMinutes: () => call('long_minutes'),
    setLongMinutes: (minutes) => call('set_long_minutes', { minutes }),
    setFindChecksums: (on) => call('set_find_checksums', { on }),
    setPerNetworkDns: (on) => call('set_per_network_dns', { on }),
    allowances: () => call('allowances'),
    setAllowance: (req) => call('set_allowance', { req }),
    setSlow: (on) => call('set_slow', { on }),
    networkPrefs: () => call('network_prefs'),
    setNetworkPref: (pref) => call('set_network_pref', { pref }),
    diagnostics: () => call('diagnostics'),
    unseenCrash: () => call('unseen_crash'),
    netCheckState: () => call('netcheck_state'),
    netCheckStart: () => call('netcheck_start'),
    netCheckCancel: () => call('netcheck_cancel'),
    netCheckReport: () => call('netcheck_report'),
    reportProblem: (crash) => call('report_problem', { crash }),
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
    setFilePriority: (id, file, priority) => call('torrent_set_priority', { id, file, priority }),
    playTorrentFile: (id, file) => call('torrent_play', { id, file }),
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
    nearbyStart: () => call('nearby_start'),
    nearbyHandoff: (id, fingerprint) => call('nearby_handoff', { id, fingerprint }),
    nearbyState: () => call('nearby_state'),
    nearbySetEveryone: (on) => call('nearby_set_everyone', { on }),
    nearbyPick: () => call('nearby_pick'),
    nearbySend: (fingerprint, paths) => call('nearby_send', { fingerprint, paths }),
    nearbyAnswer: (id, accept, trust) => call('nearby_answer', { id, accept, trust }),
    nearbyForget: (fingerprint) => call('nearby_forget', { fingerprint }),
    nearbyCancel: (id) => call('nearby_cancel', { id }),
    nearbyClear: (id) => call('nearby_clear', { id }),
    nearbyReveal: (id) => call('nearby_reveal', { id }),
    nearbyPhone: (on) => call('nearby_phone', { on }),
    onFileDrop: (cb) => {
      let stop: (() => void) | null = null
      let gone = false
      void import('@tauri-apps/api/webview').then(({ getCurrentWebview }) =>
        getCurrentWebview()
          .onDragDropEvent((e) => {
            const p = e.payload
            const at = (pos: { x: number; y: number }) => ({
              x: pos.x / devicePixelRatio,
              y: pos.y / devicePixelRatio,
            })
            if (p.type === 'leave') cb({ type: 'leave', paths: [], x: 0, y: 0 })
            else if (p.type === 'enter') cb({ type: 'over', paths: p.paths, ...at(p.position) })
            else if (p.type === 'over') cb({ type: 'over', paths: [], ...at(p.position) })
            else if (p.type === 'drop') cb({ type: 'drop', paths: p.paths, ...at(p.position) })
          })
          .then((un) => (gone ? un() : (stop = un))),
      )
      return () => {
        gone = true
        stop?.()
      }
    },
    nearbyPhoneOffer: (add, remove) => call('nearby_phone_offer', { add, remove }),
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
