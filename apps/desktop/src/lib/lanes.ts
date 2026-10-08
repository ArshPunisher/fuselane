// Network colours (DESIGN-SYSTEM.md §3): assigned by kind first, then the spares.
import type { LiveNet, NetView } from './types'

export type Lane = 'tide' | 'volt' | 'iris' | 'rose' | 'mint' | 'sky' | 'lilac' | 'steel'

const BY_KIND: Record<string, Lane> = {
  wifi: 'tide',
  tether: 'volt',
  ethernet: 'iris',
  cellular: 'rose',
}
const SPARE: Lane[] = ['mint', 'sky', 'lilac']

const NEUTRAL = new Set(['vpn', 'virtual', 'other', 'loopback'])

/** Stable lanes for a list of networks, in order. */
export function assignLanes(nets: Pick<LiveNet | NetView, 'kind'>[]): Lane[] {
  const used = new Set<Lane>()
  return nets.map((n) => {
    const preferred = BY_KIND[n.kind] ?? (NEUTRAL.has(n.kind) ? 'steel' : undefined)
    const lane =
      preferred && !used.has(preferred) ? preferred : (SPARE.find((s) => !used.has(s)) ?? 'steel')
    used.add(lane)
    return lane
  })
}

export function kindLabel(kind: string): string {
  return (
    {
      wifi: 'Wi-Fi',
      ethernet: 'Ethernet',
      tether: 'USB tether',
      cellular: 'Cellular',
      vpn: 'VPN',
      virtual: 'Virtual',
      loopback: 'Loopback',
    }[kind] ?? 'Network'
  )
}

/** The name people know: the OS's friendly label when it has one. */
export function netTitle(n: { name: string; label: string; kind: string }): string {
  return n.label && n.label !== n.name ? n.label : kindLabel(n.kind)
}
