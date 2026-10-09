import { useEffect, useRef } from 'react'

/**
 * The torrent's pieces as one strip that fills in as data arrives: one canvas
 * (cheap at any size); slices that just completed flash briefly.
 */
export function PiecesMap({ cells, label }: { cells: number[]; label: string }) {
  const canvas = useRef<HTMLCanvasElement>(null)
  const prev = useRef<number[]>([])
  const glow = useRef<Map<number, number>>(new Map())
  useEffect(() => {
    const c = canvas.current
    if (!c || !cells.length) return
    const now = performance.now()
    cells.forEach((v, i) => {
      if (v === 100 && (prev.current[i] ?? 100) < 100) glow.current.set(i, now)
    })
    prev.current = cells
    const css = getComputedStyle(c)
    const color = (name: string) => css.getPropertyValue(name).trim() || '#888'
    const [done, part, empty] = [color('--fuse'), color('--fuse-soft'), color('--hairline')]
    const still = matchMedia('(prefers-reduced-motion: reduce)').matches
    let raf = 0
    const draw = () => {
      const dpr = devicePixelRatio || 1
      const w = c.clientWidth
      const h = 10
      if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
        c.width = Math.round(w * dpr)
        c.height = Math.round(h * dpr)
      }
      const g = c.getContext('2d')
      if (!g) return
      g.setTransform(dpr, 0, 0, dpr, 0, 0)
      g.clearRect(0, 0, w, h)
      // The track, then each slice of the file in one strip.
      g.fillStyle = empty
      g.beginPath()
      g.roundRect(0, 0, w, h, 5)
      g.fill()
      g.save()
      g.clip()
      const slice = w / cells.length
      const gap = 0
      const t = performance.now()
      let glowing = false
      cells.forEach((v, i) => {
        if (v === 0) return
        const x = i * slice
        const since = glow.current.get(i)
        let lift = 0
        if (since !== undefined && !still) {
          const k = (t - since) / 900
          if (k < 1) {
            glowing = true
            lift = 1 - k
          } else glow.current.delete(i)
        }
        g.fillStyle = v >= 100 ? done : part
        g.globalAlpha = v >= 100 ? 1 : 0.9
        g.fillRect(x, 0, Math.max(1, slice - gap), h)
        if (lift > 0) {
          g.globalAlpha = 0.55 * lift
          g.fillStyle = '#fff'
          g.fillRect(x, 0, Math.max(1, slice - gap), h)
        }
      })
      g.globalAlpha = 1
      g.restore()
      if (glowing) raf = requestAnimationFrame(draw)
    }
    draw()
    const ro = new ResizeObserver(() => draw())
    ro.observe(c)
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
    }
  }, [cells])
  return <canvas ref={canvas} className="pieces" role="img" aria-label={label} />
}
