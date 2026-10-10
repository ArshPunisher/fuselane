// The hero: the Fuse Core (DESIGN-SYSTEM §7), the app's own signature view,
// running a sample download. The file is a ring of block ticks; each network
// is a satellite on a dotted orbit, firing comets that sweep along the orbit
// and dive into the exact tick it is fetching. A tick takes the colour of the
// network that finished it, so the ring becomes a record of who carried what.
// When the ring is full, a Fuse sweep ignites it and the next file starts.
// Sample data, labelled as such on the page. Plain canvas, no React.

import { approach } from './motion'

export type Lane = 'tide' | 'iris' | 'volt'

export interface CoreNet {
  name: string
  lane: Lane
  /** Typical MB/s. */
  base: number
}

export interface CoreFile {
  name: string
  mb: number
}

export interface CoreState {
  rates: number[]
  on: boolean[]
  total: number
  progress: number
  /** Seconds left at the current speed; 0 when done or stalled. */
  left: number
  file: CoreFile
  done: boolean
}

interface Comet {
  net: number
  tick: number
  t: number
  speed: number
}

const TICKS = 144
const STREAMS = 2
const MAX_COMETS = 150
const IGNITE = 2.4

type RGB = [number, number, number]

/** Any CSS colour (oklch included) as RGB, via a 1×1 canvas: the browser does the maths. */
function rgbOf(color: string): RGB {
  const c = document.createElement('canvas')
  c.width = c.height = 1
  const x = c.getContext('2d', { willReadFrequently: true })
  if (!x) return [128, 128, 128]
  x.fillStyle = '#808080'
  x.fillStyle = color
  x.fillRect(0, 0, 1, 1)
  const d = x.getImageData(0, 0, 1, 1).data
  return [d[0] ?? 128, d[1] ?? 128, d[2] ?? 128]
}

const rgba = (c: RGB, a: number) =>
  `rgba(${c[0] | 0},${c[1] | 0},${c[2] | 0},${Math.max(0, Math.min(1, a)).toFixed(3)})`
const mix = (a: RGB, b: RGB, k: number): RGB => [
  a[0] + (b[0] - a[0]) * k,
  a[1] + (b[1] - a[1]) * k,
  a[2] + (b[2] - a[2]) * k,
]
const TAU = Math.PI * 2
const wrap = (a: number) => Math.atan2(Math.sin(a), Math.cos(a))

/** A tiny seeded random, so the first frame looks the same on every visit. */
function seeded(seed: number) {
  let s = seed >>> 0
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0
    return s / 2 ** 32
  }
}

export function createCore(
  canvas: HTMLCanvasElement,
  nets: CoreNet[],
  files: CoreFile[],
  onState: (s: CoreState) => void,
) {
  const ctx = canvas.getContext('2d')
  if (!ctx) return null
  const n = nets.length
  let w = 1
  let h = 1
  let dark = true
  let colors = {
    lanes: nets.map((): RGB => [128, 128, 128]),
    fuse: [253, 133, 55] as RGB,
    hot: [255, 200, 140] as RGB,
    mute: [140, 145, 155] as RGB,
    ink: [240, 240, 245] as RGB,
    flash: [255, 255, 255] as RGB,
  }

  // Simulation state.
  const fill = new Float32Array(TICKS)
  const owner = new Int8Array(TICKS).fill(-1)
  const flying = new Int8Array(TICKS).fill(-1)
  const doneAt = new Float32Array(TICKS).fill(-10)
  const on = nets.map(() => true)
  const rates = nets.map((x) => x.base)
  const spin = nets.map((_, i) => i * 1.7)
  const spread = nets.map(() => 0)
  const emit = nets.map(() => 0)
  const streams: number[][] = nets.map(() => [])
  let comets: Comet[] = []
  let fileIndex = 0
  let filled = 0
  let clock = 0
  let ignite = -1
  let focus = -1
  let sinceState = 1
  const rnd = seeded(7)

  const file = () => files[fileIndex % files.length]!
  const mbPerTick = () => file().mb / TICKS

  function refreshColors() {
    const css = getComputedStyle(canvas)
    const read = (v: string) => rgbOf(css.getPropertyValue(v).trim() || '#888')
    colors = {
      lanes: nets.map((x) => read(`--${x.lane}`)),
      fuse: read('--fuse'),
      hot: read('--fuse-hot'),
      mute: read('--mute'),
      ink: read('--ink'),
      flash: [255, 255, 255],
    }
    const bg = read('--canvas')
    dark = bg[0] + bg[1] + bg[2] < 384
    if (!dark) colors.flash = colors.ink
  }

  function resize() {
    const r = canvas.getBoundingClientRect()
    const dpr = Math.min(window.devicePixelRatio || 1, 2)
    w = Math.max(1, r.width)
    h = Math.max(1, r.height)
    canvas.width = Math.round(w * dpr)
    canvas.height = Math.round(h * dpr)
    ctx!.setTransform(dpr, 0, 0, dpr, 0, 0)
  }

  /** Where everything sits, in CSS pixels. Satellites: upper left, upper right, bottom. */
  function geometry() {
    const s = Math.min(w, h)
    const c = { x: w / 2, y: h / 2 }
    const ring = s * 0.305
    const tick = Math.max(6, s * 0.045)
    const orbit = s * 0.445
    const sats = nets.map((_, i) => {
      const a = ((-150 + (i * 360) / n) * Math.PI) / 180
      return { a, x: c.x + Math.cos(a) * orbit, y: c.y + Math.sin(a) * orbit }
    })
    return { s, c, ring, tick, orbit, sats }
  }

  function nextPending(): number {
    for (let k = 0; k < TICKS; k++) if (owner[k]! < 0 && flying[k]! < 0) return k
    return -1
  }

  function release(i: number) {
    for (const k of streams[i]!) flying[k] = -1
    streams[i] = []
  }

  function startFile(prefill: number) {
    fill.fill(0)
    owner.fill(-1)
    flying.fill(-1)
    doneAt.fill(-10)
    streams.forEach((_, i) => (streams[i] = []))
    comets = []
    filled = 0
    ignite = -1
    // A head start, shared out by speed, so the first frame already tells the story.
    const total = nets.reduce((a, x) => a + x.base, 0)
    const count = Math.floor(TICKS * prefill)
    for (let k = 0; k < count; k++) {
      let r = rnd() * total
      let pick = 0
      for (let i = 0; i < n; i++) {
        r -= nets[i]!.base
        if (r <= 0) {
          pick = i
          break
        }
      }
      fill[k] = 1
      owner[k] = pick
      filled += 1
    }
  }

  function step(dt: number) {
    clock += dt
    if (ignite >= 0) {
      ignite += dt
      if (ignite > IGNITE) {
        fileIndex++
        startFile(0)
      }
    }
    for (let i = 0; i < n; i++) {
      const net = nets[i]!
      const live = on[i]
        ? net.base *
          (1 +
            0.07 * Math.sin(clock * (0.55 + i * 0.27) + i * 2.1) +
            0.035 * Math.sin(clock * 2.2 + i * 1.3))
        : 0
      // A network turned off slows to a stop over about 300 ms, not a cut.
      rates[i] = approach(rates[i]!, live, 6, dt)
      spread[i] = approach(spread[i]!, on[i] ? 0 : 1, 5, dt)
      spin[i]! += dt * (0.35 + rates[i]! / 12)
      if (ignite >= 0) continue
      if (!on[i]) {
        if (streams[i]!.length) release(i)
        continue
      }
      while (streams[i]!.length < STREAMS) {
        const k = nextPending()
        if (k < 0) break
        flying[k] = i
        streams[i]!.push(k)
      }
      const per = (rates[i]! / Math.max(1, streams[i]!.length)) * dt
      streams[i] = streams[i]!.filter((k) => {
        const before = fill[k]!
        const next = Math.min(1, before + per / mbPerTick())
        fill[k] = next
        filled += next - before
        if (next >= 1) {
          owner[k] = i
          flying[k] = -1
          doneAt[k] = clock
          return false
        }
        return true
      })
      // Comets: as many and as quick as the network is fast.
      emit[i]! += dt * (3 + rates[i]! * 0.42)
      while (emit[i]! >= 1) {
        emit[i]! -= 1
        const targets = streams[i]!
        if (!targets.length || comets.length >= MAX_COMETS) continue
        comets.push({
          net: i,
          tick: targets[Math.floor(rnd() * targets.length)]!,
          t: 0,
          speed: 1 / (1.05 - Math.min(0.45, rates[i]! / 110)),
        })
      }
    }
    if (ignite < 0 && filled >= TICKS - 1e-3) {
      ignite = 0
      comets = []
      streams.forEach((_, i) => (streams[i] = []))
    }
    for (const c of comets) c.t += dt * c.speed
    comets = comets.filter((c) => c.t < 1)
    sinceState += dt
    // Numbers on the page change about once a second, so they can be read.
    if (sinceState >= 1) emitState()
  }

  function emitState() {
    sinceState = 0
    const total = rates.reduce((a, b) => a + b, 0)
    const progress = filled / TICKS
    const done = ignite >= 0
    onState({
      rates: [...rates],
      on: [...on],
      total,
      progress: done ? 1 : progress,
      left: done || total < 0.05 ? 0 : ((1 - progress) * file().mb) / total,
      file: file(),
      done,
    })
  }

  function render() {
    const g = geometry()
    const { c, ring, tick, orbit } = g
    ctx!.clearRect(0, 0, w, h)
    const total = rates.reduce((a, b) => a + b, 0)
    const max = nets.reduce((a, x) => a + x.base, 0)
    const heat = Math.min(1, total / max)

    // A warm glow at the core, as strong as the combined speed.
    const glow = ctx!.createRadialGradient(c.x, c.y, ring * 0.2, c.x, c.y, ring * 1.6)
    glow.addColorStop(0, rgba(colors.fuse, (dark ? 0.16 : 0.08) * heat))
    glow.addColorStop(1, rgba(colors.fuse, 0))
    ctx!.fillStyle = glow
    ctx!.fillRect(0, 0, w, h)

    // The orbit.
    ctx!.save()
    ctx!.strokeStyle = rgba(colors.mute, dark ? 0.55 : 0.6)
    ctx!.lineWidth = 1.7
    ctx!.lineCap = 'round'
    ctx!.setLineDash([0.1, Math.max(5, (TAU * orbit) / 150)])
    ctx!.beginPath()
    ctx!.arc(c.x, c.y, orbit, 0, TAU)
    ctx!.stroke()
    ctx!.restore()

    // Progress: a fine Fuse arc inside the ring, on a hairline.
    const inner = ring - tick * 0.45
    ctx!.lineCap = 'round'
    ctx!.strokeStyle = rgba(colors.mute, 0.16)
    ctx!.lineWidth = 1
    ctx!.beginPath()
    ctx!.arc(c.x, c.y, inner, 0, TAU)
    ctx!.stroke()
    const prog = ignite >= 0 ? 1 : filled / TICKS
    ctx!.strokeStyle = rgba(colors.fuse, 0.95)
    ctx!.lineWidth = 2
    ctx!.beginPath()
    ctx!.arc(c.x, c.y, inner, -Math.PI / 2, -Math.PI / 2 + prog * TAU)
    ctx!.stroke()

    // The ignition sweep, once the file is whole.
    const sweep = ignite >= 0 ? Math.min(1, ignite / 0.75) : -1
    const sweepEase =
      sweep < 0 ? -1 : sweep < 0.5 ? 2 * sweep * sweep : 1 - (-2 * sweep + 2) ** 2 / 2

    // The ring of ticks.
    ctx!.lineWidth = Math.max(1.6, (TAU * ring) / TICKS / 2.6)
    for (let k = 0; k < TICKS; k++) {
      const a = -Math.PI / 2 + ((k + 0.5) / TICKS) * TAU
      const cos = Math.cos(a)
      const sin = Math.sin(a)
      let len = tick * 0.42
      let color = colors.mute
      let alpha = dark ? 0.22 : 0.28
      const own = owner[k]!
      const fly = flying[k]!
      if (own >= 0) {
        color = colors.lanes[own]!
        len = tick
        alpha = focus >= 0 && focus !== own ? 0.22 : 1
        const since = clock - doneAt[k]!
        if (since < 0.45) {
          const f = 1 - since / 0.45
          len *= 1 + 0.35 * f
          color = mix(color, colors.flash, 0.7 * f)
        }
      } else if (fly >= 0) {
        color = colors.lanes[fly]!
        len = tick * (0.42 + 0.7 * fill[k]!)
        alpha = (0.45 + 0.55 * fill[k]!) * (focus >= 0 && focus !== fly ? 0.3 : 1)
      } else if (fill[k]! > 0) {
        len = tick * (0.42 + 0.5 * fill[k]!)
        alpha = 0.4
      }
      if (sweepEase >= 0) {
        // Each tick flashes Fuse as the sweep passes it, then settles back.
        const at = (k + 0.5) / TICKS
        const since = ignite - at * 0.75
        if (at <= sweepEase && since >= 0) {
          const f = Math.max(0, 1 - since / 0.9)
          color = mix(color, colors.hot, f)
          len *= 1 + 0.25 * f
        }
      }
      ctx!.strokeStyle = rgba(color, alpha)
      ctx!.beginPath()
      ctx!.moveTo(c.x + cos * ring, c.y + sin * ring)
      ctx!.lineTo(c.x + cos * (ring + len), c.y + sin * (ring + len))
      ctx!.stroke()
    }

    // Comets: sweep along the orbit, then dive into their tick. Polar paths,
    // so they never cross the readout in the middle.
    ctx!.save()
    if (dark) ctx!.globalCompositeOperation = 'lighter'
    ctx!.lineCap = 'round'
    for (const cm of comets) {
      const sat = g.sats[cm.net]!
      const ta = -Math.PI / 2 + ((cm.tick + 0.5) / TICKS) * TAU
      const delta = wrap(ta - sat.a)
      const end = ring + tick * 1.05
      const at = (t: number) => {
        const u = Math.max(0, Math.min(1, t))
        const ang = sat.a + delta * (1 - (1 - u) * (1 - u))
        const rad = orbit + (end - orbit) * u * u
        return { x: c.x + Math.cos(ang) * rad, y: c.y + Math.sin(ang) * rad }
      }
      const head = at(cm.t)
      const tail = at(cm.t - 0.11)
      const col = colors.lanes[cm.net]!
      const dim = focus >= 0 && focus !== cm.net ? 0.25 : 1
      const fade = cm.t > 0.85 ? (1 - cm.t) / 0.15 : 1
      const grad = ctx!.createLinearGradient(tail.x, tail.y, head.x, head.y)
      grad.addColorStop(0, rgba(col, 0))
      grad.addColorStop(1, rgba(col, 0.95 * dim * fade))
      ctx!.strokeStyle = grad
      ctx!.lineWidth = 2
      ctx!.beginPath()
      ctx!.moveTo(tail.x, tail.y)
      const mid = at(cm.t - 0.055)
      ctx!.quadraticCurveTo(mid.x, mid.y, head.x, head.y)
      ctx!.stroke()
      ctx!.fillStyle = rgba(col, dim * fade)
      ctx!.beginPath()
      ctx!.arc(head.x, head.y, 1.6, 0, TAU)
      ctx!.fill()
    }
    ctx!.restore()

    // Satellites: a core dot and a 12-dot orb spinning at the network's speed.
    // Turned off, the orb drifts apart and greys out, and the core goes hollow.
    const orb = Math.max(9, g.s * 0.024)
    for (let i = 0; i < n; i++) {
      const sat = g.sats[i]!
      const sp = spread[i]!
      const col = mix(colors.lanes[i]!, colors.mute, sp)
      const dim = focus >= 0 && focus !== i ? 0.35 : 1
      for (let d = 0; d < 12; d++) {
        const a = spin[i]! + (d / 12) * TAU
        const r = orb * (1 + sp * 1.1) + (sp ? Math.sin(d * 1.7) * sp * orb * 0.5 : 0)
        ctx!.fillStyle = rgba(col, (0.85 - sp * 0.55) * dim * (0.5 + 0.5 * ((d % 3) / 2)))
        ctx!.beginPath()
        ctx!.arc(sat.x + Math.cos(a) * r, sat.y + Math.sin(a) * r, 1.5, 0, TAU)
        ctx!.fill()
      }
      ctx!.beginPath()
      ctx!.arc(sat.x, sat.y, 4.5, 0, TAU)
      if (sp > 0.5) {
        ctx!.strokeStyle = rgba(col, 0.8 * dim)
        ctx!.lineWidth = 1.5
        ctx!.stroke()
      } else {
        ctx!.fillStyle = rgba(col, dim)
        ctx!.fill()
      }
    }
  }

  function setOn(i: number, value: boolean) {
    if (i < 0 || i >= n) return
    on[i] = value
    emitState()
  }

  refreshColors()
  resize()
  startFile(0.34)

  return {
    step,
    render,
    resize,
    refreshColors,
    geometry,
    setOn,
    isOn: (i: number) => on[i] ?? false,
    setFocus: (i: number) => (focus = i),
    /** For a still picture: let the rates settle on their typical values. */
    settle() {
      for (let i = 0; i < n; i++) {
        rates[i] = on[i] ? nets[i]!.base : 0
        spread[i] = on[i] ? 0 : 1
      }
      emitState()
    },
  }
}

export type Core = NonNullable<ReturnType<typeof createCore>>
