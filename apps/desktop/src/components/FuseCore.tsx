import { useEffect, useMemo, useRef, useState } from 'react'
import { createFuseCore, geometry, type CoreData } from './coreRenderer'
import { assignLanes, netTitle } from '../lib/lanes'
import { rateText } from '../lib/format'
import type { JobView, Live } from '../lib/types'

const TICKS = 180

/** Snapshot (or, without one, the saved progress) as ring data. */
function toCore(job: JobView, live: Live | undefined): CoreData {
  const complete = job.status === 'completed'
  // A finished job keeps its last snapshot's colours: who fetched which part.
  if (live && live.ticks.length >= 3) {
    const n = Math.floor(live.ticks.length / 3)
    const fill = new Float32Array(TICKS)
    const owner = new Int8Array(TICKS).fill(-1)
    const inflight = new Int8Array(TICKS).fill(-1)
    // Map the engine's ticks (≤ 180) onto the ring's 180.
    for (let i = 0; i < TICKS; i++) {
      const k = Math.min(n - 1, Math.floor((i * n) / TICKS))
      fill[i] = complete ? 1 : (live.ticks[k * 3] ?? 0) / 100
      owner[i] = (live.ticks[k * 3 + 1] ?? 0) - 1
      inflight[i] = complete ? -1 : (live.ticks[k * 3 + 2] ?? 0) - 1
    }
    const lanes = assignLanes(live.networks)
    return {
      fill,
      owner,
      inflight,
      nets: live.networks.map((net, i) => ({
        lane: lanes[i] ?? 'steel',
        rate: complete ? 0 : net.rate,
        dead: complete ? false : net.dead,
      })),
      complete,
    }
  }
  const frac = complete ? 1 : job.total ? Math.min(1, job.written / job.total) : 0
  const fill = new Float32Array(TICKS)
  const full = Math.floor(frac * TICKS)
  for (let i = 0; i < full; i++) fill[i] = 1
  const lanes = live ? assignLanes(live.networks) : []
  return {
    fill,
    owner: new Int8Array(TICKS).fill(-1),
    inflight: new Int8Array(TICKS).fill(-1),
    nets: (live?.networks ?? []).map((net, i) => ({
      lane: lanes[i] ?? 'steel',
      rate: 0,
      dead: true,
    })),
    complete,
  }
}

function useReducedMotion() {
  const [reduced, setReduced] = useState(
    () => matchMedia('(prefers-reduced-motion: reduce)').matches,
  )
  useEffect(() => {
    const m = matchMedia('(prefers-reduced-motion: reduce)')
    const on = () => setReduced(m.matches)
    m.addEventListener('change', on)
    return () => m.removeEventListener('change', on)
  }, [])
  return reduced
}

export function FuseCore({
  job,
  live,
  center,
}: {
  job: JobView
  live: Live | undefined
  center: React.ReactNode
}) {
  const canvas = useRef<HTMLCanvasElement>(null)
  const box = useRef<HTMLDivElement>(null)
  const core = useRef<ReturnType<typeof createFuseCore> | null>(null)
  const [size, setSize] = useState({ w: 0, h: 0, room: 0 })
  const reduced = useReducedMotion()
  const data = useMemo(() => toCore(job, live), [job, live])
  const running = job.status === 'running'

  useEffect(() => {
    if (!canvas.current) return
    const c = createFuseCore(canvas.current, TICKS)
    core.current = c
    const ro = new ResizeObserver(() => {
      c.resize()
      const r = canvas.current?.getBoundingClientRect()
      // Room beside the ring for satellite labels (they sit outside its box).
      const room =
        (box.current?.parentElement?.getBoundingClientRect().width ?? 0) - (r?.width ?? 0)
      if (r) setSize({ w: r.width, h: r.height, room })
    })
    ro.observe(canvas.current)
    const parent = box.current?.parentElement
    if (parent) ro.observe(parent)
    // Colours follow the theme.
    const mo = new MutationObserver(() => c.refreshColors())
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
    const scheme = matchMedia('(prefers-color-scheme: dark)')
    const onScheme = () => c.refreshColors()
    scheme.addEventListener('change', onScheme)
    let raf = 0
    let last = performance.now()
    const frame = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000)
      last = now
      c.step(dt)
      c.render()
      raf = requestAnimationFrame(frame)
    }
    raf = requestAnimationFrame(frame)
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      mo.disconnect()
      scheme.removeEventListener('change', onScheme)
      core.current = null
    }
  }, [])

  useEffect(() => core.current?.setReduced(reduced), [reduced])
  useEffect(() => core.current?.setData(data), [data])

  const nets = live?.networks ?? []
  const g = geometry(size.w, size.h, nets.length)
  const labels = g.s >= 340 && size.room >= 150 && running

  return (
    <div className="core" ref={box} data-testid="fuse-core">
      <canvas ref={canvas} aria-hidden="true" />
      <div className="core-center">{center}</div>
      {labels && (
        <ul className="core-labels" aria-hidden="true">
          {nets.map((n, i) => {
            const p = g.sats[i]
            if (!p) return null
            const dx = p.x - g.c.x
            const dy = p.y - g.c.y
            const len = Math.hypot(dx, dy) || 1
            // Sit just outside the orb, anchored on the side facing away from the centre.
            const x = p.x + (dx / len) * 24
            const y = p.y + (dy / len) * 24
            const ux = dx / len
            const uy = dy / len
            const align = ux < -0.35 ? 'right' : ux > 0.35 ? 'left' : 'center'
            const tx = align === 'right' ? '-100%' : align === 'left' ? '0' : '-50%'
            const ty = uy > 0.6 ? '0' : uy < -0.6 ? '-100%' : '-50%'
            return (
              <li
                key={n.name}
                data-down={n.dead || undefined}
                style={{ left: x, top: y, transform: `translate(${tx}, ${ty})`, textAlign: align }}
              >
                <span className="name">{netTitle(n)}</span>
                <span className="num">{n.dead ? 'Offline' : rateText(n.rate)}</span>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
