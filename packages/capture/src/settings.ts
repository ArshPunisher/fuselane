// What the settings page reads and writes (BROWSER-EXTENSION.md §1): turns what a
// person types into Rules, and stored values (possibly from an older version, or
// edited by hand) back into Rules that `decide` can trust.

import { DEFAULT_RULES, type Rules } from './rules.ts'

const UNITS = { b: 1, kb: 1024, mb: 1024 ** 2, gb: 1024 ** 3 } as const
/** Anything above this is surely a typo: nothing would ever be handed over. */
export const MAX_MIN_BYTES = 1024 ** 4

export type Parsed<T> = { ok: true; value: T } | { ok: false; error: string }

/** "1.5 MB", "500kb", "2 GB", "0". A bare number means megabytes. */
export function parseSize(text: string): Parsed<number> {
  const m = /^\s*(\d+(?:\.\d+)?)\s*([kmg]?b)?\s*$/i.exec(text)
  if (!m) return { ok: false, error: 'Enter a size like 1 MB, 500 KB or 2 GB.' }
  const unit = (m[2]?.toLowerCase() ?? 'mb') as keyof typeof UNITS
  const bytes = Math.round(Number(m[1]) * UNITS[unit])
  if (bytes > MAX_MIN_BYTES) return { ok: false, error: 'Keep the minimum at 1024 GB or less.' }
  return { ok: true, value: bytes }
}

/** The shortest exact-enough form: 1048576 → "1 MB", 1572864 → "1.5 MB". */
export function formatSize(bytes: number): string {
  for (const [unit, n] of [
    ['GB', UNITS.gb],
    ['MB', UNITS.mb],
    ['KB', UNITS.kb],
  ] as const) {
    if (bytes >= n) return `${Number((bytes / n).toFixed(2))} ${unit}`
  }
  return `${bytes} B`
}

/** One list entry per line, comma or space. */
export interface ListResult {
  values: string[]
  /** Entries that aren't valid, as typed. */
  invalid: string[]
}

function parseList(text: string, clean: (entry: string) => string | null): ListResult {
  const values: string[] = []
  const invalid: string[] = []
  for (const entry of text.split(/[\s,]+/).filter(Boolean)) {
    const v = clean(entry)
    if (v === null) invalid.push(entry)
    else if (!values.includes(v)) values.push(v)
  }
  return { values, invalid }
}

const LABEL = /^(?!-)[a-z0-9-]{1,63}(?<!-)$/

/** A bare host name; pasted links and "*." prefixes are reduced to the host. */
export function cleanDomain(entry: string): string | null {
  let d = entry.trim().toLowerCase()
  if (/^[a-z][a-z0-9+.-]*:\/\//.test(d)) {
    try {
      d = new URL(d).hostname
    } catch {
      return null
    }
  }
  d = d.replace(/^\*?\.+/, '').replace(/\.$/, '')
  // Two labels at least: "com" alone would match half the web.
  const labels = d.split('.')
  if (d.length > 253 || labels.length < 2 || !labels.every((l) => LABEL.test(l))) return null
  return d
}

/** "iso", ".ISO" and "*.iso" all mean iso; "tar.gz" means gz, the part that is matched. */
export function cleanExtension(entry: string): string | null {
  const e =
    entry
      .trim()
      .toLowerCase()
      .replace(/^\*?\./, '')
      .split('.')
      .pop() ?? ''
  return /^[a-z0-9]{1,16}$/.test(e) ? e : null
}

/** "application/zip" or a whole family like "video/*". */
export function cleanMimeType(entry: string): string | null {
  const t = entry.trim().toLowerCase()
  return /^[a-z]+\/(\*|[a-z0-9][a-z0-9.+-]*)$/.test(t) ? t : null
}

export const parseDomains = (text: string) => parseList(text, cleanDomain)
export const parseExtensions = (text: string) => parseList(text, cleanExtension)
export const parseMimeTypes = (text: string) => parseList(text, cleanMimeType)

function list(value: unknown, clean: (entry: string) => string | null): string[] {
  if (!Array.isArray(value)) return []
  return parseList(value.filter((v) => typeof v === 'string').join(' '), clean).values
}

/** Stored rules → safe Rules. Anything missing or broken falls back to the default. */
export function normalizeRules(stored: unknown): Rules {
  const s = (typeof stored === 'object' && stored !== null ? stored : {}) as Record<string, unknown>
  const min = s.minBytes
  return {
    enabled: typeof s.enabled === 'boolean' ? s.enabled : DEFAULT_RULES.enabled,
    minBytes:
      typeof min === 'number' && Number.isFinite(min) && min >= 0 && min <= MAX_MIN_BYTES
        ? Math.round(min)
        : DEFAULT_RULES.minBytes,
    extensions: list(s.extensions, cleanExtension),
    includeDomains: list(s.includeDomains, cleanDomain),
    excludeDomains: list(s.excludeDomains, cleanDomain),
    mimeTypes: list(s.mimeTypes, cleanMimeType),
  }
}
