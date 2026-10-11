// A speed test of the browser's own connection, for when the Fuselane app isn't
// connected (REVIEW-2026-10-11 D). The same Cloudflare speed endpoints the app
// uses (they allow requests from extensions). A browser can only test the
// connection it's using; the app tests every network separately.

export const SPEED_HOST = 'https://speed.cloudflare.com'

export type Step = 'ping' | 'down' | 'up'
export interface Reading {
  step: Step
  /** Bytes per second now (0 during ping). */
  bps: number
  /** 0..1 through the whole test. */
  overall: number
}
export interface Result {
  pingMs: number | null
  downBps: number | null
  upBps: number | null
}

type FetchFn = (url: string, init?: RequestInit) => Promise<Response>

const STREAMS = 4
const STEP_MS = 7000
const now = () => performance.now()

/** Median of a few tiny requests, after one to warm the connection up. */
async function ping(fetchFn: FetchFn, signal: AbortSignal): Promise<number | null> {
  const times: number[] = []
  for (let i = 0; i < 6; i++) {
    const t = now()
    const r = await fetchFn(`${SPEED_HOST}/__down?bytes=0`, { cache: 'no-store', signal })
    await r.arrayBuffer()
    if (i > 0) times.push(now() - t)
  }
  times.sort((a, b) => a - b)
  return times.length ? times[Math.floor(times.length / 2)]! : null
}

/** Several streams at once for STEP_MS; the speed leaves out the first second (slow start). */
async function measure(
  step: 'down' | 'up',
  fetchFn: FetchFn,
  signal: AbortSignal,
  onReading: (bps: number, progress: number) => void,
): Promise<number | null> {
  const start = now()
  let bytes = 0
  let counted = 0
  let countedFrom = 0
  const stop = new AbortController()
  const end = () => stop.abort()
  signal.addEventListener('abort', end)
  const timer = setTimeout(end, STEP_MS)
  const tick = setInterval(() => {
    const t = now() - start
    if (t > 1000 && !countedFrom) {
      countedFrom = now()
      counted = bytes
    }
    const bps = countedFrom
      ? ((bytes - counted) * 1000) / Math.max(1, now() - countedFrom)
      : (bytes * 1000) / Math.max(1, t)
    onReading(bps, Math.min(1, t / STEP_MS))
  }, 100)
  const body = step === 'up' ? new Uint8Array(4 * 1024 * 1024) : null
  try {
    await Promise.all(
      Array.from({ length: STREAMS }, async () => {
        while (!stop.signal.aborted) {
          try {
            if (step === 'down') {
              const r = await fetchFn(`${SPEED_HOST}/__down?bytes=50000000`, {
                cache: 'no-store',
                signal: stop.signal,
              })
              const reader = r.body!.getReader()
              for (;;) {
                const { done, value } = await reader.read()
                if (done) break
                bytes += value.length
              }
            } else {
              await fetchFn(`${SPEED_HOST}/__up`, { method: 'POST', body, signal: stop.signal })
              bytes += body!.length
            }
          } catch {
            if (!stop.signal.aborted) await new Promise((r) => setTimeout(r, 200))
          }
        }
      }),
    )
  } finally {
    clearTimeout(timer)
    clearInterval(tick)
    signal.removeEventListener('abort', end)
  }
  if (signal.aborted) throw signal.reason ?? new Error('stopped')
  const span = countedFrom ? now() - countedFrom : now() - start
  const got = countedFrom ? bytes - counted : bytes
  return span > 0 && got > 0 ? (got * 1000) / span : null
}

export async function speedTest(
  onReading: (r: Reading) => void,
  signal: AbortSignal,
  fetchFn: FetchFn = fetch,
): Promise<Result> {
  onReading({ step: 'ping', bps: 0, overall: 0 })
  const pingMs = await ping(fetchFn, signal)
  const downBps = await measure('down', fetchFn, signal, (bps, p) =>
    onReading({ step: 'down', bps, overall: 0.05 + p * 0.5 }),
  )
  const upBps = await measure('up', fetchFn, signal, (bps, p) =>
    onReading({ step: 'up', bps, overall: 0.55 + p * 0.45 }),
  )
  return { pingMs, downBps, upBps }
}

/** "84.2 Mbps": speed tests speak in bits. */
export function mbps(bps: number | null | undefined): string {
  if (!bps || bps <= 0) return '0'
  const m = (bps * 8) / 1e6
  return m >= 100 ? m.toFixed(0) : m >= 10 ? m.toFixed(1) : m.toFixed(2)
}
