// Turns a design token into RGB for canvas drawing. A canvas that can't parse a
// colour keeps its previous fillStyle (no error), so oklch() is also converted here
// in case an older system webview lacks support.

export type RGB = [number, number, number]

/** OKLCH → sRGB 0..255, gamut-clipped. Formulas from Björn Ottosson's OKLab. */
export function oklchToRgb(L: number, C: number, hDeg: number): RGB {
  const h = (hDeg * Math.PI) / 180
  const a = C * Math.cos(h)
  const b = C * Math.sin(h)
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3
  const lin = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ]
  return lin.map((x) => {
    const v = x <= 0.0031308 ? 12.92 * x : 1.055 * Math.pow(Math.max(0, x), 1 / 2.4) - 0.055
    return Math.round(Math.min(1, Math.max(0, v)) * 255)
  }) as RGB
}

/** Parses `oklch(L C H)` / `oklch(L C H / a)`, with L as 0..1 or a percentage. */
export function parseOklch(text: string): RGB | null {
  const m = /^oklch\(\s*([\d.]+)(%?)\s+([\d.]+)\s+([\d.]+)(?:deg)?\s*(?:\/\s*[\d.%]+\s*)?\)$/i.exec(
    text.trim(),
  )
  if (!m) return null
  const L = Number(m[1]) / (m[2] ? 100 : 1)
  const C = Number(m[3])
  const H = Number(m[4])
  if (![L, C, H].every(Number.isFinite)) return null
  return oklchToRgb(L, C, H)
}

const SENTINEL = '#010203'

/** The current value of a CSS custom property as RGB. */
export function resolveVar(cssVar: string, fallback: RGB = [136, 136, 136]): RGB {
  const value = getComputedStyle(document.documentElement).getPropertyValue(cssVar).trim()
  if (!value) return fallback
  const probe = document.createElement('canvas')
  probe.width = probe.height = 1
  const c = probe.getContext('2d', { willReadFrequently: true })
  if (!c) return parseOklch(value) ?? fallback
  c.fillStyle = SENTINEL
  c.fillStyle = value
  // An unparseable colour leaves fillStyle unchanged instead of throwing.
  if (c.fillStyle === SENTINEL) return parseOklch(value) ?? fallback
  c.fillRect(0, 0, 1, 1)
  const d = c.getImageData(0, 0, 1, 1).data
  return [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0]
}
