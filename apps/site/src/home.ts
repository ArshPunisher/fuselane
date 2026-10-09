// Home: the fusion hero, the "combine your networks" toy, and the download button
// for the visitor's system.
import { detectOs, fileUrl, latestVersion } from './common'
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

/** Turn networks on and off; the combined speed and the time for a 4.7 GB file follow. */
function combine() {
  const toggles = [...document.querySelectorAll<HTMLButtonElement>('.net-toggle')]
  if (!toggles.length) return
  const big = $('#combined')
  const single = $('#time-one')
  const fused = $('#time-all')
  const bars = [...document.querySelectorAll<HTMLElement>('.stack span')]
  const caption = $('#combined-caption')
  const SIZE_MB = 4.7 * 1024
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
