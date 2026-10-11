// One line about the app, shared by the popup and the settings page. The ping
// also tells the app which browser this is (its Browser extension page shows it)
// and brings back where downloads should go, which the app decides.
import type { Rules } from '@fuselane/capture'
import type { AskApp } from './handoff.ts'

export type AppState = 'connected' | 'not-running' | 'not-installed'

/** Where downloads go, as the app says: off, or everything from minBytes up. */
export interface Handoff {
  enabled: boolean
  minBytes: number
}

export interface Status {
  state: AppState
  text: string
  version?: string
  handoff?: Handoff
}

/** Which browser this is, in the words the app uses. Brave hides behind Chrome's name. */
export function browserId(ua: string, brave = false): string {
  if (/Firefox\//.test(ua)) return 'firefox'
  if (/Edg(e|A|iOS)?\//.test(ua)) return 'edge'
  if (brave) return 'brave'
  if (/Vivaldi\//.test(ua)) return 'vivaldi'
  if (/Chromium\//.test(ua)) return 'chromium'
  return 'chrome'
}

/** A hand-off setting from the app, if it is one (the app may be older). */
export function readHandoff(v: unknown): Handoff | undefined {
  if (!v || typeof v !== 'object') return undefined
  const h = v as Record<string, unknown>
  const min = h.minBytes
  return typeof h.enabled === 'boolean' &&
    typeof min === 'number' &&
    Number.isInteger(min) &&
    min >= 0 &&
    min <= 2 ** 40
    ? { enabled: h.enabled, minBytes: min }
    : undefined
}

/** The rules with the app's say on where downloads go; sites and types stay ours. */
export function withHandoff(rules: Rules, h: Handoff | undefined): Rules {
  return h ? { ...rules, enabled: h.enabled, minBytes: h.minBytes } : rules
}

export async function appStatus(
  ask: AskApp,
  hello: { browser: string; version: string } = { browser: '', version: '' },
): Promise<Status> {
  try {
    const pong = (await ask({ type: 'ping', ...hello })) as
      { app?: { version?: unknown; handoff?: unknown } } | undefined
    const version = pong?.app?.version
    return typeof version === 'string'
      ? {
          state: 'connected',
          text: `Connected to Fuselane ${version}.`,
          version,
          handoff: readHandoff(pong?.app?.handoff),
        }
      : {
          state: 'not-running',
          text: 'Fuselane is installed but not running. Open it to hand downloads over.',
        }
  } catch {
    return {
      state: 'not-installed',
      text: 'Fuselane isn’t installed on this computer, so downloads stay in the browser.',
    }
  }
}

/**
 * Joins the app's setting with ours after a ping. A change made while the app was
 * closed is `pending`: it goes to the app first, so the app's older value can't
 * undo it. Returns the rules to keep and whether a change is still waiting.
 */
export async function settle(
  s: Status,
  rules: Rules,
  pending: boolean,
  ask: AskApp,
): Promise<{ rules: Rules; pending: boolean }> {
  if (s.state !== 'connected') return { rules, pending }
  if (pending) {
    const reply = (await ask({
      v: 1,
      type: 'handoff.set',
      enabled: rules.enabled,
      minBytes: rules.minBytes,
    }).catch(() => null)) as { handoff?: unknown } | null
    const now = readHandoff(reply?.handoff)
    return now ? { rules: withHandoff(rules, now), pending: false } : { rules, pending: true }
  }
  return { rules: withHandoff(rules, s.handoff), pending: false }
}

/**
 * Whether our switch and size must go to the app before we follow it: a change
 * made while it was closed, or an extension upgraded from before the app kept
 * this setting (it has rules but has never synced), so the person's earlier
 * choice isn't replaced by the app's default.
 */
export function mustPush(stored: Record<string, unknown>): boolean {
  return (
    stored.handoffPending === true || (stored.rules !== undefined && stored.handoffSynced !== true)
  )
}

/** Sends a change of the switch or size to the app; true when the app has it. */
export async function sendHandoff(ask: AskApp, rules: Rules): Promise<boolean> {
  const reply = (await ask({
    v: 1,
    type: 'handoff.set',
    enabled: rules.enabled,
    minBytes: rules.minBytes,
  }).catch(() => null)) as { handoff?: unknown } | null
  return readHandoff(reply?.handoff) !== undefined
}
