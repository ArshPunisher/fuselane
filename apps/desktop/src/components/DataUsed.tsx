import { useEffect, useRef, useState } from 'react'
import { ChartBar } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { bytes, rateText } from '../lib/format'
import { assignLanes, netTitle } from '../lib/lanes'
import type { AllowanceView, UsageHistory } from '../lib/types'
import { intlLocale, t } from '../lib/i18n'
import { NetIcon } from './NetIcon'
import { useLiveRates } from './NetworksView'

const KIND: Record<string, string> = { 'iPhone USB': 'tether' }

/** Seconds of speed kept for each network's live line. */
const HISTORY = 60

/** Today in local time, YYYY-MM-DD (the app counts days in local time too). */
function today(): string {
  const d = new Date()
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

function dayLabel(day: string, opts: Intl.DateTimeFormatOptions): string {
  return new Date(`${day}T12:00:00`).toLocaleDateString(intlLocale(), opts)
}

/** A number that eases to its new value instead of jumping. */
function useEased(target: number, ms = 700): number {
  const [shown, setShown] = useState(target)
  const from = useRef(target)
  useEffect(() => {
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
      setShown(target)
      from.current = target
      return
    }
    const start = performance.now()
    const a = from.current
    let raf = 0
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / ms)
      const v = a + (target - a) * (1 - (1 - k) ** 3)
      setShown(v)
      from.current = v
      if (k < 1) raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
    return () => cancelAnimationFrame(raf)
  }, [target, ms])
  return shown
}

/** The last minute of speed per network, one sample a second. */
function useRateHistory(rates: Record<string, number>): Record<string, number[]> {
  const latest = useRef(rates)
  latest.current = rates
  const [hist, setHist] = useState<Record<string, number[]>>({})
  useEffect(() => {
    const timer = setInterval(() => {
      setHist((h) => {
        const next: Record<string, number[]> = {}
        const names = new Set([...Object.keys(h), ...Object.keys(latest.current)])
        // A new network starts with a quiet minute, so its line spans the width.
        const quiet = () => Array<number>(HISTORY - 1).fill(0)
        for (const n of names)
          next[n] = [...(h[n] ?? quiet()), latest.current[n] ?? 0].slice(-HISTORY)
        return next
      })
    }, 1000)
    return () => clearInterval(timer)
  }, [])
  return hist
}

/** A minute of speed as a line, newest on the right. */
function Spark({ values }: { values: number[] }) {
  const w = 120
  const h = 28
  const peak = Math.max(1, ...values)
  const offset = HISTORY - values.length
  const line = values
    .map((v, i) => {
      const x = ((offset + i) / (HISTORY - 1)) * w
      const y = h - 2 - (v / peak) * (h - 4)
      return `${i ? 'L' : 'M'}${x.toFixed(1)} ${y.toFixed(1)}`
    })
    .join(' ')
  return (
    <svg className="use-spark" viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" aria-hidden>
      {line && <path className="use-spark-area" d={`${line} L${w} ${h} L0 ${h} Z`} />}
      {line && <path className="use-spark-line" d={line} />}
    </svg>
  )
}

/** "1.4 GB" that counts up when it changes. */
function Amount({ value, className }: { value: number; className: string }) {
  const v = useEased(value)
  return <span className={`num ${className}`}>{bytes(Math.max(0, Math.round(v)))}</span>
}

/**
 * Usage (B10.5): what each network carried for Fuselane this month and day by
 * day, and what's moving right now. Handy for a phone on a data plan.
 */
export function DataUsed() {
  const backend = useApp((s) => s.backend)
  const networks = useApp((s) => s.networks)
  const rates = useLiveRates()
  const hist = useRateHistory(rates)
  const [h, setH] = useState<UsageHistory | null>(null)
  const [allow, setAllow] = useState<AllowanceView[] | null>(null)
  const [polled, setPolled] = useState(() => Date.now())
  const [now, setNow] = useState(() => Date.now())
  const [hover, setHover] = useState<number | null>(null)
  useEffect(() => {
    if (!backend) return
    const load = () => {
      void backend
        .usageHistory()
        .then((v) => {
          setH(v)
          setPolled(Date.now())
        })
        .catch(() => setH({ days: [], labels: {} }))
      void backend
        .allowances()
        .then(setAllow)
        .catch(() => setAllow([]))
    }
    load()
    // The app adds up usage every few seconds; follow it closely.
    const timer = setInterval(load, 5000)
    return () => clearInterval(timer)
  }, [backend])
  // Between polls, today's figure grows with what's moving now.
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 500)
    return () => clearInterval(timer)
  }, [])

  if (!h || !allow) {
    return <div className="use-loading" aria-busy="true" aria-label={t('Loading')} />
  }

  // Every network that ever carried something, plus the ones here now.
  const names = [
    ...new Set([
      ...allow.filter((a) => a.used > 0).map((a) => a.name),
      ...h.days.flatMap((d) => Object.keys(d.nets)),
      ...networks.filter((n) => n.usable).map((n) => n.name),
    ]),
  ]
  const label = (name: string) => {
    const net = networks.find((n) => n.name === name)
    return net ? netTitle(net) : (h.labels[name] ?? name)
  }
  const kind = (name: string) =>
    networks.find((n) => n.name === name)?.kind ?? KIND[h.labels[name] ?? ''] ?? 'other'
  const lanes = assignLanes(names.map((name) => ({ name, kind: kind(name) })))
  const laneOf = (name: string) => `var(--lane-${lanes[names.indexOf(name)] ?? 'steel'})`

  const day = today()
  const extra = (name: string) => ((rates[name] ?? 0) * Math.max(0, now - polled)) / 1000
  const todayOf = (name: string) =>
    (h.days.find((d) => d.day === day)?.nets[name] ?? 0) + extra(name)
  // This month: the allowance meter's count (kept even before day-by-day
  // history existed) or the days added up, whichever is more.
  const month = day.slice(0, 7)
  const monthOf = (name: string) => {
    const fromDays = h.days
      .filter((d) => d.day.startsWith(month))
      .reduce((a, d) => a + (d.nets[name] ?? 0), 0)
    const metered = allow.find((a) => a.name === name)?.used ?? 0
    return Math.max(fromDays, metered) + extra(name)
  }
  const totals = names.map((n) => ({ name: n, month: monthOf(n), today: todayOf(n) }))
  const monthTotal = totals.reduce((a, x) => a + x.month, 0)
  const todayTotal = totals.reduce((a, x) => a + x.today, 0)
  const liveTotal = Object.values(rates).reduce((a, r) => a + r, 0)

  // The last 30 days, today always there (and growing as it goes).
  const days = h.days.slice(-30).map((d) => ({ day: d.day, nets: { ...d.nets } }))
  if (days[days.length - 1]?.day !== day) days.push({ day, nets: {} })
  if (days.length > 30) days.shift()
  const last = days[days.length - 1]!
  for (const n of names) {
    const v = todayOf(n)
    if (v > 0) last.nets[n] = v
  }
  const dayTotal = (d: { nets: Record<string, number> }) =>
    Object.values(d.nets).reduce((a, b) => a + b, 0)
  const peak = Math.max(1, ...days.map(dayTotal))
  const withData = days.filter((d) => dayTotal(d) > 0)
  const average = withData.length
    ? withData.reduce((a, d) => a + dayTotal(d), 0) / withData.length
    : 0

  if (monthTotal === 0 && withData.length === 0) {
    return (
      <section className="use-empty" aria-labelledby="du-title">
        <span className="use-empty-icon" aria-hidden>
          <ChartBar size={30} weight="duotone" />
        </span>
        <h2 id="du-title">{t('Nothing counted yet')}</h2>
        <p className="muted">
          {t(
            'Everything Fuselane downloads is counted here, per network and day by day, so you can see how much the phone carried. Start a download and watch it fill in.',
          )}
        </p>
      </section>
    )
  }

  const shown = totals.filter((x) => x.month > 0)
  const hovered = hover === null ? null : days[hover]
  return (
    <section className="usage" aria-labelledby="du-title">
      <h2 id="du-title" className="sr-only">
        {t('Data used')}
      </h2>
      <div className="use-hero">
        <div className="use-month">
          <span className="use-kicker">{t('This month')}</span>
          <Amount value={monthTotal} className="use-big" />
          <span className="muted">
            {t('{size} today', { size: bytes(Math.round(todayTotal)) })}
          </span>
          <div className="use-share" role="img" aria-label={t('Share of each network this month')}>
            {shown.map((x) => (
              <span
                key={x.name}
                style={{ flexGrow: x.month, background: laneOf(x.name) }}
                title={`${label(x.name)}: ${bytes(Math.round(x.month))}`}
              />
            ))}
          </div>
          <ul className="use-legend">
            {shown.map((x) => (
              <li key={x.name} style={{ '--lane': laneOf(x.name) } as React.CSSProperties}>
                <span className="use-swatch" aria-hidden />
                <span translate="no">{label(x.name)}</span>
                <span className="num">{Math.round((x.month / monthTotal) * 100)}%</span>
              </li>
            ))}
          </ul>
        </div>
        <div className="use-live" data-on={liveTotal > 0 || undefined}>
          <span className="use-kicker">
            <span className="use-pulse" aria-hidden /> {t('Right now')}
          </span>
          <span className="num use-rate">{liveTotal > 0 ? rateText(liveTotal) : t('Idle')}</span>
          <ul className="use-live-list">
            {names
              .filter((n) => networks.some((x) => x.name === n && x.usable))
              .map((n) => (
                <li
                  key={n}
                  style={{ '--lane': laneOf(n) } as React.CSSProperties}
                  data-on={(rates[n] ?? 0) > 0 || undefined}
                >
                  <span className="use-breath" aria-hidden />
                  <span className="use-live-name" translate="no">
                    {label(n)}
                  </span>
                  <Spark values={hist[n] ?? []} />
                  <span className="num use-live-rate">
                    {(rates[n] ?? 0) > 0 ? rateText(rates[n] ?? 0) : '0'}
                  </span>
                </li>
              ))}
          </ul>
        </div>
      </div>

      <div className="use-chart-card">
        <div className="use-chart-head">
          <h3>{t('Last {n} days', { n: days.length })}</h3>
          <span className="muted num">
            {hovered
              ? `${dayLabel(hovered.day, { weekday: 'short', day: 'numeric', month: 'short' })}: ${bytes(Math.round(dayTotal(hovered)))}`
              : t('About {size} a day', { size: bytes(Math.round(average)) })}
          </span>
        </div>
        <div
          className="use-chart"
          role="img"
          aria-label={t('Data used per day for the last {n} days', { n: days.length })}
          onMouseLeave={() => setHover(null)}
        >
          <span className="use-grid" style={{ bottom: '100%' }}>
            <span className="num">{bytes(peak)}</span>
          </span>
          <span className="use-grid" style={{ bottom: '50%' }}>
            <span className="num">{bytes(peak / 2)}</span>
          </span>
          {days.map((d, i) => (
            <div
              key={d.day}
              className="use-day"
              data-today={d.day === day || undefined}
              data-hover={hover === i || undefined}
              onMouseEnter={() => setHover(i)}
              style={{ '--i': i } as React.CSSProperties}
            >
              <div className="use-stack" style={{ height: `${(dayTotal(d) / peak) * 100}%` }}>
                {names.map((n) =>
                  d.nets[n] ? (
                    <span key={n} style={{ flexGrow: d.nets[n], background: laneOf(n) }} />
                  ) : null,
                )}
              </div>
            </div>
          ))}
        </div>
        <div className="use-axis" aria-hidden>
          {days.map((d, i) => (
            <span key={d.day}>
              {i === days.length - 1
                ? t('Today')
                : (days.length - 1 - i) % 7 === 0
                  ? dayLabel(d.day, { day: 'numeric', month: 'short' })
                  : ''}
            </span>
          ))}
        </div>
      </div>

      <div className="use-cards">
        {totals.map((x) => {
          const a = allow.find((v) => v.name === x.name)
          const limit = a?.allowance ?? null
          const share = limit ? Math.min(1, x.month / limit) : null
          return (
            <article
              key={x.name}
              className="use-card"
              style={{ '--lane': laneOf(x.name) } as React.CSSProperties}
            >
              <header>
                <span className="use-card-icon" aria-hidden>
                  <NetIcon kind={kind(x.name)} />
                </span>
                <span translate="no">{label(x.name)}</span>
              </header>
              <Amount value={x.month} className="use-card-big" />
              <span className="muted">
                {t('{size} today', { size: bytes(Math.round(x.today)) })}
              </span>
              {share !== null && limit !== null && a ? (
                <div className="use-allow" data-near={share >= 0.8 || undefined}>
                  <span className="use-allow-bar" aria-hidden>
                    <span style={{ transform: `scaleX(${share})` }} />
                  </span>
                  <span className="muted">
                    {t('{used} of {limit}, resets {date}', {
                      used: bytes(Math.round(x.month)),
                      limit: bytes(limit),
                      date: dayLabel(a.resetsOn, { day: 'numeric', month: 'short' }),
                    })}
                  </span>
                </div>
              ) : null}
            </article>
          )
        })}
      </div>
    </section>
  )
}
