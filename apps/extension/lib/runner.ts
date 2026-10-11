// Runs the extension's own downloads in a page (Chrome's offscreen document, or
// Firefox's background page): fetches with lib/fetcher.ts into a private file
// (the origin-private file system), then hands a blob link to the background,
// which saves it through the browser's downloads. It reports events only; the
// background keeps the records (offscreen pages have no storage API).
import { TooBig, probe, run, split, type Part } from './fetcher.ts'
import type { SelfJob } from './selfjobs.ts'

export type RunnerEvent =
  | {
      type: 'self.progress'
      id: string
      written: number
      rate: number
      parts: Part[] | null
      total: number | null
      finalUrl: string
      name?: string
    }
  | { type: 'self.ready'; id: string; blobUrl: string }
  | { type: 'self.paused'; id: string; written: number; parts: Part[] | null }
  | { type: 'self.failed'; id: string; error: string }
  | { type: 'self.too-big'; id: string; size: number }

interface Active {
  stop: AbortController
  why: 'pause' | 'cancel' | null
}

const PARTS = 6
/** fetch must be called on the window (or worker), never as a bare function. */
const get = (url: string, init?: RequestInit) => fetch(url, init)
const TICK_MS = 500

export function createRunner(emit: (e: RunnerEvent) => void) {
  const active = new Map<string, Active>()
  const blobs = new Map<string, string>()

  async function file(id: string, create: boolean) {
    const root = await navigator.storage.getDirectory()
    return root.getFileHandle(`${id}.part`, { create })
  }

  async function removeFile(id: string) {
    const root = await navigator.storage.getDirectory()
    await root.removeEntry(`${id}.part`).catch(() => {})
  }

  async function start(job: SelfJob, limit: number) {
    if (active.has(job.id)) return
    const me: Active = { stop: new AbortController(), why: null }
    active.set(job.id, me)
    let writable: FileSystemWritableFileStream | null = null
    let parts = job.parts
    let total = job.total
    let finalUrl = job.finalUrl
    let written = parts ? parts.reduce((n, p) => n + p.done, 0) : 0
    try {
      // First run (or a server without ranges, which starts over): look first.
      if (!parts) {
        const p = await probe(job.url, get, me.stop.signal)
        total = p.total
        finalUrl = p.finalUrl
        if (total !== null && total > limit) {
          emit({ type: 'self.too-big', id: job.id, size: total })
          return
        }
        parts = p.ranges && total ? split(total, PARTS) : null
        written = 0
        emit({
          type: 'self.progress',
          id: job.id,
          written: 0,
          rate: 0,
          parts,
          total,
          finalUrl,
          name: p.filename ?? undefined,
        })
      }
      const handle = await file(job.id, true)
      writable = await handle.createWritable({ keepExistingData: parts !== null })
      const w = writable
      let lastTick = Date.now()
      let lastBytes = written
      let rate = 0
      written = await run({
        url: finalUrl,
        parts,
        fetchFn: get,
        signal: me.stop.signal,
        limit: parts ? undefined : limit,
        write: (position, data) =>
          w.write({ type: 'write', position, data: data as Uint8Array<ArrayBuffer> }),
        onProgress: (ps, n) => {
          written = n
          const now = Date.now()
          if (now - lastTick < TICK_MS) return
          rate = ((n - lastBytes) * 1000) / (now - lastTick)
          lastTick = now
          lastBytes = n
          emit({ type: 'self.progress', id: job.id, written: n, rate, parts: ps, total, finalUrl })
        },
      })
      await writable.close()
      writable = null
      const blob = await (await file(job.id, false)).getFile()
      const blobUrl = URL.createObjectURL(blob)
      blobs.set(job.id, blobUrl)
      emit({
        type: 'self.progress',
        id: job.id,
        written,
        rate: 0,
        parts,
        total: total ?? written,
        finalUrl,
      })
      emit({ type: 'self.ready', id: job.id, blobUrl })
    } catch (e) {
      // Keep what's written for a pause; anything else throws it away.
      if (me.why === 'pause') {
        await writable?.close().catch(() => {})
        emit({ type: 'self.paused', id: job.id, written, parts })
      } else {
        await writable?.abort().catch(() => {})
        if (me.why !== 'cancel') {
          if (e instanceof TooBig) emit({ type: 'self.too-big', id: job.id, size: written })
          else
            emit({
              type: 'self.failed',
              id: job.id,
              error: e instanceof Error ? e.message : 'The download failed.',
            })
        }
        await removeFile(job.id)
      }
    } finally {
      active.delete(job.id)
    }
  }

  return {
    start,
    pause(id: string) {
      const a = active.get(id)
      if (a) {
        a.why = 'pause'
        a.stop.abort(new Error('paused'))
      }
    },
    async cancel(id: string) {
      const a = active.get(id)
      if (a) {
        a.why = 'cancel'
        a.stop.abort(new Error('cancelled'))
      } else {
        await removeFile(id)
      }
    },
    /** The browser saved the file: the private copy and its link can go. */
    async saved(id: string) {
      const url = blobs.get(id)
      if (url) URL.revokeObjectURL(url)
      blobs.delete(id)
      await removeFile(id)
    },
  }
}

export type Runner = ReturnType<typeof createRunner>
