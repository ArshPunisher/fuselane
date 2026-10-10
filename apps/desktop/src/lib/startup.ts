// When the download list can't be opened at launch, the desktop app opens in a
// "problem" mode (src-tauri/src/startup.rs): no service, just a few commands to
// explain the problem and, for a list from a newer Fuselane, to update. The demo
// shows it with ?startup=newer or ?startup=other (lib/demoStartup.ts).
import type { UpdateInfo, UpdateProgress } from './types'
import { toUiError } from './backend'

/** What the backend says went wrong (startup::Problem). */
export interface StartupProblem {
  /** newer: the list was written by a newer Fuselane. other: anything else. */
  kind: 'newer' | 'other'
  message: string
  hint: string | null
  /** The error as the system gave it, in English. */
  detail: string
  /** This copy's version. */
  version: string
  /** Fuselane's data folder, when there is one. */
  home: string | null
}

export interface StartupBackend {
  readonly demo: boolean
  readonly problem: StartupProblem
  /** A newer version on the update feed, or null. */
  checkUpdate(): Promise<UpdateInfo | null>
  /** Downloads, checks and installs the update, then restarts into it. */
  installUpdate(onProgress: (p: UpdateProgress) => void): Promise<void>
  cancelUpdate(): Promise<void>
  revealHome(): Promise<void>
  /** Opens fuselane.app's download page. */
  getFuselane(): Promise<void>
  copyDetails(): Promise<void>
  quit(): Promise<void>
}

async function tauriStartup(): Promise<StartupBackend | null> {
  const { invoke, Channel } = await import('@tauri-apps/api/core')
  const call = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    try {
      return await invoke<T>(cmd, args)
    } catch (e) {
      throw toUiError(e)
    }
  }
  const problem = await invoke<StartupProblem | null>('startup_problem').catch(() => null)
  if (!problem) return null
  return {
    demo: false,
    problem,
    checkUpdate: () => call('check_update'),
    installUpdate: (onProgress) => {
      const progress = new Channel<UpdateProgress>()
      progress.onmessage = onProgress
      return call('startup_install_update', { progress })
    },
    cancelUpdate: () => call('cancel_update'),
    revealHome: () => call('startup_reveal_home'),
    getFuselane: () => call('startup_get_fuselane'),
    copyDetails: () => call('startup_copy_details'),
    quit: () => call('startup_quit'),
  }
}

/** The problem to show instead of the app, or null when the app started normally. */
export async function startupProblem(): Promise<StartupBackend | null> {
  if ('__TAURI_INTERNALS__' in window) return tauriStartup()
  const params = new URLSearchParams(location.search)
  if (!params.has('startup')) return null
  const { createDemoStartup } = await import('./demoStartup')
  return createDemoStartup(params)
}
