// The hero's Fuse Core: wiring between the canvas, the network switches by
// each satellite, the readout in the middle, and the pointer (a slight tilt
// toward it, and hovering a network lights up its share of the ring).
import { createCore, type CoreNet, type CoreFile, type CoreState } from './core'
import { duration, loop, prefersReduced, approach, watch } from './motion'
import { isStill } from './common'

const NETS: CoreNet[] = [
  { name: 'Wi-Fi', lane: 'tide', base: 41.2 },
  { name: 'Ethernet', lane: 'iris', base: 33.8 },
  { name: 'iPhone USB', lane: 'volt', base: 12.4 },
]

// Sample files, as in the app's own demo.
const FILES: CoreFile[] = [
  { name: 'ubuntu-26.04-desktop-amd64.iso', mb: 1100 },
  { name: 'Blender-5.1-macos-arm64.dmg', mb: 412 },
  { name: 'Sprite Fright (2021) 4K.mkv', mb: 2800 },
]

const fmt = new Intl.NumberFormat('en-US', { minimumFractionDigits: 1, maximumFractionDigits: 1 })
const pct = new Intl.NumberFormat('en-US', { style: 'percent', maximumFractionDigits: 0 })

export function startHero() {
  const root = document.querySelector<HTMLElement>('.core')
  const stage = root?.querySelector<HTMLElement>('.core-stage')
  const box = root?.querySelector<HTMLElement>('.core-canvas')
  const canvas = box?.querySelector('canvas')
  if (!root || !stage || !box || !canvas) return
  const reduced = prefersReduced()
  const switches = [...root.querySelectorAll<HTMLButtonElement>('.sat')]
  const rateEls = [...root.querySelectorAll<HTMLElement>('[data-lane-rate]')]
  const total = root.querySelector<HTMLElement>('#total')
  const numEl = root.querySelector<HTMLElement>('.core-num')
  const gain = root.querySelector<HTMLElement>('#gain')
  const fileName = root.querySelector<HTMLElement>('#file-name')
  const fileLeft = root.querySelector<HTMLElement>('#file-left')
  const live = root.querySelector<HTMLElement>('#core-live')

  // The speed changes about once a second and is readable at every instant:
  // the new value replaces the old at once, and only the digits that changed
  // glow briefly (colour only, nothing moves or overlaps).
  let shown = numEl?.textContent ?? ''
  const showNumber = (text: string) => {
    if (!numEl || text === shown) return
    const prev = shown
    shown = text
    numEl.replaceChildren(
      ...[...text].map((ch, i) => {
        const cell = document.createElement('span')
        cell.textContent = ch
        if (!reduced && prev.length === text.length && prev[i] !== ch) cell.className = 'changed'
        return cell
      }),
    )
  }

  const show = (s: CoreState) => {
    s.rates.forEach((r, i) => {
      const el = rateEls[i]
      if (el) el.textContent = s.on[i] ? fmt.format(r) : 'off'
    })
    const sum = s.rates.reduce((a, b) => a + b, 0)
    if (total) total.textContent = fmt.format(sum)
    showNumber(fmt.format(sum))
    const best = Math.max(...s.rates.filter((_, i) => s.on[i]), 0)
    root.classList.toggle('core-done', s.done)
    if (gain)
      gain.textContent = s.done
        ? 'Done, checked'
        : !s.on.some(Boolean)
          ? 'Turn a network on'
          : s.on.filter(Boolean).length === 1
            ? 'One network'
            : `${fmt.format(sum / Math.max(best, 0.1))}× one network`
    if (fileName && fileName.textContent !== s.file.name) fileName.textContent = s.file.name
    if (fileLeft)
      fileLeft.textContent = s.done
        ? 'Done'
        : s.left > 0
          ? `${pct.format(s.progress)}, ${duration(s.left)} left`
          : `${pct.format(s.progress)}, waiting for a network`
  }

  const core = createCore(canvas, NETS, FILES, show)
  if (!core) return

  // Satellite labels sit on the orbit when there is room for them.
  const place = () => {
    const wide = box.clientWidth >= 520
    root.toggleAttribute('data-wide', wide)
    if (!wide) return
    const g = core.geometry()
    switches.forEach((b, i) => {
      const s = g.sats[i]
      if (!s) return
      b.style.setProperty('--sx', `${s.x.toFixed(1)}px`)
      b.style.setProperty('--sy', `${s.y.toFixed(1)}px`)
      b.toggleAttribute('data-below', s.y > g.c.y)
    })
  }

  const still = () => {
    core.settle()
    core.render()
  }

  new ResizeObserver(() => {
    core.resize()
    place()
    if (reduced) core.render()
  }).observe(box)
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    core.refreshColors()
    if (reduced) core.render()
  })

  switches.forEach((b, i) => {
    b.addEventListener('click', () => {
      const next = b.getAttribute('aria-checked') !== 'true'
      b.setAttribute('aria-checked', String(next))
      core.setOn(i, next)
      if (reduced) still()
      const names = NETS.filter((_, k) => core.isOn(k)).map((x) => x.name)
      const sum = NETS.reduce((a, x, k) => a + (core.isOn(k) ? x.base : 0), 0)
      if (live)
        live.textContent = names.length
          ? `${names.join(' and ')}: about ${fmt.format(sum)} MB/s together.`
          : 'No network on.'
    })
    b.addEventListener('pointerenter', () => core.setFocus(i))
    b.addEventListener('pointerleave', () => core.setFocus(-1))
    b.addEventListener('focus', () => core.setFocus(i))
    b.addEventListener('blur', () => core.setFocus(-1))
  })

  place()
  core.render()
  if (reduced) {
    still()
    return
  }

  // A slight tilt toward the pointer, eased, and only while the hero is on screen.
  let tx = 0
  let ty = 0
  let rx = 0
  let ry = 0
  if (matchMedia('(hover: hover)').matches) {
    const hero = root.closest('.hero') ?? root
    hero.addEventListener('pointermove', (e) => {
      const r = stage.getBoundingClientRect()
      const ex = (e as PointerEvent).clientX
      const ey = (e as PointerEvent).clientY
      tx = Math.max(-1, Math.min(1, (ex - (r.left + r.width / 2)) / (r.width / 1.2)))
      ty = Math.max(-1, Math.min(1, (ey - (r.top + r.height / 2)) / (r.height / 1.2)))
    })
    hero.addEventListener('pointerleave', () => {
      tx = 0
      ty = 0
    })
  }
  let visible = true
  watch(stage, (v) => (visible = v))
  loop(stage, (dt) => {
    if (isStill()) return
    core.step(dt)
    core.render()
    rx = approach(rx, -ty * 5, 4, dt)
    ry = approach(ry, tx * 6, 4, dt)
    if (visible) {
      stage.style.setProperty('--rx', `${rx.toFixed(2)}deg`)
      stage.style.setProperty('--ry', `${ry.toFixed(2)}deg`)
    }
  })
}
