// Throughput stream: the last 15 to 60 seconds as a centred silhouette, one layer per
// network. Thickness is the combined speed. Canvas, crisp 1px edges.
import { resolveVar, type RGB } from '../lib/color'
import { useEffect, useRef } from 'react'
import type { History } from '../lib/store'
import type { Lane } from '../lib/lanes'

const RATE = 5

const rgba = (c: RGB, a: number) => `rgba(${c[0]},${c[1]},${c[2]},${a})`

function smooth(c: CanvasRenderingContext2D, pts: { x: number; y: number }[], move: boolean) {
  const first = pts[0]
  if (!first) return
  if (move) c.moveTo(first.x, first.y)
  else c.lineTo(first.x, first.y)
  for (let i = 1; i < pts.length - 1; i++) {
    const p = pts[i]!
    const q = pts[i + 1]!
    c.quadraticCurveTo(p.x, p.y, (p.x + q.x) / 2, (p.y + q.y) / 2)
  }
  const last = pts[pts.length - 1]!
  c.lineTo(last.x, last.y)
}

export function Stream({ history, lanes }: { history: History | undefined; lanes: Lane[] }) {
  const ref = useRef<HTMLCanvasElement>(null)
  const latest = useRef({ history, lanes })
  latest.current = { history, lanes }

  useEffect(() => {
    const canvas = ref.current
    const ctx = canvas?.getContext('2d')
    if (!canvas || !ctx) return
    let w = 1
    let h = 1
    let theme = ''
    let colors: RGB[] = []
    let mute: RGB = [128, 128, 128]
    let dark = true
    const fit = () => {
      const r = canvas.getBoundingClientRect()
      const dpr = Math.min(devicePixelRatio || 1, 2)
      w = Math.max(1, r.width)
      h = Math.max(1, r.height)
      canvas.width = Math.round(w * dpr)
      canvas.height = Math.round(h * dpr)
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      draw()
    }
    const draw = () => {
      const { history, lanes } = latest.current
      const key = `${document.documentElement.dataset.theme ?? ''}|${matchMedia('(prefers-color-scheme: dark)').matches}|${lanes.join()}`
      if (key !== theme) {
        theme = key
        colors = lanes.map((l) => resolveVar(`--lane-${l}`))
        mute = resolveVar('--mute')
        const cv = resolveVar('--canvas')
        dark = cv[0] + cv[1] + cv[2] < 300
      }
      ctx.clearRect(0, 0, w, h)
      const mid = h / 2
      ctx.strokeStyle = rgba(mute, dark ? 0.25 : 0.3)
      ctx.setLineDash([2, 6])
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(0, mid)
      ctx.lineTo(w, mid)
      ctx.stroke()
      ctx.setLineDash([])
      const rows = history?.rates ?? []
      if (rows.length < 2) return
      const peak = Math.max(1, ...rows.map((r) => r.reduce((a, b) => a + b, 0)))
      const scale = (h * 0.86) / peak
      const span = Math.min(60 * RATE, Math.max(15 * RATE, rows.length)) - 1
      const x = (k: number) => w - ((rows.length - 1 - k) / span) * w
      const bases = rows.map((r) => mid - (r.reduce((a, b) => a + b, 0) * scale) / 2)
      const n = rows[rows.length - 1]?.length ?? 0
      for (let lane = 0; lane < n; lane++) {
        const top: { x: number; y: number }[] = []
        const bottom: { x: number; y: number }[] = []
        rows.forEach((r, k) => {
          let y0 = bases[k]!
          for (let j = 0; j < lane; j++) y0 += (r[j] ?? 0) * scale
          top.push({ x: x(k), y: y0 })
          bottom.push({ x: x(k), y: y0 + (r[lane] ?? 0) * scale })
        })
        const col = colors[lane] ?? mute
        ctx.beginPath()
        smooth(ctx, top, true)
        smooth(ctx, bottom.reverse(), false)
        ctx.closePath()
        ctx.fillStyle = rgba(col, dark ? 0.62 : 0.55)
        ctx.fill()
        ctx.beginPath()
        smooth(ctx, top, true)
        ctx.strokeStyle = rgba(col, 0.95)
        ctx.stroke()
      }
    }
    const ro = new ResizeObserver(fit)
    ro.observe(canvas)
    const timer = setInterval(draw, 200)
    return () => {
      ro.disconnect()
      clearInterval(timer)
    }
  }, [])

  const seconds = Math.min(60, Math.max(15, Math.round((history?.rates.length ?? 0) / RATE)))
  return (
    <div className="stream">
      <canvas ref={ref} aria-hidden="true" />
      <div className="stream-axis">
        <span>{seconds} s ago</span>
        <span>now</span>
      </div>
    </div>
  )
}
