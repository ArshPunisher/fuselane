// The startup-problem screen in a plain browser (Playwright, pnpm dev):
//   ?startup=newer   the list is from a newer Fuselane
//   ?startup=other   any other problem (a folder Fuselane can't write to)
// with the update simulated by ?update=1 (found), bad (fails while downloading),
// offline (the check fails) or nothing (no update found). What a button did is
// written to <html data-demo-action> so tests can see it.
import type { UiError, UpdateProgress } from './types'
import type { StartupBackend, StartupProblem } from './startup'

const MB = 1024 * 1024

function err(code: string, message: string, hint: string | null): UiError {
  return { code, message, hint }
}

function did(action: string) {
  document.documentElement.dataset.demoAction = action
}

export function createDemoStartup(params: URLSearchParams): StartupBackend {
  const update = params.get('update')
  const home = '/Users/demo/Library/Application Support/app.fuselane'
  const problem: StartupProblem =
    params.get('startup') === 'newer'
      ? {
          kind: 'newer',
          message: 'This download list was made by a newer Fuselane.',
          hint: 'Update Fuselane to keep going. Your downloads are kept.',
          detail:
            "couldn't open the download list: the database was made by a newer Fuselane (schema v15, this build knows v13)",
          version: '0.1.0-beta.10',
          home,
        }
      : {
          kind: 'other',
          message: "Fuselane isn't allowed to change its folder.",
          hint: 'Check that your account can write to the folder, then open Fuselane again.',
          detail: `couldn't create ${home}: Permission denied (os error 13)`,
          version: '0.1.0-beta.10',
          home,
        }
  let cancelled = false
  return {
    demo: true,
    problem,
    checkUpdate: async () => {
      await new Promise((r) => setTimeout(r, 300))
      if (update === 'offline')
        throw err('update-check', "Couldn't check for updates: you're offline.", null)
      return update === '1' || update === 'bad'
        ? { version: '0.1.0-beta.11', notes: null, size: 38.6 * MB }
        : null
    },
    installUpdate: async (onProgress: (p: UpdateProgress) => void) => {
      const total = 38.6 * MB
      let done = 0
      cancelled = false
      while (done < total) {
        await new Promise((r) => setTimeout(r, 60))
        if (cancelled) throw err('update-cancelled', 'Update cancelled.', null)
        if (update === 'bad' && done > total * 0.57)
          throw err(
            'update-failed',
            "Couldn't download the update: every network dropped at 22.0 MB.",
            'Try again. It picks up the check from the start, and your downloads are safe.',
          )
        done = Math.min(total, done + 2.2 * MB)
        onProgress({ phase: 'downloading', done, total, rate: 6.1 * MB, networks: 3 })
      }
      onProgress({ phase: 'installing', done: total, total, rate: 0, networks: 0 })
      // The real app restarts here; the demo stays on "installing".
      await new Promise(() => {})
    },
    cancelUpdate: async () => {
      cancelled = true
    },
    revealHome: async () => did('reveal'),
    getFuselane: async () => did('get-fuselane'),
    copyDetails: async () => did('copy'),
    quit: async () => did('quit'),
  }
}
