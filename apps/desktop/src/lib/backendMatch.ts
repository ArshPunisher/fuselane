// Translating text the Rust core sends (errors, hints, notes) without changing the core:
// it keeps writing English, which the CLI and logs use too. A table maps each English
// sentence to its translation; a sentence with `{names}` is a template, matched whole
// against the text, and the parts it captures go into the translation.
// How to add one: docs/07-design/I18N.md ("Text from the core").
// No imports here: scripts/i18n-check.ts loads this file straight into Node.

export type BackendTable = Readonly<Record<string, string>>

interface Pattern {
  re: RegExp
  /** The English template, for the checks. */
  en: string
  to: string
}

export interface Compiled {
  exact: Map<string, string>
  /** Most literal text first, so `Starts at {when} tomorrow.` wins over `Starts at {when}.` */
  patterns: Pattern[]
  cache: Map<string, string>
}

const NAME = /\{(\w+)\}/g

/**
 * Parts that are themselves sentences from the core ("Every network failed. Last
 * problem: {last}"), so they are translated too. Other parts (a file name, a network,
 * an address, a number) are kept exactly as sent.
 */
const NESTED = new Set(['why', 'e', 'last', 'reason', 'problem', 'next', 'what', 'todo', 'm'])

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/** Builds the lookup once per table. */
export function compileBackend(table: BackendTable): Compiled {
  const exact = new Map<string, string>()
  const patterns: (Pattern & { weight: number })[] = []
  for (const [en, to] of Object.entries(table)) {
    if (!en.match(NAME)) {
      exact.set(en, to)
      continue
    }
    const seen = new Set<string>()
    let source = '^'
    let last = 0
    for (const m of en.matchAll(NAME)) {
      const name = m[1] ?? ''
      source += escape(en.slice(last, m.index))
      source += seen.has(name) ? `\\k<${name}>` : `(?<${name}>.+?)`
      seen.add(name)
      last = m.index + m[0].length
    }
    source += `${escape(en.slice(last))}$`
    patterns.push({ re: new RegExp(source, 's'), en, to, weight: en.replace(NAME, '').length })
  }
  patterns.sort((a, b) => b.weight - a.weight)
  return { exact, patterns, cache: new Map() }
}

/** One whole sentence (or run of sentences) from the table, or null. */
function whole(s: string, c: Compiled, depth: number): string | null {
  const hit = c.exact.get(s)
  if (hit !== undefined) return hit
  for (const p of c.patterns) {
    const m = p.re.exec(s)
    if (!m) continue
    const parts = m.groups ?? {}
    const filled = p.to.replace(NAME, (all, name: string) => {
      const v = parts[name]
      if (v === undefined) return all
      return NESTED.has(name) && depth < 3 ? part(v, c, depth + 1) : v
    })
    // A part that ends its own sentence, then the template's full stop: keep one.
    return filled.replace(/([।.?!…])।/g, '$1')
  }
  return null
}

/**
 * A part inside another sentence. The core often drops a reason's full stop to fit it
 * in ("Not added: {why}."), so a part is also tried with one (and the translation's
 * full stop left off), and a lower-case reason that ends a sentence without one.
 */
function part(v: string, c: Compiled, depth: number): string {
  const direct = whole(v, c, depth)
  if (direct !== null) return direct
  if (!/[.?!…]$/.test(v)) {
    const stopped = whole(`${v}.`, c, depth)
    if (stopped !== null) return stopped.replace(/[।.]$/, '')
  } else if (v.endsWith('.')) {
    const bare = whole(v.slice(0, -1), c, depth)
    if (bare !== null) return `${bare}।`
  }
  return bySentence(v, c, depth) ?? v
}

/** Where a sentence may end: after . ? ! or : and a space. */
function boundaries(s: string): number[] {
  const out: number[] = []
  for (const m of s.matchAll(/[.?!:।] +/g)) out.push(m.index + m[0].length)
  return out
}

/**
 * Text made of several sentences joined together (a message and its hint, "{what}
 * {todo}"): the longest run of sentences the table knows at each point. Sentences it
 * doesn't know stay in English. Null when nothing at all was known.
 */
function bySentence(s: string, c: Compiled, depth: number): string | null {
  const ends = [...boundaries(s), s.length]
  if (ends.length < 2) return null
  const out: string[] = []
  let known = false
  let i = 0
  while (i < s.length) {
    let next = -1
    for (let k = ends.length - 1; k >= 0; k--) {
      const j = ends[k] ?? s.length
      if (j <= i) break
      const piece = s.slice(i, j).trim()
      const hit = piece ? whole(piece, c, depth) : null
      if (hit !== null) {
        out.push(hit)
        known = true
        next = j
        break
      }
    }
    if (next < 0) {
      next = ends.find((j) => j > i) ?? s.length
      const piece = s.slice(i, next).trim()
      if (piece) out.push(piece)
    }
    i = next
  }
  return known ? out.join(' ') : null
}

/** The translation of `text`, or `text` itself when the table doesn't know it. */
export function translateBackend(text: string, c: Compiled): string {
  const s = text.trim()
  if (!s) return text
  const cached = c.cache.get(s)
  if (cached !== undefined) return cached
  const out = whole(s, c, 0) ?? bySentence(s, c, 0) ?? s
  if (c.cache.size > 500) c.cache.clear()
  c.cache.set(s, out)
  return out
}

/** Which template (or exact entry) a text matched, for the checks. */
export function matchedTemplate(text: string, c: Compiled): string | null {
  if (c.exact.has(text)) return text
  return c.patterns.find((p) => p.re.test(text))?.en ?? null
}
