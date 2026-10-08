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

export function rate(n: number): { value: string; unit: string } {
  const s = splitBytes(n)
  return { value: s.value, unit: `${s.unit}/s` }
}

export function rateText(n: number): string {
  const r = rate(n)
  return `${r.value}${NBSP}${r.unit}`
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
