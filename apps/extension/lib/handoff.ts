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

/** The browser session for a link (cookies, User-Agent), when the person allowed it. */
export interface Session {
  cookies: string | null
  userAgent: string | null
}
export type GetSession = (url: string) => Promise<Session | null>

export type Outcome = 'kept' | 'handed-over' | 'returned'

/** The app gets 3 s to answer; after that the browser carries on. */
export const ANSWER_WITHIN_MS = 3000

function timeout(ms: number): Promise<never> {
  return new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), ms))
}

export function offerFor(
  item: Item,
  source: Offer['source'],
  session: Session | null = null,
): Offer {
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
    cookies: session?.cookies || null,
    headers: {},
    userAgent: session?.userAgent || null,
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
  getSession: GetSession = async () => null,
): Promise<Outcome> {
  if (item.state !== 'in_progress' || !decide(item, rules).capture) return 'kept'
  try {
    await downloads.pause(item.id)
  } catch {
    return 'kept' // already finished or not pausable: leave it alone
  }
  // Without the session the app can still take public files; never fail over it.
  const session = await getSession(item.finalUrl || item.url).catch(() => null)
  let reply: unknown
  try {
    reply = await Promise.race([ask(offerFor(item, 'auto', session)), timeout(waitMs)])
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

/**
 * Hands a link the person picked (context menu, the popup's list) to the app; if
 * the app can't take it, the browser downloads it instead. True when the app took it.
 */
export async function offerLink(
  url: string,
  referrer: string | undefined,
  ask: AskApp,
  getSession: GetSession,
  browserDownload: (url: string) => Promise<unknown>,
  waitMs = ANSWER_WITHIN_MS,
): Promise<boolean> {
  const session = await getSession(url).catch(() => null)
  const offer = offerFor({ id: -1, state: 'in_progress', url, referrer }, 'contextMenu', session)
  let accepted = false
  try {
    const reply = await Promise.race([ask(offer), timeout(waitMs)])
    const checked = checkReply(reply)
    accepted = checked.ok && checked.value.type === 'download.accepted'
  } catch {
    accepted = false
  }
  if (!accepted) await browserDownload(url)
  return accepted
}
