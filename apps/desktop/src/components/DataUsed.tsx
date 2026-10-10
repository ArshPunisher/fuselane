import { useEffect, useState } from 'react'
import { useApp } from '../lib/store'
import { bytes } from '../lib/format'
import { assignLanes } from '../lib/lanes'
import type { UsageHistory } from '../lib/types'
import { intlLocale, t } from '../lib/i18n'

const KIND: Record<string, string> = { 'iPhone USB': 'tether' }

/**
 * Data used (B10.5): how much each network carried this month and each day,
 * from what Fuselane downloaded. Handy for a phone on a data plan.
 */
export function DataUsed() {
  const backend = useApp((s) => s.backend)
  const networks = useApp((s) => s.networks)
  const [h, setH] = useState<UsageHistory | null>(null)
  useEffect(() => {
    if (!backend) return
    const load = () =>
      void backend
        .usageHistory()
        .then(setH)
        .catch(() => {})
    load()
    const timer = setInterval(load, 60_000)
    return () => clearInterval(timer)
  }, [backend])
  if (!h || !h.days.length) return null
  const names = [...new Set(h.days.flatMap((d) => Object.keys(d.nets)))]
  const lanes = assignLanes(
    names.map((name) => ({
      name,
      kind: networks.find((n) => n.name === name)?.kind ?? KIND[h.labels[name] ?? ''] ?? 'other',
    })),
  )
  const month = new Date().toISOString().slice(0, 7)
  const monthTotal = (name: string) =>
    h.days.filter((d) => d.day.startsWith(month)).reduce((a, d) => a + (d.nets[name] ?? 0), 0)
  const last30 = h.days.slice(-30)
  const max = Math.max(1, ...last30.map((d) => Object.values(d.nets).reduce((a, b) => a + b, 0)))
  return (
    <section className="net-limits data-used" aria-labelledby="du-title">
      <h2 id="du-title" className="section-title">
        {t('Data used')}
      </h2>
      <p className="muted">
        {t('What each network carried for Fuselane this month and day by day.')}
      </p>
      <ul className="du-totals">
        {names.map((n, i) => (
          <li key={n} style={{ '--lane': `var(--lane-${lanes[i]})` } as React.CSSProperties}>
            <span className="du-dot" aria-hidden />
            <span translate="no">{h.labels[n] ?? n}</span>
            <span className="num">{bytes(monthTotal(n))}</span>
          </li>
        ))}
      </ul>
      <div
        className="du-chart"
        role="img"
        aria-label={t('Data used per day for the last {n} days', { n: last30.length })}
      >
        {last30.map((d) => {
          const total = Object.values(d.nets).reduce((a, b) => a + b, 0)
          return (
            <div
              key={d.day}
              className="du-day"
              title={`${new Date(d.day).toLocaleDateString(intlLocale(), { day: 'numeric', month: 'short' })}: ${bytes(total)}`}
            >
              <div className="du-stack" style={{ height: `${(total / max) * 100}%` }}>
                {names.map((n, i) =>
                  d.nets[n] ? (
                    <span
                      key={n}
                      style={{
                        flexGrow: d.nets[n],
                        background: `var(--lane-${lanes[i]})`,
                      }}
                    />
                  ) : null,
                )}
              </div>
            </div>
          )
        })}
      </div>
    </section>
  )
}
