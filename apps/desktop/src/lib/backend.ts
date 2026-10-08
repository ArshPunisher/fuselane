// The one door to the backend. In the Tauri window it calls Rust commands; in a
// plain browser (pnpm dev, Playwright) it uses the demo engine in ./demo.ts,
// and the UI says so.
import type { AppInfo, JobView, NetView, PreviewView, UiError, UiEvent } from './types'

export interface Backend {
  readonly demo: boolean
  appInfo(): Promise<AppInfo>
  listNetworks(): Promise<NetView[]>
  add(url: string, dir: string | null): Promise<number>
  pause(id: number): Promise<void>
  resume(id: number): Promise<void>
  remove(id: number): Promise<void>
  /** Shows a finished file in the file manager. */
  reveal(id: number): Promise<void>
  openFile(id: number): Promise<void>
  /** A folder the user picked, or null if they cancelled. */
  pickFolder(): Promise<string | null>
  /** Continues a stopped download from a new link to the same file. */
  fixLink(id: number, url: string): Promise<void>
  /** Discards a download that can't continue and starts it fresh; the new id. */
  startOver(id: number): Promise<number>
  /** Name and size of what a link points at, without downloading it. */
  preview(url: string): Promise<PreviewView>
  subscribe(onEvent: (e: UiEvent) => void): Promise<void>
  listJobs(): Promise<JobView[]>
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
    add: (url, dir) => call('add_download', { url, dir }),
    pause: (id) => call('pause', { id }),
    resume: (id) => call('resume', { id }),
    remove: (id) => call('remove', { id }),
    reveal: (id) => call('reveal', { id }),
    openFile: (id) => call('open_file', { id }),
    pickFolder: () => call('pick_folder'),
    fixLink: (id, url) => call('fix_link', { id, url }),
    startOver: (id) => call('start_over', { id }),
    preview: (url) => call('preview', { url }),
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
