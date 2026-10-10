// Translations (STEPS 8.8). The English sentence is the key: a string with no
// translation shows in English, and tests that look for English text keep working.
// How to add a string or a language: docs/07-design/I18N.md.
import { cloneElement, isValidElement, type ReactNode } from 'react'
import { create } from 'zustand'
import { hi } from '../locales/hi'
import { hiBackend } from '../locales/hi-backend'
import { compileBackend, translateBackend, type BackendTable, type Compiled } from './backendMatch'

export type Locale = 'en' | 'hi'
/** What the person picked in Settings; 'system' follows the computer's language. */
export type LangPref = 'system' | Locale
export type Vars = Record<string, string | number>

const CATALOGUES: Record<Locale, Readonly<Record<string, string>> | null> = { en: null, hi }
/** Text the core sends, by language (lib/backendMatch.ts). */
const BACKEND: Record<Locale, BackendTable | null> = { en: null, hi: hiBackend }
const compiled = new Map<Locale, Compiled>()
const KEY = 'fuselane.lang'

/** The desktop app; anything else is the demo in a plain browser (pnpm dev, Playwright). */
const inApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

function languages(): readonly string[] {
  if (typeof navigator === 'undefined') return []
  return navigator.languages?.length ? navigator.languages : [navigator.language]
}

/** The first of the computer's languages that Fuselane speaks; English otherwise. */
export function systemLocale(langs: readonly string[] = languages()): Locale {
  for (const l of langs) {
    const base = l.toLowerCase().split('-')[0]
    if (base === 'hi' || base === 'en') return base
  }
  return 'en'
}

/**
 * The demo always starts in English, whatever the machine's language, so tests never
 * depend on it; `?lang=hi` (or a choice made in Settings) switches it.
 */
function resolve(pref: LangPref): Locale {
  if (pref !== 'system') return pref
  return inApp ? systemLocale() : 'en'
}

function urlPref(): Locale | null {
  if (inApp || typeof location === 'undefined') return null
  const v = new URLSearchParams(location.search).get('lang')
  return v === 'hi' || v === 'en' ? v : null
}

function savedPref(): LangPref {
  try {
    const v = localStorage.getItem(KEY)
    return v === 'hi' || v === 'en' ? v : 'system'
  } catch {
    return 'system'
  }
}

// Read synchronously, so the first paint is already in the right language.
const startPref: LangPref = urlPref() ?? savedPref()
let current: Locale = resolve(startPref)

interface LangState {
  pref: LangPref
  locale: Locale
}

/** The language preference and the language in use; components re-render when it changes. */
export const useLang = create<LangState>(() => ({ pref: startPref, locale: current }))

function applyLang(l: Locale) {
  if (typeof document !== 'undefined') document.documentElement.lang = l
}
applyLang(current)

function use(l: Locale) {
  current = l
  applyLang(l)
  useLang.setState({ locale: l })
}

/** Saves the choice (a per-window UI preference, so local storage is enough) and applies it now. */
export function setLangPref(pref: LangPref) {
  try {
    if (pref === 'system') localStorage.removeItem(KEY)
    else localStorage.setItem(KEY, pref)
  } catch {
    // Private mode: the choice lasts until the window closes.
  }
  useLang.setState({ pref })
  use(resolve(pref))
}

// 'System' follows the computer if its language changes while Fuselane is open.
if (typeof window !== 'undefined')
  addEventListener('languagechange', () => {
    if (useLang.getState().pref === 'system') use(resolve('system'))
  })

/** The language in use. Call it in a component (App does) to re-render on a change. */
export function useLocale(): Locale {
  return useLang((s) => s.locale)
}

/** The language in use, outside React. */
export function locale(): Locale {
  return current
}

/**
 * The tag for Intl (numbers, dates, times). Hindi is written as in India, with
 * Western digits (CLDR's default for hi-IN). English keeps the computer's English
 * (en-GB, en-IN…), so its clock and number style stay as people set them.
 */
export function intlLocale(): string {
  if (current === 'hi') return 'hi-IN'
  return languages().find((l) => l.toLowerCase().startsWith('en')) ?? 'en'
}

function lookup(en: string): string {
  const table = CATALOGUES[current]
  return table && Object.hasOwn(table, en) ? (table[en] ?? en) : en
}

function fill(s: string, vars: Vars): string {
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (Object.hasOwn(vars, k) ? String(vars[k]) : m))
}

/**
 * Translates text the Rust core sent (an error, a hint, a note on a download). The core
 * writes English; unknown text is shown as sent. Call it where the text is shown, so a
 * language change applies at once.
 */
export function tb(text: string): string
export function tb(text: string | null | undefined): string | null
export function tb(text: string | null | undefined): string | null {
  if (text == null) return null
  const table = BACKEND[current]
  if (!table) return text
  let c = compiled.get(current)
  if (!c) {
    c = compileBackend(table)
    compiled.set(current, c)
  }
  return translateBackend(text, c)
}

/** Translates an English sentence; `{name}` is filled from `vars`. Use a literal string. */
export function t(en: string, vars?: Vars): string {
  const s = lookup(en)
  return vars ? fill(s, vars) : s
}

const plurals = new Map<Locale, Intl.PluralRules>()

/** Which English form a count takes in the language in use (Hindi: 0 and 1 take `one`). */
function form(n: number, one: string, other: string): string {
  let rules = plurals.get(current)
  if (!rules) {
    rules = new Intl.PluralRules(intlLocale())
    plurals.set(current, rules)
  }
  return rules.select(n) === 'one' ? one : other
}

/** A count with the right form: tn(n, '{n} file', '{n} files'). `{n}` is the count. */
export function tn(n: number, one: string, other: string, vars?: Vars): string {
  return t(form(n, one, other), { n, ...vars })
}

/**
 * A sentence with elements inside it, so the words around them can move in another
 * language: tr('Saved to {path}.', { path: <code>{dir}</code> }).
 */
export function tr(en: string, vars: Record<string, ReactNode>): ReactNode {
  const s = lookup(en)
  const out: ReactNode[] = []
  let last = 0
  let key = 0
  for (const m of s.matchAll(/\{(\w+)\}/g)) {
    const name = m[1] ?? ''
    out.push(s.slice(last, m.index))
    const v = Object.hasOwn(vars, name) ? vars[name] : m[0]
    out.push(isValidElement(v) ? cloneElement(v, { key: key++ }) : v)
    last = m.index + m[0].length
  }
  out.push(s.slice(last))
  return out
}

/** tr() with a count: trn(n, 'In {n} second.', 'In {n} seconds.', { n: <b>{n}</b> }). */
export function trn(n: number, one: string, other: string, vars?: Record<string, ReactNode>) {
  return tr(form(n, one, other), { n, ...vars })
}

/**
 * Marks a sentence for translation where it's defined (a table or constant) when
 * t() translates it later, where it's shown: `label: mark('Video')` … `t(label)`.
 */
export function mark<T extends string>(en: T): T {
  return en
}
