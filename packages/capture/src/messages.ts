// Messages between the extension and the app, schema v1 (BROWSER-EXTENSION.md §3).
// Everything is validated on both sides; the app's copy lives in Rust and runs the
// same test vectors (vectors/offers.json).

export const LIMITS = {
  url: 8 * 1024,
  filename: 255,
  mime: 255,
  cookies: 16 * 1024,
  headers: 32,
  headerValue: 8 * 1024,
  userAgent: 1024,
} as const

export interface Offer {
  v: 1
  type: 'download.offer'
  url: string
  finalUrl: string | null
  referrer: string | null
  filename: string | null
  mime: string | null
  size: number | null
  cookies: string | null
  headers: Record<string, string>
  userAgent: string | null
  source: 'auto' | 'contextMenu'
}

export type Reply =
  | { v: 1; type: 'download.accepted'; jobId: string }
  | {
      v: 1
      type: 'download.declined'
      reason: 'ip_locked' | 'unsupported' | 'user_cancelled' | 'invalid'
      fallback: 'browser'
    }

export type Checked<T> = { ok: true; value: T } | { ok: false; error: string }

const fail = (error: string): Checked<never> => ({ ok: false, error })

function isObject(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null && !Array.isArray(x)
}

function link(x: unknown, what: string, schemes: string[]): Checked<string> {
  if (typeof x !== 'string' || x.length === 0) return fail(`${what} is missing`)
  if (x.length > LIMITS.url) return fail(`${what} is too long`)
  if (/[\s\0]/.test(x)) return fail(`${what} has spaces or control characters`)
  const scheme = x.slice(0, x.indexOf(':') + 1).toLowerCase()
  if (!schemes.includes(scheme)) return fail(`${what} must be ${schemes.join(' or ')}`)
  if (scheme !== 'magnet:') {
    try {
      new URL(x)
    } catch {
      return fail(`${what} isn't a valid link`)
    }
  }
  return { ok: true, value: x }
}

function optText(x: unknown, what: string, max: number): Checked<string | null> {
  if (x === undefined || x === null) return { ok: true, value: null }
  if (typeof x !== 'string') return fail(`${what} must be text`)
  if (x.length > max) return fail(`${what} is too long`)
  if (/[\r\n\0]/.test(x)) return fail(`${what} has line breaks`)
  return { ok: true, value: x }
}

const HEADER_NAME = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]{1,64}$/
/** Headers the app sets itself; an offer may not choose them. */
const RESERVED = new Set([
  'host',
  'range',
  'content-length',
  'connection',
  'transfer-encoding',
  'cookie',
])

/** Checks a message claiming to be a download offer. */
export function checkOffer(x: unknown): Checked<Offer> {
  if (!isObject(x)) return fail('not an object')
  if (x.v !== 1) return fail('unknown schema version')
  if (x.type !== 'download.offer') return fail('not a download offer')
  const url = link(x.url, 'url', ['http:', 'https:', 'magnet:'])
  if (!url.ok) return url
  const finalUrl =
    x.finalUrl == null
      ? { ok: true as const, value: null }
      : link(x.finalUrl, 'finalUrl', ['http:', 'https:'])
  if (!finalUrl.ok) return finalUrl
  const referrer =
    x.referrer == null
      ? { ok: true as const, value: null }
      : link(x.referrer, 'referrer', ['http:', 'https:'])
  if (!referrer.ok) return referrer
  const filename = optText(x.filename, 'filename', LIMITS.filename)
  if (!filename.ok) return filename
  if (filename.value && /[\\/]/.test(filename.value))
    return fail('filename may not contain folders')
  const mime = optText(x.mime, 'mime', LIMITS.mime)
  if (!mime.ok) return mime
  const cookies = optText(x.cookies, 'cookies', LIMITS.cookies)
  if (!cookies.ok) return cookies
  const userAgent = optText(x.userAgent, 'userAgent', LIMITS.userAgent)
  if (!userAgent.ok) return userAgent
  let size: number | null = null
  if (x.size !== undefined && x.size !== null) {
    if (typeof x.size !== 'number' || !Number.isSafeInteger(x.size) || x.size < 0)
      return fail('size must be a whole number of bytes')
    size = x.size
  }
  const headers: Record<string, string> = {}
  if (x.headers !== undefined && x.headers !== null) {
    if (!isObject(x.headers)) return fail('headers must be an object')
    const entries = Object.entries(x.headers)
    if (entries.length > LIMITS.headers) return fail('too many headers')
    for (const [k, v] of entries) {
      if (!HEADER_NAME.test(k))
        return fail(`header name ${JSON.stringify(k.slice(0, 40))} isn't allowed`)
      if (RESERVED.has(k.toLowerCase())) return fail(`header ${k} is set by the app`)
      const val = optText(v, `header ${k}`, LIMITS.headerValue)
      if (!val.ok) return val
      if (val.value !== null) headers[k] = val.value
    }
  }
  const source = x.source ?? 'auto'
  if (source !== 'auto' && source !== 'contextMenu') return fail('unknown source')
  return {
    ok: true,
    value: {
      v: 1,
      type: 'download.offer',
      url: url.value,
      finalUrl: finalUrl.value,
      referrer: referrer.value,
      filename: filename.value,
      mime: mime.value,
      size,
      cookies: cookies.value,
      headers,
      userAgent: userAgent.value,
      source,
    },
  }
}

/** Checks the app's answer; anything unexpected means "let the browser have it". */
export function checkReply(x: unknown): Checked<Reply> {
  if (!isObject(x) || x.v !== 1) return fail('unknown reply')
  if (
    x.type === 'download.accepted' &&
    typeof x.jobId === 'string' &&
    x.jobId.length > 0 &&
    x.jobId.length <= 64
  )
    return { ok: true, value: { v: 1, type: 'download.accepted', jobId: x.jobId } }
  const reasons = ['ip_locked', 'unsupported', 'user_cancelled', 'invalid'] as const
  if (x.type === 'download.declined' && reasons.includes(x.reason as (typeof reasons)[number]))
    return {
      ok: true,
      value: {
        v: 1,
        type: 'download.declined',
        reason: x.reason as (typeof reasons)[number],
        fallback: 'browser',
      },
    }
  return fail('unknown reply')
}
