// Download page: links from the signed update feed, the visitor's system picked
// out, copy buttons, and the app's own Fuse Core renderer on sample data.
import './style.css'
import { createFuseCore, type CoreData } from '../../desktop/src/components/coreRenderer'

const REPO = 'https://github.com/ArshPunisher/fuselane'
const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)

type Os = 'mac' | 'windows' | 'linux' | 'other'

export function detectOs(ua: string, platform: string): Os {
  const s = `${platform} ${ua}`.toLowerCase()
  if (/iphone|ipad|android/.test(s)) return 'other'
  if (/mac/.test(s)) return 'mac'
  if (/win/.test(s)) return 'windows'
  if (/linux|x11|cros/.test(s)) return 'linux'
  return 'other'
}

function fileUrl(version: string, suffix: string, cli = false) {
  const name = cli ? `fuselane-cli_${version}_${suffix}` : `Fuselane_${version}_${suffix}`
  return `${REPO}/releases/download/v${version}/${name}`
}

async function wireDownloads() {
  const os = detectOs(navigator.userAgent, navigator.platform)
  document.querySelector(`.platform[data-os="${os}"]`)?.setAttribute('data-current', '')
  const primary = $<HTMLAnchorElement>('#primary-download')
  let version: string | null = null
  try {
    const r = await fetch('./updates/latest.json', { cache: 'no-cache' })
    if (r.ok) version = ((await r.json()) as { version?: string }).version ?? null
  } catch {
    /* offline or not deployed yet: links fall back to the releases page */
  }
  document.querySelectorAll<HTMLAnchorElement>('a[data-file]').forEach((a) => {
    a.href = version ? fileUrl(version, a.dataset.file ?? '') : `${REPO}/releases`
  })
  document.querySelectorAll<HTMLAnchorElement>('a[data-cli]').forEach((a) => {
    a.href = version ? fileUrl(version, a.dataset.cli ?? '', true) : `${REPO}/releases`
  })
  if (version) {
    $('#version-tag')!.textContent = version
    $('#version-line')!.textContent = `Version ${version}. Free and open source.`
    $<HTMLAnchorElement>('#sums')!.href = `${REPO}/releases/download/v${version}/SHA256SUMS`
  }
  if (primary) {
    if (os === 'mac') primary.textContent = 'Download for macOS'
    if (os === 'linux') primary.textContent = 'Download for Linux'
    if (os === 'windows') {
      primary.textContent = 'Download for Windows'
      if (version) primary.href = fileUrl(version, 'windows-x64-setup.exe')
    }
  }
}

function wireCopy() {
  document.querySelectorAll<HTMLButtonElement>('button[data-copy]').forEach((b) => {
    b.addEventListener('click', async () => {
      const text = document.getElementById(b.dataset.copy ?? '')?.textContent ?? ''
      try {
        await navigator.clipboard.writeText(text)
        b.textContent = 'Copied'
      } catch {
        b.textContent = 'Select and copy'
      }
      setTimeout(() => (b.textContent = 'Copy'), 2000)
    })
  })
}

/** Sample data: three networks filling 180 ticks, then the file "completes" and restarts. */
function demoCore() {
  const canvas = $<HTMLCanvasElement>('#core')
  if (!canvas) return
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches
  const core = createFuseCore(canvas, 180)
  core.setReduced(reduced)
  const lanes = ['tide', 'volt', 'iris']
  const base = [7.2, 3.1, 10.4]
  const ticks = 180
  let fill = new Float32Array(ticks)
  let owner = new Int8Array(ticks).fill(-1)
  let inflight = new Int8Array(ticks).fill(-1)
  let held = [-1, -1, -1]
  let t = 0
  let doneFor = 0
  const next = () => {
    for (let i = 0; i < ticks; i++) if (fill[i]! < 1 && !held.includes(i)) return i
    return -1
  }
  const rates = () => base.map((b, i) => b * (1 + 0.18 * Math.sin(t * (0.7 + i * 0.3) + i * 2)))
  const step = (dt: number) => {
    t += dt
    const r = rates()
    if (doneFor > 0) {
      doneFor -= dt
      if (doneFor <= 0) {
        fill = new Float32Array(ticks)
        owner = new Int8Array(ticks).fill(-1)
        inflight = new Int8Array(ticks).fill(-1)
        held = [-1, -1, -1]
      }
    } else {
      r.forEach((rate, n) => {
        if (held[n]! < 0) held[n] = next()
        const k = held[n]!
        if (k < 0) return
        inflight[k] = n
        fill[k] = Math.min(1, fill[k]! + rate * dt * 0.3)
        if (fill[k]! >= 1) {
          owner[k] = n
          inflight[k] = -1
          held[n] = next()
        }
      })
      if (fill.every((f) => f >= 1)) doneFor = 2.5
    }
    const data: CoreData = {
      fill,
      owner,
      inflight,
      nets: lanes.map((lane, i) => ({ lane, rate: doneFor > 0 ? 0 : r[i]!, dead: false })),
      complete: doneFor > 0,
    }
    core.setData(data)
    const total = r.reduce((a, b) => a + b, 0)
    $('#speed')!.textContent = doneFor > 0 ? '100' : total.toFixed(1)
    $('.core .unit')!.textContent = doneFor > 0 ? '%' : 'MB/s'
    $('#speed-sub')!.textContent =
      doneFor > 0 ? 'Done' : `${(total / Math.max(...r)).toFixed(1)}x one network`
  }
  new ResizeObserver(() => core.resize()).observe(canvas)
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => core.refreshColors())
  let last = performance.now()
  let acc = 0
  const frame = (now: number) => {
    const dt = Math.min(0.05, (now - last) / 1000)
    last = now
    acc += dt
    if (acc > 0.2) {
      step(acc)
      acc = 0
    }
    core.step(dt)
    core.render()
    requestAnimationFrame(frame)
  }
  core.resize()
  step(0.2)
  requestAnimationFrame(frame)
}

void wireDownloads()
wireCopy()
demoCore()
