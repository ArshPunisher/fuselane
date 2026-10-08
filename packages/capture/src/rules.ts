// When the extension takes a browser download over (BROWSER-EXTENSION.md §1).
// The browser keeps anything Fuselane can't fetch on its own.

export interface Rules {
  enabled: boolean
  /** Smaller downloads stay in the browser (default 1 MiB). Unknown sizes are taken. */
  minBytes: number
  /** File extensions to take, without the dot ("iso", "zip"); empty means any. */
  extensions: string[]
  /** Only these sites (and their subdomains), when not empty. */
  includeDomains: string[]
  /** Never these sites (and their subdomains). */
  excludeDomains: string[]
  /** MIME types to take ("application/zip", "video/*"); empty means any. */
  mimeTypes: string[]
}

export const DEFAULT_RULES: Rules = {
  enabled: true,
  minBytes: 1024 * 1024,
  extensions: [],
  includeDomains: [],
  excludeDomains: [],
  mimeTypes: [],
}

/** What the browser tells the extension about a download that just started. */
export interface DownloadLike {
  url: string
  finalUrl?: string
  filename?: string
  mime?: string
  /** Bytes, or -1 / 0 / undefined when the browser doesn't know. */
  size?: number
}

export type Decision = { capture: true } | { capture: false; reason: string }

const keep = (reason: string): Decision => ({ capture: false, reason })

function host(url: string): string | null {
  try {
    const u = new URL(url)
    return u.protocol === 'http:' || u.protocol === 'https:' ? u.hostname.toLowerCase() : null
  } catch {
    return null
  }
}

/** True when `h` is `domain` or one of its subdomains (never a look-alike suffix). */
export function onDomain(h: string, domain: string): boolean {
  const d = domain.trim().toLowerCase().replace(/^\.+/, '')
  return d !== '' && (h === d || h.endsWith(`.${d}`))
}

function extension(name: string): string {
  const base = name.split(/[\\/]/).pop() ?? ''
  const dot = base.lastIndexOf('.')
  return dot > 0 ? base.slice(dot + 1).toLowerCase() : ''
}

function mimeMatches(mime: string, patterns: string[]): boolean {
  const m = mime.split(';')[0]?.trim().toLowerCase() ?? ''
  return patterns.some((p) => {
    const q = p.trim().toLowerCase()
    return q.endsWith('/*') ? m.startsWith(q.slice(0, -1)) : m === q
  })
}

/** Should Fuselane take this download? `altKey`: the user held Alt to skip capture. */
export function decide(item: DownloadLike, rules: Rules, altKey = false): Decision {
  if (!rules.enabled) return keep('capture is off')
  if (altKey) return keep('Alt was held')
  const url = item.finalUrl || item.url
  const h = host(url)
  // blob:, data:, file: and anything else the app can't fetch by itself.
  if (h === null) return keep('not a web link')
  if (rules.excludeDomains.some((d) => onDomain(h, d))) return keep('site excluded')
  if (rules.includeDomains.length > 0 && !rules.includeDomains.some((d) => onDomain(h, d)))
    return keep('site not included')
  const size = item.size ?? -1
  if (size > 0 && size < rules.minBytes) return keep('smaller than the minimum')
  if (rules.extensions.length > 0) {
    const ext = extension(item.filename || new URL(url).pathname)
    const wanted = rules.extensions.map((e) => e.trim().toLowerCase().replace(/^\./, ''))
    if (!wanted.includes(ext)) return keep('file type not chosen')
  }
  if (rules.mimeTypes.length > 0 && !mimeMatches(item.mime ?? '', rules.mimeTypes))
    return keep('MIME type not chosen')
  return { capture: true }
}
