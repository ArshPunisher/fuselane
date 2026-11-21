// The extension's own downloader, for computers without the Fuselane app
// (REVIEW-2026-10-11 D): one network, several parts at once when the server
// allows ranges, pause and resume. Written against `fetch` and a positioned
// writer so it runs in tests without a browser.

export type FetchFn = (url: string, init?: RequestInit) => Promise<Response>

export interface Probe {
  /** Bytes, or null when the server doesn't say. */
  total: number | null
  /** The server answers byte ranges, so parts and resume work. */
  ranges: boolean
  /** From Content-Disposition, if the server names the file. */
  filename: string | null
  /** Where redirects ended (the parts are fetched from there). */
  finalUrl: string
  /**
   * What names this version of the file for `If-Range`: the strong ETag, else
   * Last-Modified (a weak ETag can't be used there), else null.
   */
  validator: string | null
}

/** One part: bytes [start, end] inclusive, `done` of them saved from start. */
export interface Part {
  start: number
  end: number
  done: number
}

/** A file name from a Content-Disposition header, RFC 6266 `filename*` first. */
export function dispositionName(header: string | null): string | null {
  if (!header) return null
  const star = /filename\*\s*=\s*(?:UTF-8|utf-8)?''([^;]+)/.exec(header)
  if (star?.[1]) {
    try {
      return clean(decodeURIComponent(star[1].trim()))
    } catch {
      // fall through to the plain form
    }
  }
  const plain = /filename\s*=\s*"?([^";]+)"?/.exec(header)
  return plain?.[1] ? clean(plain[1].trim()) : null
}

/** A name that's safe as a file name: no folders, no control characters. */
export function clean(name: string): string | null {
  const base =
    name
      .split(/[\\/]/)
      .pop()
      ?.replace(/[\x00-\x1f\x7f<>:"|?*]/g, '_')
      .trim() ?? ''
  return base && base !== '.' && base !== '..' ? base.slice(0, 200) : null
}

/** The name to save as: the server's, the browser's guess, then the link's. */
export function nameFor(url: string, server: string | null, browserGuess?: string): string {
  if (server) return server
  const guess = browserGuess ? clean(browserGuess) : null
  if (guess) return guess
  try {
    const last = new URL(url).pathname.split('/').pop() ?? ''
    return clean(decodeURIComponent(last)) ?? 'download'
  } catch {
    return 'download'
  }
}

/** The part of a `Content-Range: bytes 10-99/1000` answer that is checked. */
export function parseContentRange(
  header: string | null,
): { start: number; end: number; total: number | null } | null {
  const m = /^\s*bytes\s+(\d+)-(\d+)\/(\d+|\*)\s*$/i.exec(header ?? '')
  if (!m) return null
  return { start: Number(m[1]), end: Number(m[2]), total: m[3] === '*' ? null : Number(m[3]) }
}

/** The file on the server isn't the one the earlier bytes came from (or the answer isn't the range asked for). */
export class FileChanged extends Error {
  constructor(message = 'The file on the server changed while it was downloading.') {
    super(message)
  }
}

/**
 * What pins the file's version: its strong ETag, or else its Last-Modified as
 * `date:<value>`. Only an ETag is sent as If-Range: RFC 9110 13.1.5 forbids an
 * HTTP-date there unless the date is strong, which a client can't know, and a
 * server treats a weak one as "changed". A date is compared with each answer.
 */
function validatorOf(h: Headers): string | null {
  const etag = h.get('etag')
  if (etag && !/^w\//i.test(etag.trim())) return etag.trim()
  const date = h.get('last-modified')?.trim()
  return date ? `date:${date}` : null
}

/** A strong ETag is quoted; anything else is a date (older jobs saved it bare). */
const isEtag = (v: string | null | undefined): boolean => !!v && v.startsWith('"')
const dateOf = (v: string | null | undefined): string | null =>
  !v || isEtag(v) ? null : v.startsWith('date:') ? v : `date:${v}`

/** Asks for the first byte: a 206 means ranges work and tells the size. */
export async function probe(url: string, fetchFn: FetchFn, signal?: AbortSignal): Promise<Probe> {
  const res = await fetchFn(url, {
    headers: { Range: 'bytes=0-0' },
    signal,
    credentials: 'include',
  })
  if (!res.ok) throw new Error(`The server answered ${res.status}.`)
  const filename = dispositionName(res.headers.get('content-disposition'))
  const range = /\/(\d+)\s*$/.exec(res.headers.get('content-range') ?? '')
  // Don't download the body of a server that ignored the range.
  await res.body?.cancel().catch(() => {})
  if (res.status === 206 && range?.[1]) {
    return {
      total: Number(range[1]),
      ranges: true,
      filename,
      finalUrl: res.url || url,
      validator: validatorOf(res.headers),
    }
  }
  const len = Number(res.headers.get('content-length'))
  return {
    total: Number.isFinite(len) && len > 0 ? len : null,
    ranges: false,
    filename,
    finalUrl: res.url || url,
    validator: validatorOf(res.headers),
  }
}

/** Parts no smaller than this, so small files take one request. */
const MIN_PART = 2 * 1024 * 1024

/** Splits `total` bytes into at most `count` parts. */
export function split(total: number, count: number): Part[] {
  const n = Math.max(1, Math.min(count, Math.ceil(total / MIN_PART)))
  const size = Math.ceil(total / n)
  const parts: Part[] = []
  for (let start = 0; start < total; start += size) {
    parts.push({ start, end: Math.min(total, start + size) - 1, done: 0 })
  }
  return parts
}

export interface RunOptions {
  url: string
  /** Null: one stream from the start (no ranges). */
  parts: Part[] | null
  /** The size the probe found: every 206 must come from a file of this size. */
  total?: number | null
  /** The probe's version of the file, sent as `If-Range` with every part. */
  validator?: string | null
  fetchFn: FetchFn
  /** Saves bytes at a position; awaited, so slow disks slow the network. */
  write: (position: number, data: Uint8Array) => Promise<void>
  /** Called with the parts as they advance (for progress and resume). */
  onProgress?: (parts: Part[] | null, written: number) => void
  signal: AbortSignal
  /** Stop with an error once more than this has come in (unknown sizes). */
  limit?: number
  /** Tries per part before giving up. */
  tries?: number
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const t = setTimeout(resolve, ms)
    signal.addEventListener('abort', () => {
      clearTimeout(t)
      reject(signal.reason ?? new Error('stopped'))
    })
  })
}

export class TooBig extends Error {}

/**
 * Downloads until done, aborted or failed. With parts, each part is fetched with
 * its own range and retried on its own; without, one stream from byte 0.
 */
export async function run(o: RunOptions): Promise<number> {
  const tries = o.tries ?? 4
  if (!o.parts) {
    let written = 0
    const res = await o.fetchFn(o.url, { signal: o.signal, credentials: 'include' })
    if (!res.ok || !res.body) throw new Error(`The server answered ${res.status}.`)
    const reader = res.body.getReader()
    for (;;) {
      const { done, value } = await reader.read()
      if (done) break
      await o.write(written, value)
      written += value.length
      if (o.limit !== undefined && written > o.limit) {
        await reader.cancel().catch(() => {})
        throw new TooBig()
      }
      o.onProgress?.(null, written)
    }
    return written
  }
  const parts = o.parts
  const sum = () => parts.reduce((n, p) => n + p.done, 0)
  await Promise.all(
    parts.map(async (p) => {
      for (let attempt = 1; p.start + p.done <= p.end; attempt++) {
        try {
          const from = p.start + p.done
          const res = await o.fetchFn(o.url, {
            headers: {
              Range: `bytes=${from}-${p.end}`,
              ...(o.validator && isEtag(o.validator) ? { 'If-Range': o.validator } : {}),
            },
            signal: o.signal,
            credentials: 'include',
          })
          // With If-Range, a 200 is the server saying the file is no longer the one
          // we started with.
          if (res.status === 200 && isEtag(o.validator)) {
            await res.body?.cancel().catch(() => {})
            throw new FileChanged()
          }
          // 200 here would be the whole file from byte 0: refuse rather than corrupt.
          if (res.status !== 206 || !res.body) throw new Error(`The server answered ${res.status}.`)
          // The bytes must be the ones asked for, from a file of the size we know.
          const got = parseContentRange(res.headers.get('content-range'))
          // A date pins the version by comparison: a different date is another file.
          const date = res.headers.get('last-modified')?.trim()
          const pinned = dateOf(o.validator)
          const dateMoved = !!pinned && !!date && `date:${date}` !== pinned
          if (dateMoved) {
            await res.body.cancel().catch(() => {})
            throw new FileChanged()
          }
          // An unknown total ("bytes a-b/*") is fine once an ETag pins the file.
          if (
            !got ||
            got.start !== from ||
            got.end < from ||
            (o.total != null && got.total !== null && got.total !== o.total) ||
            (o.total != null && got.total === null && !isEtag(o.validator))
          ) {
            await res.body.cancel().catch(() => {})
            throw new FileChanged(
              'The server sent a different part of the file than was asked for.',
            )
          }
          const reader = res.body.getReader()
          const before = p.done
          for (;;) {
            const { done, value } = await reader.read()
            if (done) break
            const room = p.end - (p.start + p.done) + 1
            const chunk = value.length > room ? value.subarray(0, room) : value
            await o.write(p.start + p.done, chunk)
            p.done += chunk.length
            o.onProgress?.(parts, sum())
            if (chunk.length < value.length) {
              await reader.cancel().catch(() => {})
              break
            }
          }
          // Ended early with nothing new: a failed try. With progress: a fresh count.
          if (p.done === before && p.start + p.done <= p.end) {
            throw new Error('The server stopped sending.')
          }
          attempt = 0
        } catch (e) {
          if (o.signal.aborted) throw o.signal.reason ?? e
          // A different file is no use retrying: the caller starts again from zero.
          if (e instanceof FileChanged) throw e
          if (attempt >= tries) throw e
          await sleep(500 * 2 ** (attempt - 1), o.signal)
        }
      }
    }),
  )
  return sum()
}
