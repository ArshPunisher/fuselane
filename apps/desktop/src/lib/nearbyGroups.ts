import type { NearbyTransfer } from './types'

/** One tile in the activity list: a single transfer, or one selection of many files. */
export type ActivityRow =
  | { kind: 'one'; key: string; x: NearbyTransfer }
  | {
      kind: 'batch'
      /** The selection's id: cancel and clear take it. */
      key: string
      items: NearbyTransfer[]
      direction: 'out' | 'in'
      device: string
      files: number
      filesDone: number
      filesFailed: number
      bytes: number
      done: number
      state: NearbyTransfer['state']
    }

const LIVE = new Set<NearbyTransfer['state']>(['asking', 'sending', 'receiving'])

/**
 * Newest selection first; inside one, the order it was picked in. The core
 * already sends them this way, but sorting here too keeps the demo and any
 * older core stable: a row never moves because its progress changed.
 */
export function groupTransfers(transfers: NearbyTransfer[]): ActivityRow[] {
  const groups = new Map<string, NearbyTransfer[]>()
  for (const x of transfers) {
    const k = x.batch ?? x.id
    const g = groups.get(k)
    if (g) g.push(x)
    else groups.set(k, [x])
  }
  const first = (g: NearbyTransfer[]) => Math.min(...g.map((x) => x.seq))
  const rows = [...groups.entries()].map(([key, g]) => {
    g.sort((a, b) => a.batchIndex - b.batchIndex || a.seq - b.seq || a.id.localeCompare(b.id))
    return { key, g, at: first(g) }
  })
  rows.sort((a, b) => b.at - a.at || a.key.localeCompare(b.key))
  return rows.map(({ key, g }): ActivityRow => {
    const head = g[0]!
    const files = Math.max(head.batchFiles, g.length)
    if (g.length === 1 && files <= 1) return { kind: 'one', key, x: head }
    const filesDone = g.filter((x) => x.state === 'done').length
    const filesFailed = g.filter((x) => x.state === 'failed' || x.state === 'declined').length
    const live = g.some((x) => LIVE.has(x.state))
    // The batch is still going while its files are still coming: a phone sends
    // them one after another, so fewer rows than files means more to come.
    const more = g.length < files
    const bytes = Math.max(
      head.batchBytes,
      g.reduce((n, x) => n + x.size, 0),
    )
    const done = g.reduce((n, x) => n + (x.state === 'done' ? x.size : Math.min(x.done, x.size)), 0)
    let state: NearbyTransfer['state']
    if (live || (more && !g.some((x) => x.state === 'failed' || x.state === 'cancelled'))) {
      state = head.direction === 'out' ? 'sending' : 'receiving'
    } else if (filesFailed) state = 'failed'
    else if (g.some((x) => x.state === 'cancelled')) state = 'cancelled'
    else state = 'done'
    return {
      kind: 'batch',
      key,
      items: g,
      direction: head.direction,
      device: head.device,
      files,
      filesDone,
      filesFailed,
      bytes,
      done: Math.min(done, bytes || done),
      state,
    }
  })
}
