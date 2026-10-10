import { CheckCircle, Gauge, WarningCircle } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { netTitle } from '../lib/lanes'
import { rateText } from '../lib/format'
import type { NetNote } from '../lib/types'
import { t } from '../lib/i18n'

/** What a note says, with the person's name for the network and their speed unit. */
export function noteText(n: NetNote, title: string): string {
  const rate = rateText(n.rate ?? 0)
  switch (n.kind) {
    case 'slow': {
      // "Slowed" only when it was clearly faster before; a network that was never
      // fast is "only managing" its speed.
      const fell = n.best !== null && n.rate !== null && n.best >= 2 * n.rate
      return fell
        ? t(
            '{network} slowed to {rate} (throttled?), so the other networks carry the rest. Fuselane tries it again every few minutes.',
            { network: title, rate },
          )
        : t(
            '{network} is only managing {rate} (throttled?), so the other networks carry the rest. Fuselane tries it again every few minutes.',
            { network: title, rate },
          )
    }
    case 'back':
      return t("{network} is fast again ({rate}), so it's helping again.", { network: title, rate })
    case 'trouble':
      return n.message ?? t("{network} can't connect.", { network: title })
  }
}

const ICONS = { slow: Gauge, back: CheckCircle, trouble: WarningCircle } as const

/**
 * A download's network news (STEPS 8.2, 8.4): a network benched as throttled
 * and back again, or one whose proxy won't let it connect.
 */
export function NetworkNotes({ notes }: { notes: NetNote[] }) {
  const networks = useApp((s) => s.networks)
  useApp((s) => s.netPrefs) // re-render when a network is renamed
  if (!notes.length) return null
  const title = (name: string) => {
    const n = networks.find((x) => x.name === name)
    return n ? netTitle(n) : name
  }
  return (
    <ul className="net-notes" aria-label={t('Network notes')}>
      {notes.map((n) => {
        const Icon = ICONS[n.kind]
        return (
          <li key={n.name} data-kind={n.kind}>
            <Icon size={16} aria-hidden />
            <span>{noteText(n, title(n.name))}</span>
          </li>
        )
      })}
    </ul>
  )
}
