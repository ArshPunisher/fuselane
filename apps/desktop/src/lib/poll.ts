import { useEffect, useState } from 'react'

/**
 * Calls `load` now and about every `ms` while the component is shown and the
 * window is visible; the latest result, or null before the first one.
 */
export function usePoll<T>(load: () => Promise<T>, ms: number, deps: unknown[]): T | null {
  const [value, setValue] = useState<T | null>(null)
  useEffect(() => {
    let stop = false
    let timer = 0
    const tick = async () => {
      if (!document.hidden) {
        try {
          const v = await load()
          if (!stop) setValue(v)
        } catch {
          // Gone or not ready: keep what was shown; the next tick tries again.
        }
      }
      if (!stop) timer = window.setTimeout(tick, ms)
    }
    void tick()
    return () => {
      stop = true
      clearTimeout(timer)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)
  return value
}
