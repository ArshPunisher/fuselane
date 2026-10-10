// Network colours (DESIGN-SYSTEM.md §3): assigned by kind first, then the spares.
import type { LiveNet, NetPref, NetView } from './types'
import { mark, t } from './i18n'

export type Lane = 'tide' | 'volt' | 'iris' | 'rose' | 'mint' | 'sky' | 'lilac' | 'steel'

const BY_KIND: Record<string, Lane> = {
  wifi: 'tide',
  tether: 'volt',
  ethernet: 'iris',
  cellular: 'rose',
}
const SPARE: Lane[] = ['mint', 'sky', 'lilac']

const NEUTRAL = new Set(['vpn', 'virtual', 'other', 'loopback'])

export const LANES: Lane[] = ['tide', 'volt', 'iris', 'rose', 'mint', 'sky', 'lilac', 'steel']

/** The user's names and colours by device name; kept in step by the store. */
let prefs = new Map<string, NetPref>()
export function setNetPrefs(list: NetPref[]) {
  prefs = new Map(list.map((p) => [p.name, p]))
}

/** Stable lanes for a list of networks, in order: chosen colours first, then by kind. */
export function assignLanes(nets: Pick<LiveNet | NetView, 'kind' | 'name'>[]): Lane[] {
  const chosen = nets.map((n) => {
    const lane = prefs.get(n.name)?.lane
    return lane && (LANES as string[]).includes(lane) ? (lane as Lane) : null
  })
  const used = new Set<Lane>(chosen.filter((l): l is Lane => l !== null))
  return nets.map((n, i) => {
    const own = chosen[i]
    if (own) return own
    const preferred = BY_KIND[n.kind] ?? (NEUTRAL.has(n.kind) ? 'steel' : undefined)
    const lane =
      preferred && !used.has(preferred) ? preferred : (SPARE.find((s) => !used.has(s)) ?? 'steel')
    used.add(lane)
    return lane
  })
}

const KIND_LABEL: Record<string, string> = {
  wifi: 'Wi-Fi',
  ethernet: 'Ethernet',
  tether: mark('USB tether'),
  cellular: mark('Cellular'),
  vpn: 'VPN',
  virtual: mark('Virtual'),
  loopback: mark('Loopback'),
}

/** The kind of network in words, in the language in use. */
export function kindLabel(kind: string): string {
  return t(KIND_LABEL[kind] ?? mark('Network'))
}

/** The name people know: the OS's friendly label when it has one. */
export function netTitle(n: { name: string; label: string; kind: string }): string {
  const custom = prefs.get(n.name)?.label
  if (custom) return custom
  return n.label && n.label !== n.name ? n.label : kindLabel(n.kind)
}
