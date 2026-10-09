const UNITS = ['B', 'KB', 'MB', 'GB', 'TB']
const NBSP = '\u00A0'
const nf0 = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 })
const nf1 = new Intl.NumberFormat(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 })

export function splitBytes(n: number): { value: string; unit: string } {
  let v = Math.max(0, n)
  let i = 0
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024
    i++
  }
  const value = i === 0 || v >= 100 ? nf0.format(v) : nf1.format(v)
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
      value: i === 0 || v >= 100 ? nf0.format(v) : nf1.format(v),
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
  return new Date(unix * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

/** "Starts at 02:00", "Starts tomorrow at 02:00", "Starts Mon at 02:00" (B8.5). */
export function startsAt(unix: number, now = Date.now()): string {
  const at = new Date(unix * 1000)
  const today = new Date(now)
  const days = Math.round(
    (new Date(at.getFullYear(), at.getMonth(), at.getDate()).getTime() -
      new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime()) /
      86_400_000,
  )
  const time = clockTime(unix)
  if (days <= 0) return `Starts at ${time}`
  if (days === 1) return `Starts tomorrow at ${time}`
  return `Starts ${at.toLocaleDateString([], { weekday: 'short' })} at ${time}`
}

/** "Ready by 08:00", "Ready by tomorrow 08:00", "Ready by Fri 08:00". */
export function readyBy(unix: number, now = Date.now()): string {
  return startsAt(unix, now)
    .replace(/^Starts (at )?/, 'Ready by ')
    .replace(' at ', ' ')
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
  if (s < 60) return `${Math.max(1, Math.round(s))}${NBSP}s left`
  if (s < 3600) return `${Math.round(s / 60)}${NBSP}min left`
  const h = Math.floor(s / 3600)
  return `${h}${NBSP}h ${Math.round((s - h * 3600) / 60)}${NBSP}min left`
}

export function percent(written: number, total: number | null): number | null {
  if (!total || total <= 0) return null
  return Math.min(100, (written / total) * 100)
}
