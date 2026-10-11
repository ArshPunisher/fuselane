// Who downloads what (REVIEW-2026-10-11 D, decided 11 Oct):
// - the Fuselane switch is off: the browser does everything;
// - the app is installed (running or not): every download goes to the app, which
//   the native host starts when it's closed;
// - no app: the extension downloads by itself up to the person's limit, if they
//   let it reach sites; bigger files get "Get the app" or "Download in browser".
import { decide, type DownloadLike, type Rules } from '@fuselane/capture'
import type { AppState } from './status.ts'

export type Route =
  | { to: 'browser'; reason: string }
  | { to: 'app' }
  | { to: 'self' }
  | { to: 'too-big'; size: number }

/** The extension's own downloads: 100 MB to 2 GB, 2 GB unless changed. */
export const MIN_LIMIT = 100 * 1024 ** 2
export const MAX_LIMIT = 2 * 1024 ** 3
export const DEFAULT_LIMIT = MAX_LIMIT

export function cleanLimit(v: unknown): number {
  return typeof v === 'number' && Number.isFinite(v)
    ? Math.min(MAX_LIMIT, Math.max(MIN_LIMIT, Math.round(v)))
    : DEFAULT_LIMIT
}

export interface Situation {
  rules: Rules
  app: AppState
  /** The person allowed Fuselane on all sites (needed to fetch files itself). */
  siteAccess: boolean
  /** The extension's own size limit, bytes. */
  limit: number
}

export function route(item: DownloadLike, s: Situation): Route {
  // Size no longer decides with the app: everything goes to it (minBytes 0).
  const d = decide(item, { ...s.rules, minBytes: 0 })
  if (!d.capture) return { to: 'browser', reason: d.reason }
  if (s.app !== 'not-installed') return { to: 'app' }
  if (!s.siteAccess) return { to: 'browser', reason: 'no site access' }
  const size = item.size && item.size > 0 ? item.size : null
  if (size !== null && size > s.limit) return { to: 'too-big', size }
  return { to: 'self' }
}
