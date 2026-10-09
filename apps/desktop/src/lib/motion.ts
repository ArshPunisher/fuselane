// Motion helpers shared by the window (docs: "simple, but alive"). Live numbers
// glide between the backend's updates instead of jumping; nothing here touches
// React state per frame, and everything goes still under reduced motion or
// while the window is hidden.
import { useLayoutEffect, useRef } from 'react'

const reduce = () => matchMedia('(prefers-reduced-motion: reduce)').matches

/** Looping CSS animations pause while the window can't be seen. */
export function pauseWhenHidden() {
  const set = () => document.documentElement.toggleAttribute('data-hidden', document.hidden)
  document.addEventListener('visibilitychange', set)
  set()
}

/**
 * Glides from the last value to `value` (about 0.6 s, easing out), calling
 * `draw` with each in-between number. `draw` writes to the DOM directly.
 */
export function useGlide(value: number, draw: (n: number) => void) {
  const shown = useRef(value)
  const drawRef = useRef(draw)
  drawRef.current = draw
  useLayoutEffect(() => {
    const from = shown.current
    const to = value
    if (reduce() || document.hidden || from === to || !Number.isFinite(from)) {
      shown.current = to
      drawRef.current(to)
      return
    }
    let raf = 0
    const start = performance.now()
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / 600)
      shown.current = from + (to - from) * (1 - (1 - t) ** 3)
      drawRef.current(shown.current)
      if (t < 1) raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
    return () => cancelAnimationFrame(raf)
  }, [value])
  // A new way of drawing (MB/s to Mbps) shows at once, without waiting for a value.
  useLayoutEffect(() => drawRef.current(shown.current))
}
