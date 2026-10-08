// Every error is JSON with the right status and words a person can act on
// (BONDED-UPLOADS.md §6; never a generic 500).
import type { Context } from 'hono'
import type { ContentfulStatusCode } from 'hono/utils/http-status'

export function fail(
  c: Context,
  status: ContentfulStatusCode,
  code: string,
  message: string,
  hint?: string,
  retryAfter?: number,
) {
  if (retryAfter) c.header('Retry-After', String(retryAfter))
  return c.json({ error: { code, message, ...(hint ? { hint } : {}) } }, status)
}

export function humanBytes(n: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let v = n
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${i === 0 || v >= 100 ? Math.round(v) : v.toFixed(1)} ${units[i]}`
}
