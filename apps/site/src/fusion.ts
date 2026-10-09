// The hero: three networks as lanes of light that braid into one core, and one
// beam out of it filling a file. Sample data, labelled as such on the page.
// Pauses off screen and in background tabs; a still frame under reduced motion.

export interface Lane {
  name: string
  /** CSS colour. */
  color: string
  /** Typical MB/s. */
  base: number
}

interface Particle {
  lane: number // -1: the output beam
  t: number
  speed: number
  size: number
}

const FILE_MB = 4.7 * 1024

export function startFusion(
  root: HTMLElement,
  lanes: Lane[],
  onRates: (rates: number[], total: number, progress: number, left: number) => void,
) {
  const canvas = root.querySelector('canvas')
  const ctx = canvas?.getContext('2d')
  if (!canvas || !ctx) return
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches
  let w = 0
  let h = 0
  let dpr = 1
  const particles: Particle[] = []
  let time = 0
  let pulse = 0
  let progress = 0.18
  let holdDone = 0
  let rates = lanes.map((l) => l.base)
  let spawn = lanes.map(() => 0)
  let beamSpawn = 0
  let visible = true
  let last = performance.now()
  let sinceText = 1

  const resize = () => {
    const r = canvas.getBoundingClientRect()
    dpr = Math.min(2, window.devicePixelRatio || 1)
    w = r.width
    h = r.height
    canvas.width = Math.round(w * dpr)
    canvas.height = Math.round(h * dpr)
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  }

  // Geometry, in CSS pixels, recomputed from the size every frame (cheap).
  // Wide: lanes flow left to right. Narrow (phones): they fall from three chips at
  // the top into the core, and the beam runs down into the file.
  const geo = () => {
    const vertical = w < 520
    if (vertical) {
      const cx = w * 0.5
      const cy = h * 0.56
      const r = w * 0.17
      return {
        vertical,
        sx: 0,
        sy: h * 0.17,
        xs: [0.17, 0.5, 0.83].map((f) => w * f),
        cx,
        cy,
        r,
        ys: [] as number[],
      }
    }
    const sx = w * 0.27
    const cx = w * 0.66
    const cy = h * 0.5
    const r = Math.min(w, h) * 0.16
    const ys = [0.22, 0.5, 0.78].map((f) => h * f)
    return { vertical, sx, sy: 0, xs: [] as number[], cx, cy, r, ys }
  }

  /** A point on lane `i`'s curve at t (0..1). */
  const along = (i: number, t: number, g: ReturnType<typeof geo>) => {
    if (g.vertical) {
      const x0 = g.xs[i] ?? g.cx
      const y0 = g.sy
      const y3 = g.cy - g.r * 0.92
      const mid = y0 + (y3 - y0) * 0.5
      const u = 1 - t
      const x = u * u * u * x0 + 3 * u * u * t * x0 + 3 * u * t * t * g.cx + t * t * t * g.cx
      const y = u * u * u * y0 + 3 * u * u * t * mid + 3 * u * t * t * mid + t * t * t * y3
      return { x, y }
    }
    const y0 = g.ys[i] ?? g.cy
    const x0 = g.sx
    const x3 = g.cx - g.r * 0.92
    const c1x = x0 + (x3 - x0) * 0.45
    const c2x = x0 + (x3 - x0) * 0.6
    const u = 1 - t
    const x = u * u * u * x0 + 3 * u * u * t * c1x + 3 * u * t * t * c2x + t * t * t * x3
    const y = u * u * u * y0 + 3 * u * u * t * y0 + 3 * u * t * t * g.cy + t * t * t * g.cy
    return { x, y }
  }

  const beamAt = (t: number, g: ReturnType<typeof geo>) =>
    g.vertical
      ? { x: g.cx, y: g.cy + g.r * 0.9 + (h - (g.cy + g.r * 0.9)) * t }
      : { x: g.cx + g.r * 0.9 + (w - (g.cx + g.r * 0.9)) * t, y: g.cy }

  const drawLanes = (g: ReturnType<typeof geo>) => {
    lanes.forEach((lane, i) => {
      ctx.save()
      ctx.strokeStyle = lane.color
      ctx.globalAlpha = 0.18
      ctx.lineWidth = 2
      ctx.shadowColor = lane.color
      ctx.shadowBlur = 16
      ctx.beginPath()
      for (let k = 0; k <= 40; k++) {
        const p = along(i, k / 40, g)
        if (k === 0) ctx.moveTo(p.x, p.y)
        else ctx.lineTo(p.x, p.y)
      }
      ctx.stroke()
      // The source node.
      const s = along(i, 0, g)
      ctx.globalAlpha = 0.9
      ctx.fillStyle = lane.color
      ctx.beginPath()
      ctx.arc(s.x, s.y, 3.5, 0, Math.PI * 2)
      ctx.fill()
      ctx.restore()
    })
    // The output beam.
    const a = beamAt(0, g)
    const end = beamAt(1, g)
    const grad = ctx.createLinearGradient(a.x, a.y, end.x, end.y)
    grad.addColorStop(0, 'oklch(0.92 0.09 72 / 0.55)')
    grad.addColorStop(1, 'oklch(0.74 0.17 50 / 0)')
    ctx.save()
    ctx.strokeStyle = grad
    ctx.lineWidth = 3
    ctx.shadowColor = 'oklch(0.86 0.13 70)'
    ctx.shadowBlur = 20
    ctx.beginPath()
    ctx.moveTo(a.x, a.y)
    ctx.lineTo(end.x, end.y)
    ctx.stroke()
    ctx.restore()
  }

  const drawCore = (g: ReturnType<typeof geo>, total: number) => {
    const glow = g.r * (2.4 + pulse * 0.6)
    const halo = ctx.createRadialGradient(g.cx, g.cy, 0, g.cx, g.cy, glow)
    halo.addColorStop(0, 'oklch(0.97 0.04 80 / 0.55)')
    halo.addColorStop(0.25, 'oklch(0.86 0.13 70 / 0.28)')
    halo.addColorStop(0.6, 'oklch(0.74 0.17 50 / 0.08)')
    halo.addColorStop(1, 'oklch(0.74 0.17 50 / 0)')
    ctx.fillStyle = halo
    ctx.beginPath()
    ctx.arc(g.cx, g.cy, glow, 0, Math.PI * 2)
    ctx.fill()
    // Rings turning with the speed.
    ctx.save()
    ctx.translate(g.cx, g.cy)
    for (let k = 0; k < 3; k++) {
      ctx.rotate((time * (0.12 + total / 900) * (k % 2 ? -1 : 1)) % (Math.PI * 2))
      ctx.strokeStyle = `oklch(0.9 0.08 70 / ${0.35 - k * 0.09})`
      ctx.lineWidth = 1.2
      ctx.setLineDash([2 + k * 3, 6 + k * 4])
      ctx.beginPath()
      ctx.arc(0, 0, g.r * (1 + k * 0.22), 0, Math.PI * 2)
      ctx.stroke()
    }
    ctx.restore()
    // The dark disc the number sits on.
    ctx.fillStyle = 'oklch(0.14 0.012 262 / 0.92)'
    ctx.beginPath()
    ctx.arc(g.cx, g.cy, g.r * 0.86, 0, Math.PI * 2)
    ctx.fill()
    ctx.strokeStyle = `oklch(0.86 0.13 70 / ${0.5 + pulse * 0.4})`
    ctx.lineWidth = 1.5
    ctx.stroke()
  }

  const drawParticles = (g: ReturnType<typeof geo>) => {
    ctx.save()
    ctx.globalCompositeOperation = 'lighter'
    for (const p of particles) {
      const color = p.lane < 0 ? 'oklch(0.94 0.08 75)' : (lanes[p.lane]?.color ?? '#fff')
      const at = (t: number) => (p.lane < 0 ? beamAt(t, g) : along(p.lane, t, g))
      const head = at(p.t)
      const tail = at(Math.max(0, p.t - 0.05 * (0.6 + p.speed)))
      const grad = ctx.createLinearGradient(tail.x, tail.y, head.x, head.y)
      grad.addColorStop(0, 'transparent')
      grad.addColorStop(1, color)
      ctx.strokeStyle = grad
      ctx.lineWidth = p.size
      ctx.lineCap = 'round'
      ctx.beginPath()
      ctx.moveTo(tail.x, tail.y)
      ctx.lineTo(head.x, head.y)
      ctx.stroke()
    }
    ctx.restore()
  }

  const step = (dt: number) => {
    time += dt
    rates = lanes.map(
      (l, i) =>
        l.base *
        (1 + 0.09 * Math.sin(time * (0.5 + i * 0.23) + i * 1.7) + 0.04 * Math.sin(time * 2.1 + i)),
    )
    const total = rates.reduce((a, b) => a + b, 0)
    // Spawn in proportion to each network's speed.
    rates.forEach((r, i) => {
      spawn[i]! += dt * r * 0.42
      while (spawn[i]! >= 1) {
        spawn[i]! -= 1
        particles.push({ lane: i, t: 0, speed: 0.38 + r / 90, size: 1.6 + Math.random() * 1.2 })
      }
    })
    beamSpawn += dt * total * 0.32
    while (beamSpawn >= 1) {
      beamSpawn -= 1
      particles.push({
        lane: -1,
        t: 0,
        speed: 0.9 + Math.random() * 0.4,
        size: 2 + Math.random() * 1.4,
      })
    }
    for (let k = particles.length - 1; k >= 0; k--) {
      const p = particles[k]!
      p.t += dt * p.speed
      if (p.t >= 1) {
        if (p.lane >= 0) pulse = Math.min(1, pulse + 0.035)
        particles.splice(k, 1)
      }
    }
    pulse *= Math.pow(0.08, dt)
    // The file fills at the combined speed, then starts again.
    if (holdDone > 0) {
      holdDone -= dt
      if (holdDone <= 0) progress = 0
    } else {
      progress = Math.min(1, progress + (total * dt) / FILE_MB)
      if (progress >= 1) holdDone = 2
    }
    sinceText += dt
    if (sinceText >= 0.25) {
      sinceText = 0
      onRates(rates, total, progress, holdDone > 0 ? 0 : ((1 - progress) * FILE_MB) / total)
    }
    return total
  }

  let lastR = 0
  const render = (total: number) => {
    ctx.clearRect(0, 0, w, h)
    const g = geo()
    // The page places the "× one network" chip just below the disc.
    if (Math.abs(g.r - lastR) > 0.5) {
      lastR = g.r
      root.style.setProperty('--core-r', `${g.r.toFixed(1)}px`)
    }
    drawLanes(g)
    drawParticles(g)
    drawCore(g, total)
  }

  const frame = (now: number) => {
    const dt = Math.min(0.05, (now - last) / 1000)
    last = now
    if (visible && !document.hidden) render(step(dt))
    requestAnimationFrame(frame)
  }

  resize()
  new ResizeObserver(() => {
    resize()
    if (reduced) render(step(0))
  }).observe(canvas)
  new IntersectionObserver((e) => (visible = e.some((x) => x.isIntersecting))).observe(root)

  if (reduced) {
    // A still picture: particles spread evenly along each lane.
    lanes.forEach((l, i) => {
      for (let k = 0; k < 10; k++)
        particles.push({ lane: i, t: (k + 0.5) / 10, speed: 0.4 + l.base / 90, size: 2 })
    })
    for (let k = 0; k < 8; k++) particles.push({ lane: -1, t: (k + 0.5) / 8, speed: 1, size: 2.4 })
    const total = rates.reduce((a, b) => a + b, 0)
    render(total)
    onRates(rates, total, 0.42, ((1 - 0.42) * FILE_MB) / total)
    return
  }
  requestAnimationFrame(frame)
}
