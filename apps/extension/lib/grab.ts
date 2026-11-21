// What may start a grab, kept apart from the content script so it can be tested.

/** The parts of a click event that decide whether Fuselane takes the link. */
export interface ClickLike {
  /** False for a click a page made with `a.click()` or `dispatchEvent`. */
  isTrusted: boolean
  defaultPrevented: boolean
  button: number
  altKey: boolean
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
}

/**
 * A plain left click by the person. A click a page scripted is never one: the page
 * could otherwise queue unlimited downloads, with the person's cookies for the
 * linked site, and skip the browser's "download multiple files" question.
 */
export function clickCanGrab(e: ClickLike): boolean {
  if (!e.isTrusted || e.defaultPrevented || e.button !== 0) return false
  return !(e.altKey || e.ctrlKey || e.metaKey || e.shiftKey)
}

/**
 * At most `max` grabs per tab in any `windowMs`; the rest are left to the browser
 * (defence in depth if a page finds another way to click). Tabs that have gone
 * quiet are forgotten, so the map can't grow.
 */
export function createGrabCap(max = 3, windowMs = 2000) {
  const seen = new Map<number, number[]>()
  return {
    allow(tab: number, now: number): boolean {
      for (const [id, times] of seen) {
        if (times.every((t) => now - t >= windowMs)) seen.delete(id)
      }
      const recent = (seen.get(tab) ?? []).filter((t) => now - t < windowMs)
      if (recent.length >= max) {
        seen.set(tab, recent)
        return false
      }
      recent.push(now)
      seen.set(tab, recent)
      return true
    },
    size: () => seen.size,
  }
}
