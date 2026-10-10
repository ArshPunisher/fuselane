import { intlLocale, t, tn } from './i18n'

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB']
const NBSP = '\u00A0'

// Numbers in the language in use (Hindi keeps Western digits, as hi-IN does).
const formats = new Map<string, Intl.NumberFormat>()
function nf(digits: 0 | 1): Intl.NumberFormat {
  const tag = intlLocale()
  const key = `${tag}/${digits}`
  let f = formats.get(key)
  if (!f) {
    f = new Intl.NumberFormat(tag, { minimumFractionDigits: digits, maximumFractionDigits: digits })
    formats.set(key, f)
  }
  return f
}

/** A count or plain number, as the language in use writes it. */
export function num(n: number): string {
  return new Intl.NumberFormat(intlLocale()).format(n)
}

export function splitBytes(n: number): { value: string; unit: string } {
  let v = Math.max(0, n)
  let i = 0
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024
    i++
  }
  const value = i === 0 || v >= 100 ? nf(0).format(v) : nf(1).format(v)
  return { value, unit: UNITS[i] ?? 'B' }
}

export function bytes(n: number): string {
  const s = splitBytes(n)
  return `${s.value}${NBSP}${s.unit}`
}

/** Speeds in bytes (MB/s, as files are measured) or bits (Mbps, as internet plans are sold). */
export type SpeedUnit = 'bytes' | 'bits'
let speedUnit: SpeedUnit = 'bytes'
export function setSpeedUnit(u: SpeedUnit) {
  speedUnit = u
}

const BIT_UNITS = ['bps', 'Kbps', 'Mbps', 'Gbps', 'Tbps']

export function rate(n: number): { value: string; unit: string } {
  if (speedUnit === 'bits') {
    let v = Math.max(0, n) * 8
    let i = 0
    while (v >= 1000 && i < BIT_UNITS.length - 1) {
      v /= 1000
      i++
    }
    return {
      value: i === 0 || v >= 100 ? nf(0).format(v) : nf(1).format(v),
      unit: BIT_UNITS[i] ?? 'bps',
    }
  }
  const s = splitBytes(n)
  return { value: s.value, unit: `${s.unit}/s` }
}

export function rateText(n: number): string {
  const r = rate(n)
  return `${r.value}${NBSP}${r.unit}`
}

/** The local clock time, as the person's system writes it ("02:00", "2:00 AM"). */
export function clockTime(unix: number): string {
  return new Date(unix * 1000).toLocaleTimeString(intlLocale(), {
    hour: '2-digit',
    minute: '2-digit',
  })
}

/** Days from today to `unix`, by the calendar (0 today, 1 tomorrow). */
function daysAway(unix: number, now: number): number {
  const at = new Date(unix * 1000)
  const today = new Date(now)
  return Math.round(
    (new Date(at.getFullYear(), at.getMonth(), at.getDate()).getTime() -
      new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime()) /
      86_400_000,
  )
}

function weekday(unix: number): string {
  return new Date(unix * 1000).toLocaleDateString(intlLocale(), { weekday: 'short' })
}

/** "at 02:00", "tomorrow at 02:00", "Mon at 02:00". */
export function when(unix: number, now = Date.now()): string {
  const days = daysAway(unix, now)
  const time = clockTime(unix)
  if (days <= 0) return t('at {time}', { time })
  if (days === 1) return t('tomorrow at {time}', { time })
  return t('{day} at {time}', { day: weekday(unix), time })
}

/** "Starts at 02:00", "Starts tomorrow at 02:00", "Starts Mon at 02:00" (B8.5). */
export function startsAt(unix: number, now = Date.now()): string {
  return t('Starts {when}', { when: when(unix, now) })
}

/** "Ready by 08:00", "Ready by tomorrow 08:00", "Ready by Fri 08:00". */
export function readyBy(unix: number, now = Date.now()): string {
  const days = daysAway(unix, now)
  const time = clockTime(unix)
  if (days <= 0) return t('Ready by {time}', { time })
  if (days === 1) return t('Ready by tomorrow {time}', { time })
  return t('Ready by {day} {time}', { day: weekday(unix), time })
}

/** The next time the clock shows "HH:MM": today if it's still ahead, else tomorrow. */
export function nextAt(hhmm: string, now = Date.now()): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(hhmm.trim())
  if (!m) return null
  const h = Number(m[1])
  const min = Number(m[2])
  if (h > 23 || min > 59) return null
  const d = new Date(now)
  d.setHours(h, min, 0, 0)
  if (d.getTime() <= now) d.setDate(d.getDate() + 1)
  return Math.floor(d.getTime() / 1000)
}

/** "3 min left", "40 s left"; empty when unknown or not moving. */
export function eta(remaining: number, speed: number): string {
  if (!(speed > 1) || !(remaining > 0)) return ''
  const s = remaining / speed
  if (s < 60) return t('{n}\u00A0s left', { n: Math.max(1, Math.round(s)) })
  if (s < 3600) return t('{n}\u00A0min left', { n: Math.round(s / 60) })
  const h = Math.floor(s / 3600)
  return tn(h, '1\u00A0h {m}\u00A0min left', '{n}\u00A0h {m}\u00A0min left', {
    m: Math.round((s - h * 3600) / 60),
  })
}

export function percent(written: number, total: number | null): number | null {
  if (!total || total <= 0) return null
  return Math.min(100, (written / total) * 100)
}
