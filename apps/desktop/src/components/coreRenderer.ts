// Fuse Core renderer (DESIGN-SYSTEM.md §7), driven by real engine snapshots.
// - The file is a ring of block ticks. Each tick fills as its bytes arrive and
//   takes the colour of the network that fetched most of it.
// - Each network is a satellite on a dotted orbit, with an orb spinning at its speed,
//   firing comets that spiral into the tick it is fetching (polar paths, so they never
//   cross the centre readout).
// - On completion a Fuse sweep "ignites" the ring once.
// Thin, crisp strokes only. Plain canvas; React only mounts it.

type RGB = [number, number, number]
type Pt = { x: number; y: number }

export interface CoreNet {
  lane: string
  rate: number
  dead: boolean
}

export interface CoreData {
  /** Per tick, 0..1. */
  fill: Float32Array
  /** Per tick, network index or -1. */
  owner: Int8Array
  /** Per tick, network index fetching it or -1. */
  inflight: Int8Array
  nets: CoreNet[]
  complete: boolean
}

const MAX_COMETS = 140

interface Comet {
  lane: number
  tick: number
  t: number
  speed: number
  bend: number
}

export function geometry(w: number, h: number, n: number) {
  const s = Math.min(w, h)
  const c = { x: w / 2, y: h / 2 }
  const ring = s * 0.3
  const tickLen = Math.max(5, s * 0.032)
  const orbit = s * 0.41
  const sats: Pt[] = Array.from({ length: Math.max(1, n) }, (_, i) => {
    const a = ((-150 + (i * 360) / Math.max(1, n)) * Math.PI) / 180
    return { x: c.x + Math.cos(a) * orbit, y: c.y + Math.sin(a) * orbit }
  })
  return { s, c, ring, tickLen, orbit, sats, small: s < 220 }
}

function resolve(cssVar: string): RGB {
  const probe = document.createElement('canvas')
  probe.width = probe.height = 1
  const c = probe.getContext('2d', { willReadFrequently: true })
  const value = getComputedStyle(document.documentElement).getPropertyValue(cssVar).trim() || '#888'
  if (!c) return [136, 136, 136]
  c.fillStyle = value
  c.fillRect(0, 0, 1, 1)
  const d = c.getImageData(0, 0, 1, 1).data
  return [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0]
}
const rgba = (c: RGB, a: number) => `rgba(${c[0] | 0},${c[1] | 0},${c[2] | 0},${a})`
const mix = (a: RGB, b: RGB, k: number): RGB => [
  a[0] + (b[0] - a[0]) * k,
  a[1] + (b[1] - a[1]) * k,
  a[2] + (b[2] - a[2]) * k,
]
const ease = (v: number, to: number, rate: number, dt: number) =>
  v + (to - v) * (1 - Math.exp(-rate * dt))

export function createFuseCore(canvas: HTMLCanvasElement, ticks: number) {
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('Canvas 2D unavailable')
  let w = 1
  let h = 1
  let dark = true
  let reduced = false
  let lanes: string[] = []
  let colors = {
    lanes: [] as RGB[],
    fuse: [0, 0, 0] as RGB,
    mute: [0, 0, 0] as RGB,
    ink: [0, 0, 0] as RGB,
    white: [255, 255, 255] as RGB,
  }
  let data: CoreData | null = null
  let shown = new Float32Array(ticks)
  let doneAt = new Float32Array(ticks).fill(-1)
  let comets: Comet[] = []
  let emit: number[] = []
  let spin: number[] = []
  let spread: number[] = []
  let clock = 0
  let ignite = -1
  let wasComplete: boolean | null = null

  function refreshColors() {
    colors = {
      lanes: lanes.map((l) => resolve(`--lane-${l}`)),
      fuse: resolve('--fuse'),
      mute: resolve('--mute'),
      ink: resolve('--ink'),
      white: [255, 255, 255],
    }
    const cv = resolve('--canvas')
    dark = cv[0] + cv[1] + cv[2] < 300
  }

  function resize() {
    const r = canvas.getBoundingClientRect()
    const dpr = Math.min(devicePixelRatio || 1, 2)
    w = Math.max(1, r.width)
    h = Math.max(1, r.height)
    canvas.width = Math.round(w * dpr)
    canvas.height = Math.round(h * dpr)
    ctx!.setTransform(dpr, 0, 0, dpr, 0, 0)
  }

  function setData(d: CoreData) {
    const laneKey = d.nets.map((n) => n.lane).join()
    if (laneKey !== lanes.join()) {
      lanes = d.nets.map((n) => n.lane)
      emit = lanes.map(() => 0)
      spin = lanes.map(() => 0)
      spread = lanes.map(() => 0)
      comets = []
      refreshColors()
    }
    if (wasComplete === false && d.complete && !reduced) ignite = 0
    if (wasComplete === null && d.complete) shown = Float32Array.from(d.fill)
    wasComplete = d.complete
    data = d
  }

  const tickAngle = (i: number) => -Math.PI / 2 + ((i + 0.5) / ticks) * Math.PI * 2

  function targetFor(lane: number): number {
    if (!data) return -1
    const mine: number[] = []
    for (let i = 0; i < ticks; i++) if (data.inflight[i] === lane) mine.push(i)
    if (mine.length) return mine[Math.floor(Math.random() * mine.length)]!
    return -1
  }

  function step(dt: number) {
    if (!data) return
    clock += dt
    if (ignite >= 0) ignite += dt
    for (let i = 0; i < ticks; i++) {
      const before = shown[i]!
      shown[i] = reduced ? data.fill[i]! : ease(before, data.fill[i]!, 9, dt)
      if (data.fill[i]! >= 1 && before < 0.999) {
        shown[i] = 1
        doneAt[i] = clock
      }
    }
    const maxRate = Math.max(1, ...data.nets.map((n) => n.rate))
    data.nets.forEach((n, i) => {
      spread[i] = ease(spread[i] ?? 0, n.dead ? 1 : 0, 3, dt)
      spin[i] = (spin[i] ?? 0) + dt * (n.dead ? 0 : 0.4 + 2.6 * (n.rate / maxRate))
      if (n.dead || reduced || data!.complete) return
      emit[i] = (emit[i] ?? 0) + dt * (4 + 20 * (n.rate / maxRate))
      while (emit[i]! >= 1 && comets.length < MAX_COMETS) {
        emit[i]! -= 1
        const tick = targetFor(i)
        if (tick < 0) {
          emit[i] = 0
          break
        }
        comets.push({
          lane: i,
          tick,
          t: 0,
          speed: 0.9 + 0.9 * (n.rate / maxRate),
          bend: 0.18 + Math.random() * 0.14,
        })
      }
    })
    for (const c of comets) c.t += c.speed * dt
    comets = comets.filter((c) => c.t < 1 && !data!.nets[c.lane]?.dead)
  }

  function cometPoint(c: Comet, t: number, g: ReturnType<typeof geometry>): Pt {
    const sat = g.sats[c.lane] ?? g.c
    const a0 = Math.atan2(sat.y - g.c.y, sat.x - g.c.x)
    let da = tickAngle(c.tick) - a0
    while (da > Math.PI) da -= Math.PI * 2
    while (da < -Math.PI) da += Math.PI * 2
    const sweep = 1 - Math.pow(1 - t, 1.6 + c.bend * 2)
    const a = a0 + da * sweep
    const r1 = g.ring + g.tickLen * 0.7
    const r = g.orbit + (r1 - g.orbit) * Math.pow(t, 1.25)
    return { x: g.c.x + Math.cos(a) * r, y: g.c.y + Math.sin(a) * r }
  }

  function render() {
    const c = ctx!
    c.clearRect(0, 0, w, h)
    if (!data) return
    const g = geometry(w, h, data.nets.length)
    c.lineCap = 'round'

    // Orbit guide: a fine dotted circle.
    c.setLineDash([1, 7])
    c.lineWidth = 1
    c.strokeStyle = rgba(colors.mute, dark ? 0.35 : 0.4)
    c.beginPath()
    c.arc(g.c.x, g.c.y, g.orbit, 0, Math.PI * 2)
    c.stroke()
    c.setLineDash([])

    // Block ticks: empty = hairline, in flight = growing radially, done = network colour.
    const inner = g.ring - g.tickLen / 2
    c.lineWidth = g.small ? 1.6 : 2.2
    for (let i = 0; i < ticks; i++) {
      const a = tickAngle(i)
      const cos = Math.cos(a)
      const sin = Math.sin(a)
      const f = shown[i]!
      let len = g.tickLen
      if (f >= 0.999) {
        const owner = data.owner[i]!
        const col =
          owner >= 0
            ? (colors.lanes[owner] ?? colors.mute)
            : data.complete
              ? colors.fuse
              : colors.mute
        const flash = reduced || doneAt[i]! < 0 ? 0 : Math.max(0, 1 - (clock - doneAt[i]!) / 0.45)
        c.strokeStyle = rgba(
          mix(col, dark ? colors.white : colors.ink, flash * 0.6),
          owner >= 0 || data.complete ? 1 : 0.7,
        )
        len = g.tickLen * (1 + flash * 0.35)
      } else if (f > 0.004) {
        const holder = data.inflight[i]!
        const col = holder >= 0 ? (colors.lanes[holder] ?? colors.mute) : colors.mute
        c.strokeStyle = rgba(col, holder >= 0 ? 0.9 : 0.5)
        len = g.tickLen * Math.max(0.14, f)
      } else {
        c.strokeStyle = rgba(colors.mute, dark ? 0.22 : 0.16)
      }
      c.beginPath()
      c.moveTo(g.c.x + cos * inner, g.c.y + sin * inner)
      c.lineTo(g.c.x + cos * (inner + len), g.c.y + sin * (inner + len))
      c.stroke()
    }

    // Progress arc in Fuse, just outside the ticks.
    let sum = 0
    for (let i = 0; i < ticks; i++) sum += Math.min(1, shown[i]!)
    const progress = data.complete ? 1 : sum / ticks
    const arcR = g.ring + g.tickLen * 0.5 + (g.small ? 5 : 9)
    c.lineWidth = g.small ? 1.5 : 2
    c.strokeStyle = rgba(colors.mute, dark ? 0.18 : 0.2)
    c.beginPath()
    c.arc(g.c.x, g.c.y, arcR, 0, Math.PI * 2)
    c.stroke()
    if (progress > 0) {
      c.strokeStyle = rgba(colors.fuse, 1)
      c.beginPath()
      c.arc(g.c.x, g.c.y, arcR, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * progress)
      c.stroke()
    }

    // Comets: head plus a fading tail, curving into their tick.
    for (const k of comets) {
      const col = colors.lanes[k.lane] ?? colors.mute
      const trail = 7
      for (let s = trail; s >= 0; s--) {
        const p = cometPoint(k, Math.max(0, k.t - s * 0.022), g)
        const a = (1 - s / (trail + 1)) * (s === 0 ? 1 : 0.55)
        c.fillStyle = rgba(s === 0 && dark ? mix(col, colors.white, 0.35) : col, a)
        c.beginPath()
        c.arc(p.x, p.y, (g.small ? 1 : 1.5) * (s === 0 ? 1.3 : 1 - s / (trail + 2)), 0, Math.PI * 2)
        c.fill()
      }
    }

    // Satellites: a core dot and a 12-dot orb spinning at the network's speed.
    data.nets.forEach((n, i) => {
      const p = g.sats[i]
      const col = colors.lanes[i]
      if (!p || !col) return
      const sp = spread[i] ?? 0
      const r = (g.small ? 6 : 10) + sp * (g.small ? 5 : 8)
      for (let d = 0; d < 12; d++) {
        const a = (spin[i] ?? 0) + (d / 12) * Math.PI * 2
        const jitter = sp * Math.sin(d * 7.3) * 3
        c.fillStyle = rgba(sp > 0.5 ? colors.mute : col, (0.35 + 0.65 * (d / 12)) * (1 - sp * 0.55))
        c.beginPath()
        c.arc(
          p.x + Math.cos(a) * (r + jitter),
          p.y + Math.sin(a) * (r + jitter),
          g.small ? 0.9 : 1.3,
          0,
          Math.PI * 2,
        )
        c.fill()
      }
      c.beginPath()
      c.arc(p.x, p.y, g.small ? 2.5 : 3.5, 0, Math.PI * 2)
      if (n.dead) {
        c.lineWidth = 1.2
        c.strokeStyle = rgba(colors.mute, 0.9)
        c.stroke()
      } else {
        c.fillStyle = rgba(col, 1)
        c.fill()
      }
    })

    // Ignition: one Fuse sweep around the ring, then calm.
    if (ignite >= 0 && ignite < 1.8) {
      const k = Math.min(1, ignite / 0.6)
      const fade = ignite < 0.6 ? 1 : Math.max(0, 1 - (ignite - 0.6) / 1.2)
      c.strokeStyle = rgba(colors.fuse, 0.9 * fade)
      c.lineWidth = (g.small ? 3 : 5) * fade + 1
      c.beginPath()
      c.arc(g.c.x, g.c.y, g.ring, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * k)
      c.stroke()
    }
  }

  return {
    resize,
    refreshColors,
    setData,
    step,
    render,
    setReduced: (v: boolean) => {
      reduced = v
      if (v) comets = []
    },
    comets: () => comets.length,
  }
}
