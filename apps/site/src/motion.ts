// Motion helpers shared by every animated part of the site. The rules
// (MOTION.md §1, §5): motion stops off screen and in background tabs, and
// reduced motion means a still, complete picture.

export const prefersReduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches

/** Calls back whenever `el` enters or leaves the viewport (grown by `margin`). */
export function watch(el: Element, cb: (visible: boolean) => void, margin = '0px') {
  if (!('IntersectionObserver' in window)) {
    cb(true)
    return
  }
  new IntersectionObserver((entries) => cb(entries.some((e) => e.isIntersecting)), {
    rootMargin: margin,
  }).observe(el)
}

/** Calls back once, the first time `el` is at least `share` visible. */
export function whenSeen(el: Element, cb: () => void, share = 0.35) {
  if (!('IntersectionObserver' in window)) {
    cb()
    return
  }
  const io = new IntersectionObserver(
    (entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        io.disconnect()
        cb()
      }
    },
    { threshold: share },
  )
  io.observe(el)
}

/**
 * Runs `frame(dt)` on every animation frame, but only while `el` is on screen
 * and the tab is visible. `dt` is in seconds and capped, so a pause never
 * turns into a jump.
 */
export function loop(el: Element, frame: (dt: number) => void, margin = '80px') {
  let visible = false
  let running = false
  let raf = 0
  let last = 0
  const tick = (now: number) => {
    const dt = Math.min(0.05, Math.max(0, (now - last) / 1000))
    last = now
    frame(dt)
    raf = requestAnimationFrame(tick)
  }
  const update = () => {
    const go = visible && !document.hidden
    if (go && !running) {
      running = true
      last = performance.now()
      raf = requestAnimationFrame(tick)
    } else if (!go && running) {
      running = false
      cancelAnimationFrame(raf)
    }
  }
  watch(
    el,
    (v) => {
      visible = v
      update()
    },
    margin,
  )
  document.addEventListener('visibilitychange', update)
  return { isRunning: () => running }
}

/** Ease toward a target at a frame-rate independent rate. */
export const approach = (v: number, to: number, rate: number, dt: number) =>
  v + (to - v) * (1 - Math.exp(-rate * dt))

/** Reads a colour token (any CSS colour) as an `rgb()`/`oklch()` string the canvas understands. */
export function token(name: string, el: Element = document.documentElement) {
  return getComputedStyle(el).getPropertyValue(name).trim() || '#888'
}

/** "54 s", "2 min 32 s", "1 h 5 min". */
export function duration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0 s'
  const s = Math.round(seconds)
  if (s < 60) return `${s} s`
  if (s < 3600) {
    const m = Math.floor(s / 60)
    const rest = s % 60
    return rest ? `${m} min ${rest} s` : `${m} min`
  }
  const h = Math.floor(s / 3600)
  const m = Math.round((s % 3600) / 60)
  return m ? `${h} h ${m} min` : `${h} h`
}
