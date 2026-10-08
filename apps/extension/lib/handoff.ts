// Handing one browser download to the app, written against a small interface so
// it can be tested without a browser (test/handoff.test.ts).
import { checkReply, decide, type DownloadLike, type Offer, type Rules } from '@fuselane/capture'

export interface BrowserDownloads {
  pause(id: number): Promise<void>
  resume(id: number): Promise<void>
  cancel(id: number): Promise<void>
  erase(id: number): Promise<void>
}

export interface Item extends DownloadLike {
  id: number
  state: string
  referrer?: string
}

/** Sends a message to the app's native host; rejects when it isn't installed. */
export type AskApp = (message: unknown) => Promise<unknown>

export type Outcome = 'kept' | 'handed-over' | 'returned'

/** The app gets 3 s to answer; after that the browser carries on. */
export const ANSWER_WITHIN_MS = 3000

function timeout(ms: number): Promise<never> {
  return new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), ms))
}

export function offerFor(item: Item, source: Offer['source']): Offer {
  const base = (item.filename ?? '').split(/[\\/]/).pop() || null
  return {
    v: 1,
    type: 'download.offer',
    url: item.url,
    finalUrl: item.finalUrl && item.finalUrl !== item.url ? item.finalUrl : null,
    referrer: item.referrer || null,
    filename: base,
    mime: item.mime || null,
    size: item.size && item.size > 0 ? item.size : null,
    cookies: null,
    headers: {},
    userAgent: null,
    source,
  }
}

/**
 * Pauses the browser's download, asks the app, then cancels it if the app took
 * it or resumes it if not. The browser never loses a download.
 */
export async function handOff(
  item: Item,
  rules: Rules,
  downloads: BrowserDownloads,
  ask: AskApp,
  waitMs = ANSWER_WITHIN_MS,
): Promise<Outcome> {
  if (item.state !== 'in_progress' || !decide(item, rules).capture) return 'kept'
  try {
    await downloads.pause(item.id)
  } catch {
    return 'kept' // already finished or not pausable: leave it alone
  }
  let reply: unknown
  try {
    reply = await Promise.race([ask(offerFor(item, 'auto')), timeout(waitMs)])
  } catch {
    reply = null
  }
  const r = checkReply(reply)
  if (r.ok && r.value.type === 'download.accepted') {
    await downloads.cancel(item.id).catch(() => {})
    await downloads.erase(item.id).catch(() => {})
    return 'handed-over'
  }
  await downloads.resume(item.id).catch(() => {})
  return 'returned'
}
