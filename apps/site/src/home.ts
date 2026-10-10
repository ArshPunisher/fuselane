// Home: the fusion hero, the "combine your networks" toy, and the download button
// for the visitor's system.
import { detectOs, fileUrl, latestVersion } from './common'
import './site.css'
import { startFusion } from './fusion'

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)
const fmt = new Intl.NumberFormat(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 })

/** "54 s", "2 min 32 s". */
export function duration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0 s'
  const s = Math.round(seconds)
  if (s < 60) return `${s} s`
  const m = Math.floor(s / 60)
  const rest = s % 60
  return rest ? `${m} min ${rest} s` : `${m} min`
}

function hero() {
  const root = $('.fusion')
  if (!root) return
  const lanes = [
    { name: 'Wi-Fi', color: 'oklch(0.78 0.12 215)', base: 41.2 },
    { name: 'Ethernet', color: 'oklch(0.72 0.14 285)', base: 33.8 },
    { name: 'iPhone USB', color: 'oklch(0.86 0.17 128)', base: 12.4 },
  ]
  const rateEls = [...root.querySelectorAll<HTMLElement>('[data-lane-rate]')]
  const total = $('#total')
  const gain = $('#gain')
  const fill = $('.file-chip .fill')
  const left = $('#file-left')
  startFusion(root, lanes, (rates, sum, progress, secondsLeft) => {
    rates.forEach((r, i) => {
      const el = rateEls[i]
      if (el) el.textContent = fmt.format(r)
    })
    if (total) total.textContent = fmt.format(sum)
    if (gain) gain.textContent = `${fmt.format(sum / Math.max(...rates))}× one network`
    if (fill) fill.style.width = `${(progress * 100).toFixed(2)}%`
    if (left) left.textContent = secondsLeft > 0 ? `${duration(secondsLeft)} left` : 'Done'
  })
}

/**
 * A rolling-digit counter: each digit is a 0-9 strip that slides to its value.
 * Separators stay put. Columns are reused while the number keeps its shape.
 */
export function odometer(el: HTMLElement) {
  let shape = ''
  return (text: string) => {
    const next = text.replace(/\d/g, '0')
    if (next !== shape) {
      shape = next
      el.replaceChildren(
        ...[...text].map((ch) => {
          const cell = document.createElement('span')
          if (!/\d/.test(ch)) {
            cell.className = 'odo-sep'
            cell.textContent = ch
            return cell
          }
          cell.className = 'odo-d'
          const strip = document.createElement('span')
          strip.className = 'odo-strip'
          strip.textContent = '0123456789'
          cell.append(strip)
          return cell
        }),
      )
    }
    const strips = el.querySelectorAll<HTMLElement>('.odo-strip')
    let i = 0
    for (const ch of text) {
      if (!/\d/.test(ch)) continue
      strips[i++]?.style.setProperty('--n', ch)
    }
  }
}

/** Turn networks on and off; the combined speed and the time for a 4.7 GB file follow. */
function combine() {
  const toggles = [...document.querySelectorAll<HTMLButtonElement>('.net-toggle')]
  if (!toggles.length) return
  const big = $('#combined')
  const single = $('#time-one')
  const fused = $('#time-all')
  const bars = [...document.querySelectorAll<HTMLElement>('.stack span')]
  const caption = $('#combined-caption')
  const odo = document.querySelector<HTMLElement>('.odo')
  const roll = odo ? odometer(odo) : () => {}
  const rowSpeed = toggles.map((t) => t.querySelector<HTMLElement>('.speed'))
  const SIZE_MB = 4.7 * 1024
  // Real networks never sit still: while the section is on screen, each speed
  // drifts a few percent and the total runs with them. The exact figures (for
  // screen readers and the times) stay on the nominal speeds.
  const still = matchMedia('(prefers-reduced-motion: reduce)').matches
  let drift = toggles.map(() => 1)
  const show = () => {
    const on = toggles.map((t) => t.getAttribute('aria-checked') === 'true')
    const speeds = toggles.map((t) => Number(t.dataset.speed))
    const live = speeds.map((v, i) => v * drift[i]!)
    rowSpeed.forEach((el, i) => {
      if (el) el.textContent = `${fmt.format(live[i]!)} MB/s`
    })
    roll(fmt.format(live.reduce((a, v, i) => a + (on[i] ? v : 0), 0)))
  }
  const update = () => {
    const on = toggles.map((t) => t.getAttribute('aria-checked') === 'true')
    const speeds = toggles.map((t) => Number(t.dataset.speed))
    const sum = speeds.reduce((a, s, i) => a + (on[i] ? s : 0), 0)
    const best = Math.max(0, ...speeds.filter((_, i) => on[i]))
    if (big) big.textContent = fmt.format(sum)
    bars.forEach(
      (b, i) =>
        (b.style.width =
          on[i] && sum > 0
            ? `${((speeds[i] ?? 0) / speeds.reduce((a, s) => a + s, 0)) * 100}%`
            : '0%'),
    )
    if (single) single.textContent = best ? duration(SIZE_MB / best) : 'No network'
    if (fused) fused.textContent = sum ? duration(SIZE_MB / sum) : 'No network'
    const count = on.filter(Boolean).length
    if (caption)
      caption.textContent =
        count === 0
          ? 'Turn a network on.'
          : count === 1
            ? 'One network: as fast as it goes, no faster.'
            : `${count} networks at once: their speeds add up.`
    show()
  }
  if (!still && odo) {
    let visible = false
    new IntersectionObserver(([e]) => (visible = Boolean(e?.isIntersecting))).observe(odo)
    setInterval(() => {
      if (!visible || document.hidden) return
      drift = drift.map((d) => Math.min(1.05, Math.max(0.95, d + (Math.random() - 0.5) * 0.04)))
      show()
    }, 900)
  }
  toggles.forEach((t) =>
    t.addEventListener('click', () => {
      t.setAttribute('aria-checked', t.getAttribute('aria-checked') === 'true' ? 'false' : 'true')
      update()
    }),
  )
  update()
}

async function primaryDownload() {
  const btn = $<HTMLAnchorElement>('#primary-download')
  const label = $('#primary-label')
  const line = $('#version-line')
  const os = detectOs(navigator.userAgent, navigator.platform)
  const version = await latestVersion('./')
  const names: Record<string, string> = { mac: 'macOS', windows: 'Windows', linux: 'Linux' }
  if (label && names[os]) label.textContent = `Download for ${names[os]}`
  if (btn && version) {
    if (os === 'mac') btn.href = fileUrl(version, 'macos-universal.dmg')
    else if (os === 'windows') btn.href = fileUrl(version, 'windows-x64-setup.exe')
  }
  if (line && version) line.textContent = `Version ${version}. Free and open source, no account.`
}

hero()
combine()
void primaryDownload()
