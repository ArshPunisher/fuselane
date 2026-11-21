/**
 * Wraps `fn` so a call made while the previous one is still running returns
 * undefined at once instead of starting another. For polling a slow peer: the
 * requests never pile up, and a late answer can't be paired with the wrong ask.
 */
export function skipWhileRunning<T>(fn: () => Promise<T>): () => Promise<T | undefined> {
  let running = false
  return async () => {
    if (running) return undefined
    running = true
    try {
      return await fn()
    } finally {
      running = false
    }
  }
}
