import { useEffect, useState, type ReactNode } from 'react'
import {
  ArrowClockwise,
  ChartPieSlice,
  Gauge,
  Lightning,
  PencilSimple,
  ShieldCheck,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { assignLanes, kindLabel, LANES, netTitle, type Lane } from '../lib/lanes'
import type { AllowanceView, NetUse, NetView } from '../lib/types'
import { bytes as bytesText, rateText } from '../lib/format'
import { NetIcon } from './NetIcon'
import { Orb } from './Orb'
import { LimitField } from './LimitField'
import { DataUsed } from './DataUsed'
import { ProxyRow } from './NetworkProxy'
import { intlLocale, mark, t, tr } from '../lib/i18n'

/** Live speed per network, summed over running downloads. */
export function useLiveRates(): Record<string, number> {
  const live = useApp((s) => s.live)
  const jobs = useApp((s) => s.jobs)
  const torrents = useApp((s) => s.torrents)
  const running = new Set(jobs.filter((j) => j.status === 'running').map((j) => j.id))
  const out: Record<string, number> = {}
  for (const l of Object.values(live)) {
    if (!running.has(l.id)) continue
    for (const n of l.networks) out[n.name] = (out[n.name] ?? 0) + (n.dead ? 0 : n.rate)
  }
  // Torrent traffic counts too.
  for (const tor of torrents) {
    if (tor.status !== 'downloading') continue
    for (const n of tor.networks) out[n.name] = (out[n.name] ?? 0) + n.rate
  }
  return out
}

/**
 * The sidebar's total: every download and torrent together, said in so many
 * words so it isn't mistaken for the speed of the one that's open (B8.1).
 */
export function NetworksTotal() {
  const rates = useLiveRates()
  const total = Object.values(rates).reduce((a, r) => a + r, 0)
  return (
    <>
      <div className="net-total">
        <h2 className="group">{t('Networks')}</h2>
        {total > 0 && <span className="num">{rateText(total)}</span>}
      </div>
      {total > 0 && <p className="net-total-note">{t('All downloads together')}</p>}
    </>
  )
}

/** A network answering with a sign-in page (hotel, café, airport Wi-Fi). */
function SignIn({ net }: { net: NetView }) {
  const act = useApp((s) => s.act)
  return (
    <div className="signin" role="note">
      <p>
        {t(
          '{name} wants you to sign in before it reaches the internet, so downloads leave it out for now. Fuselane checks again every minute.',
          { name: netTitle(net) },
        )}
      </p>
      <button className="btn" onClick={() => act((b) => b.openSignIn())}>
        {t('Open sign-in page')}
      </button>
    </div>
  )
}

/** The colours' names, for screen readers. */
const LANE_NAME: Record<Lane, string> = {
  tide: mark('tide'),
  volt: mark('volt'),
  iris: mark('iris'),
  rose: mark('rose'),
  mint: mark('mint'),
  sky: mark('sky'),
  lilac: mark('lilac'),
  steel: mark('steel'),
}

/** Inline editor for a network's name and colour. */
function NetEditor({ net, lane, onDone }: { net: NetView; lane: Lane; onDone: () => void }) {
  const prefs = useApp((s) => s.netPrefs)
  const save = useApp((s) => s.saveNetPref)
  const current = prefs.find((p) => p.name === net.name)
  const [label, setLabel] = useState(current?.label ?? '')
  const [color, setColor] = useState<Lane>((current?.lane as Lane | undefined) ?? lane)
  const id = `edit-${net.name}`
  const tooLong = label.trim().length > 40
  return (
    <form
      className="net-editor"
      onSubmit={async (e) => {
        e.preventDefault()
        if (tooLong) return
        if (
          await save({
            name: net.name,
            label: label.trim() || null,
            lane: color,
            useFor: current?.useFor ?? 'always',
            hours: current?.hours ?? null,
          })
        )
          onDone()
      }}
    >
      <label htmlFor={id}>{t('Name')}</label>
      <input
        id={id}
        value={label}
        maxLength={60}
        autoComplete="off"
        placeholder={`${netTitle({ ...net, name: '' })}…`}
        aria-invalid={tooLong ? true : undefined}
        aria-describedby={tooLong ? `${id}-err` : undefined}
        onChange={(e) => setLabel(e.target.value)}
      />
      {tooLong && (
        <p id={`${id}-err`} className="field-error" aria-live="polite">
          {t('Use up to 40 characters.')}
        </p>
      )}
      <fieldset className="swatches">
        <legend>{t('Colour')}</legend>
        {LANES.map((l) => (
          <label
            key={l}
            className="swatch"
            style={{ '--lane': `var(--lane-${l})` } as React.CSSProperties}
          >
            <input
              type="radio"
              name={`${id}-lane`}
              value={l}
              checked={color === l}
              onChange={() => setColor(l)}
            />
            <span className="sr-only">{t(LANE_NAME[l])}</span>
          </label>
        ))}
      </fieldset>
      <div className="net-editor-foot">
        <button
          type="button"
          className="btn btn-ghost"
          onClick={async () => {
            if (
              await save({
                name: net.name,
                label: null,
                lane: null,
                useFor: current?.useFor ?? 'always',
                hours: current?.hours ?? null,
              })
            )
              onDone()
          }}
        >
          {t('Reset')}
        </button>
        <button type="button" className="btn btn-ghost" onClick={onDone}>
          {t('Cancel')}
        </button>
        <button type="submit" className="btn btn-primary" disabled={tooLong}>
          {t('Save')}
        </button>
      </div>
    </form>
  )
}

const hhmm = (m: number) =>
  `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
const minutes = (value: string) => {
  const [h, m] = value.split(':').map(Number)
  return (h ?? 0) * 60 + (m ?? 0)
}

/** tr()'s output with each run of words in a <span>, as items of a flex row. */
function spans(nodes: ReactNode): ReactNode {
  if (!Array.isArray(nodes)) return nodes
  return nodes.map((n: ReactNode, i) =>
    typeof n === 'string' ? n.trim() ? <span key={`w${i}`}>{n.trim()}</span> : null : n,
  )
}

/** "Only from 23:00 to 06:00" for one network (B10.6), e.g. a night data plan. */
function Hours({
  name,
  hours,
  onChange,
}: {
  name: string
  hours: { start: number; stop: number } | null
  onChange: (h: { start: number; stop: number } | null) => void
}) {
  return (
    <div className="net-hours">
      <label className="check">
        <input
          type="checkbox"
          checked={hours !== null}
          onChange={(e) => onChange(e.target.checked ? { start: 23 * 60, stop: 6 * 60 } : null)}
        />
        <span>{t('Only from')}</span>
      </label>
      {spans(
        tr('{start} to {end}', {
          start: (
            <input
              type="time"
              aria-label={t('{name} from', { name })}
              disabled={!hours}
              value={hhmm(hours?.start ?? 23 * 60)}
              onChange={(e) => hours && onChange({ ...hours, start: minutes(e.target.value) })}
            />
          ),
          end: (
            <input
              type="time"
              aria-label={t('{name} until', { name })}
              disabled={!hours}
              value={hhmm(hours?.stop ?? 6 * 60)}
              onChange={(e) => hours && onChange({ ...hours, stop: minutes(e.target.value) })}
            />
          ),
        }),
      )}
      {/* Daily data packs expire at midnight (8.1): the phone helps in the last two
          hours with what's left of today's data; if the pack runs out, the carrier's
          slowdown is caught by throttle detection (8.2). */}
      <button
        type="button"
        className="link-btn"
        aria-label={t('{name}: only before midnight, 22:00 to 00:00', { name })}
        title={t(
          "22:00 to midnight. Daily data packs expire at midnight, so the phone helps with what's left of today's data. If it runs out, Fuselane notices the slowdown and stops using it.",
        )}
        onClick={() => onChange({ start: 22 * 60, stop: 0 })}
      >
        {t('Before midnight')}
      </button>
    </div>
  )
}

const USES: { value: NetUse; label: string }[] = [
  { value: 'always', label: mark('Always') },
  { value: 'long', label: mark('Long downloads') },
  { value: 'never', label: mark('Never') },
]

export function NetworkList({ compact = false }: { compact?: boolean }) {
  const networks = useApp((s) => s.networks)
  useApp((s) => s.netPrefs) // re-render when names or colours change
  const rates = useLiveRates()
  const [editing, setEditing] = useState<string | null>(null)
  const usable = networks.filter((n) => n.usable)
  const lanes = assignLanes(usable)
  const max = Math.max(1, ...Object.values(rates))
  if (!networks.length)
    return (
      <p className="muted">
        {t('No networks found. Join Wi-Fi, plug in Ethernet, or tether a phone over USB.')}
      </p>
    )
  return (
    <ul className={compact ? 'netlist compact' : 'netlist'}>
      {usable.map((n, i) => {
        const r = rates[n.name] ?? 0
        return (
          <li key={n.name} className={editing === n.name ? 'is-editing' : undefined}>
            <div className="netlist-row">
              <Orb lane={lanes[i] ?? 'steel'} speed={r / max} state={r > 0 ? 'live' : 'idle'} />
              <span className="netlist-text">
                <span className="net-name" translate="no">
                  {netTitle(n)}
                </span>
                <span className="net-kind num" data-reach={n.reach ?? undefined}>
                  {n.reach === 'portal'
                    ? t('Sign in needed')
                    : r > 0
                      ? rateText(r)
                      : compact
                        ? t('Ready')
                        : `${kindLabel(n.kind)}, ${n.name}`}
                </span>
              </span>
              {!compact && (
                <>
                  <NetIcon kind={n.kind} />
                  <button
                    className="icon-btn"
                    aria-label={t('Rename or recolour {name}', { name: netTitle(n) })}
                    title={t('Rename or recolour')}
                    aria-expanded={editing === n.name}
                    onClick={() => setEditing(editing === n.name ? null : n.name)}
                  >
                    <PencilSimple size={16} aria-hidden />
                  </button>
                </>
              )}
            </div>
            {n.reach === 'portal' && !compact && <SignIn net={n} />}
            {editing === n.name && (
              <NetEditor net={n} lane={lanes[i] ?? 'steel'} onDone={() => setEditing(null)} />
            )}
          </li>
        )
      })}
    </ul>
  )
}

const GB = 1024 ** 3
const MB = 1024 ** 2

function shortDate(iso: string): string {
  const [y, m, d] = iso.split('-').map(Number)
  if (!y || !m || !d) return iso
  return new Intl.DateTimeFormat(intlLocale(), { day: 'numeric', month: 'short' }).format(
    new Date(y, m - 1, d),
  )
}

/** One network's monthly allowance: amount, reset day, and how much is used. */
function AllowanceRow({
  view,
  onSaved,
}: {
  view: AllowanceView
  onSaved: (v: AllowanceView[]) => void
}) {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const networks = useApp((s) => s.networks)
  const net = networks.find((x) => x.name === view.name)
  const startUnit = view.allowance && view.allowance < GB ? 'MB' : 'GB'
  const [unit, setUnit] = useState<'MB' | 'GB'>(startUnit)
  const [text, setText] = useState(
    view.allowance ? String(+(view.allowance / (startUnit === 'GB' ? GB : MB)).toFixed(2)) : '',
  )
  const [day, setDay] = useState(view.resetDay)
  const trimmed = text.trim().replace(',', '.')
  const valid = trimmed === '' || /^\d+(\.\d+)?$/.test(trimmed)
  const bytes = trimmed === '' ? 0 : Math.round(Number(trimmed) * (unit === 'GB' ? GB : MB))
  const changed = valid && (bytes !== (view.allowance ?? 0) || day !== view.resetDay)
  const title = net ? netTitle(net) : view.name
  const pct = view.allowance ? Math.min(100, (view.used / view.allowance) * 100) : 0
  const id = `allow-${view.name}`
  return (
    <li className="allowance" data-reached={view.reached || undefined}>
      <div className="allowance-head">
        <span className="net-name" translate="no">
          {title}
        </span>
        <span className="muted num">
          {view.allowance
            ? t('{used} of {allowance}, resets {date}', {
                used: bytesText(view.used),
                allowance: bytesText(view.allowance),
                date: shortDate(view.resetsOn),
              })
            : t('{used} used, resets {date}', {
                used: bytesText(view.used),
                date: shortDate(view.resetsOn),
              })}
        </span>
      </div>
      {view.allowance ? (
        <div className="bar allowance-bar" aria-hidden="true">
          <div className="bar-fill" style={{ width: `${pct}%` }} />
        </div>
      ) : null}
      {view.reached && (
        <p className="allowance-note">
          {t("Allowance reached: Fuselane won't use this network until {date}.", {
            date: shortDate(view.resetsOn),
          })}
        </p>
      )}
      <form
        className="allowance-form"
        onSubmit={(e) => {
          e.preventDefault()
          if (!valid || !backend) return
          void act(async (b) =>
            onSaved(await b.setAllowance({ name: view.name, bytes, resetDay: day })),
          )
        }}
      >
        <label htmlFor={id} className="sr-only">
          {t('{name} monthly allowance', { name: title })}
        </label>
        <div className="limit-inputs">
          <input
            id={id}
            inputMode="decimal"
            autoComplete="off"
            placeholder={t('No limit')}
            value={text}
            aria-invalid={valid ? undefined : true}
            onChange={(e) => setText(e.target.value)}
          />
          <select
            aria-label={t('{name} allowance unit', { name: title })}
            value={unit}
            onChange={(e) => setUnit(e.target.value as 'MB' | 'GB')}
          >
            <option value="MB">MB</option>
            <option value="GB">GB</option>
          </select>
        </div>
        <label className="reset-day">
          <span className="muted">{t('Resets on day')}</span>
          <select
            aria-label={t('{name} reset day', { name: title })}
            value={day}
            onChange={(e) => setDay(Number(e.target.value))}
          >
            {Array.from({ length: 28 }, (_, i) => i + 1).map((d) => (
              <option key={d} value={d}>
                {d}
              </option>
            ))}
          </select>
        </label>
        <button type="submit" className="btn" disabled={!changed}>
          {t('Save')}
        </button>
      </form>
      {!valid && <p className="field-error">{t('Enter a number, like 5 or 2.5.')}</p>}
    </li>
  )
}

/**
 * The long-download threshold (B9.5), shown once any network waits for long
 * downloads.
 */
function LongMinutes() {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const [minutes, setMinutes] = useState<number | null>(null)
  const [draft, setDraft] = useState('')
  useEffect(() => {
    void backend?.longMinutes().then((m) => {
      setMinutes(m)
      setDraft(String(m))
    })
  }, [backend])
  if (minutes === null) return null
  return (
    <form
      className="long-minutes"
      onSubmit={(e) => {
        e.preventDefault()
        const m = Number(draft)
        void act(async (b) => setMinutes(await b.setLongMinutes(m)))
      }}
    >
      <label htmlFor="long-min">{t('A long download takes more than')}</label>
      <input
        id="long-min"
        className="num"
        inputMode="numeric"
        value={draft}
        onChange={(e) => setDraft(e.target.value.replace(/\D/g, '').slice(0, 3))}
        aria-describedby="long-help"
      />
      <span>{t('minutes without them.')}</span>
      <button type="submit" className="btn" disabled={draft === String(minutes) || !draft}>
        {t('Save')}
      </button>
      <p id="long-help" className="field-help">
        {t(
          "Downloads start without them; once a download's speed shows it's long, they join in and it carries on where it was.",
        )}
      </p>
    </form>
  )
}

/** When this network helps (B9.5), and the hours it may be used (B10.6). */
function UseChoice({ net }: { net: NetView }) {
  const prefs = useApp((s) => s.netPrefs)
  const save = useApp((s) => s.saveNetPref)
  const p = prefs.find((x) => x.name === net.name)
  const value = p?.useFor ?? 'always'
  const id = `use-${net.name}`
  const put = (useFor: NetUse, hours: { start: number; stop: number } | null) =>
    void save({ name: net.name, label: p?.label ?? null, lane: p?.lane ?? null, useFor, hours })
  return (
    <>
      <span className="sr-only" id={id}>
        {netTitle(net)}
      </span>
      <div className="segmented" role="radiogroup" aria-labelledby={id}>
        {USES.map((u) => (
          <button
            key={u.value}
            type="button"
            role="radio"
            aria-checked={value === u.value}
            onClick={() => put(u.value, p?.hours ?? null)}
          >
            {t(u.label)}
          </button>
        ))}
      </div>
      <Hours name={netTitle(net)} hours={p?.hours ?? null} onChange={(h) => put(value, h)} />
    </>
  )
}

/** This network's speed limit, saved on its own (other networks' limits kept). */
function NetLimit({ net }: { net: NetView }) {
  const limits = useApp((s) => s.limits)
  const save = useApp((s) => s.saveLimits)
  const current = limits.networks.find((l) => l.name === net.name)?.rate ?? 0
  const [draft, setDraft] = useState<number | null | undefined>(undefined)
  const [status, setStatus] = useState('')
  const value = draft === undefined ? current : draft
  const title = netTitle(net)
  return (
    <form
      className="net-limit"
      onSubmit={async (e) => {
        e.preventDefault()
        if (value === null) return
        setStatus('')
        const others = limits.networks.filter((l) => l.name !== net.name)
        const next = value > 0 ? [...others, { name: net.name, rate: value }] : others
        if (await save({ ...limits, networks: next })) {
          setDraft(undefined)
          setStatus(value > 0 ? t('Saved. Running downloads follow it now.') : t('Limit removed.'))
        }
      }}
    >
      <LimitField
        label={t('{name} speed limit', { name: title })}
        hideLabel
        rate={current}
        onChange={(r) => setDraft(r)}
      />
      <button
        type="submit"
        className="btn btn-sm"
        disabled={value === null || value === current}
        aria-label={t('Save the speed limit for {name}', { name: title })}
      >
        {t('Save')}
      </button>
      <p className="field-help net-limit-status" role="status">
        {status}
      </p>
    </form>
  )
}

/** Allowances for every network, kept fresh. */
function useAllowances(): [AllowanceView[], (v: AllowanceView[]) => void] {
  const backend = useApp((s) => s.backend)
  const [views, setViews] = useState<AllowanceView[]>([])
  useEffect(() => {
    if (!backend) return
    let live = true
    const load = () =>
      backend
        .allowances()
        .then((v) => live && setViews(v))
        .catch(() => {})
    void load()
    const timer = setInterval(load, 5000)
    return () => {
      live = false
      clearInterval(timer)
    }
  }, [backend])
  return [views, setViews]
}

/** A ring that fills with the share of an allowance used. */
function Ring({ share }: { share: number }) {
  const r = 15
  const c = 2 * Math.PI * r
  return (
    <svg className="allow-ring" viewBox="0 0 40 40" aria-hidden>
      <circle cx="20" cy="20" r={r} className="allow-ring-track" />
      <circle
        cx="20"
        cy="20"
        r={r}
        className="allow-ring-fill"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - Math.min(1, share))}
      />
    </svg>
  )
}

/**
 * The live picture at the top: every network as a lane flowing into Fuselane,
 * the flow quicker the more it carries, the total in the core.
 */
function FlowHero({
  nets,
  lanes,
  rates,
}: {
  nets: NetView[]
  lanes: Lane[]
  rates: Record<string, number>
}) {
  const total = nets.reduce((a, n) => a + (rates[n.name] ?? 0), 0)
  const max = Math.max(1, ...nets.map((n) => rates[n.name] ?? 0))
  const row = 64
  const h = Math.max(1, nets.length) * row
  return (
    <section className="flow" aria-label={t('Your networks, joined')}>
      <ul className="flow-nets">
        {nets.map((n, i) => {
          const r = rates[n.name] ?? 0
          return (
            <li
              key={n.name}
              style={{ '--lane': `var(--lane-${lanes[i] ?? 'steel'})` } as React.CSSProperties}
              data-on={r > 0 || undefined}
              data-reach={n.reach ?? undefined}
            >
              <span className="flow-icon" aria-hidden>
                <NetIcon kind={n.kind} />
              </span>
              <span className="flow-name" translate="no">
                {netTitle(n)}
              </span>
              <span className="flow-rate num">
                {n.reach === 'portal' ? t('Sign in needed') : r > 0 ? rateText(r) : t('Ready')}
              </span>
            </li>
          )
        })}
      </ul>
      <svg
        className="flow-lines"
        viewBox={`0 0 400 ${h}`}
        preserveAspectRatio="none"
        style={{ height: h }}
        aria-hidden
      >
        {nets.map((n, i) => {
          const y = i * row + row / 2
          const d = `M14 ${y} C 200 ${y}, 200 ${h / 2}, 400 ${h / 2}`
          const r = rates[n.name] ?? 0
          const lane = `var(--lane-${lanes[i] ?? 'steel'})`
          return (
            <g key={n.name} data-on={r > 0 || undefined}>
              <path d={d} className="flow-base" style={{ stroke: lane }} />
              <path
                d={d}
                className="flow-dash"
                style={
                  {
                    stroke: lane,
                    '--flow-dur': `${r > 0 ? 2.6 - 1.9 * (r / max) : 6}s`,
                  } as React.CSSProperties
                }
              />
            </g>
          )
        })}
      </svg>
      <div className="flow-core" data-on={total > 0 || undefined}>
        <span className="flow-core-ring" aria-hidden />
        <span className="flow-core-label">{t('Together')}</span>
        <span className="flow-core-rate num">{total > 0 ? rateText(total) : t('Idle')}</span>
        <span className="flow-core-count">
          {nets.length === 1 ? t('1 network') : t('{n} networks', { n: nets.length })}
        </span>
      </div>
    </section>
  )
}

/** Everything about one network in one place. */
function NetworkCard({
  net,
  lane,
  rate,
  max,
  allowance,
  onAllowance,
}: {
  net: NetView
  lane: Lane
  rate: number
  max: number
  allowance: AllowanceView | undefined
  onAllowance: (v: AllowanceView[]) => void
}) {
  const prefs = useApp((s) => s.netPrefs)
  const [editing, setEditing] = useState(false)
  const title = netTitle(net)
  const share = allowance?.allowance ? allowance.used / allowance.allowance : null
  return (
    <article
      className="net-card"
      style={{ '--lane': `var(--lane-${lane})` } as React.CSSProperties}
      data-on={rate > 0 || undefined}
      aria-labelledby={`card-${net.name}`}
    >
      <header className="net-card-head">
        <Orb lane={lane} speed={rate / max} state={rate > 0 ? 'live' : 'idle'} />
        <span className="net-card-title">
          <span className="net-name" id={`card-${net.name}`} translate="no">
            {title}
          </span>
          <span className="net-kind">
            {kindLabel(net.kind)}, <span translate="no">{net.name}</span>
          </span>
        </span>
        <span className="net-card-rate num" data-reach={net.reach ?? undefined}>
          {net.reach === 'portal' ? t('Sign in needed') : rate > 0 ? rateText(rate) : t('Ready')}
        </span>
        <button
          className="icon-btn"
          aria-label={t('Rename or recolour {name}', { name: title })}
          title={t('Rename or recolour')}
          aria-expanded={editing}
          onClick={() => setEditing((e) => !e)}
        >
          <PencilSimple size={16} aria-hidden />
        </button>
      </header>
      {net.reach === 'portal' && <SignIn net={net} />}
      {editing && <NetEditor net={net} lane={lane} onDone={() => setEditing(false)} />}
      <div className="net-card-body">
        <div className="net-card-row">
          <span className="net-card-label">
            <Lightning size={15} aria-hidden /> {t('Helps with')}
          </span>
          <div className="net-card-field">
            <UseChoice net={net} />
          </div>
        </div>
        <div className="net-card-row">
          <span className="net-card-label">
            <Gauge size={15} aria-hidden /> {t('Speed limit')}
          </span>
          <div className="net-card-field">
            <NetLimit net={net} />
          </div>
        </div>
        {allowance && (
          <div className="net-card-row">
            <span className="net-card-label">
              <ChartPieSlice size={15} aria-hidden /> {t('Monthly data')}
            </span>
            <div className="net-card-field net-card-allow">
              {share !== null && <Ring share={share} />}
              <ul className="allowance-list">
                <AllowanceRow
                  key={`${allowance.name}-${allowance.allowance}-${allowance.resetDay}`}
                  view={allowance}
                  onSaved={onAllowance}
                />
              </ul>
            </div>
          </div>
        )}
        <div className="net-card-row">
          <span className="net-card-label">
            <ShieldCheck size={15} aria-hidden /> {t('Proxy')}
          </span>
          <div className="net-card-field">
            <ul className="proxy-list">
              <ProxyRow net={net} pref={prefs.find((p) => p.name === net.name)} />
            </ul>
          </div>
        </div>
      </div>
    </article>
  )
}

/** Networks → Setup: the live flow, then one card per network. */
function NetworkSetup() {
  const networks = useApp((s) => s.networks)
  const prefs = useApp((s) => s.netPrefs)
  const rates = useLiveRates()
  const [allowances, setAllowances] = useAllowances()
  const usable = networks.filter((n) => n.usable)
  const lanes = assignLanes(usable)
  const max = Math.max(1, ...Object.values(rates))
  const anyLong = usable.some((n) => prefs.find((p) => p.name === n.name)?.useFor === 'long')
  if (!usable.length)
    return (
      <p className="muted net-none">
        {t('No networks found. Join Wi-Fi, plug in Ethernet, or tether a phone over USB.')}
      </p>
    )
  return (
    <>
      <FlowHero nets={usable} lanes={lanes} rates={rates} />
      <div className="net-cards">
        {usable.map((n, i) => (
          <NetworkCard
            key={n.name}
            net={n}
            lane={lanes[i] ?? 'steel'}
            rate={rates[n.name] ?? 0}
            max={max}
            allowance={allowances.find((a) => a.name === n.name)}
            onAllowance={setAllowances}
          />
        ))}
      </div>
      {anyLong && <LongMinutes />}
      <p className="field-help net-notes">
        {t(
          'Helps with: a phone on a data plan can wait for the downloads where it makes a real difference. Speed limit: cap a network and the others carry the rest. Monthly data: when a network reaches its allowance, Fuselane stops using it until the reset day.',
        )}{' '}
        {t(
          "Proxy: for a network that only reaches the internet through one; https stays encrypted end to end. Torrents don't use these proxies, and proxy settings from your system aren't used, only the ones set here.",
        )}
      </p>
    </>
  )
}

type NetTab = 'setup' | 'usage'
const NET_TABS: [NetTab, string][] = [
  ['setup', mark('Setup')],
  ['usage', mark('Usage')],
]

export function NetworksView() {
  const networks = useApp((s) => s.networks)
  const refresh = useApp((s) => s.refreshNetworks)
  useEffect(() => {
    void refresh()
    const timer = setInterval(() => void refresh(), 10000)
    return () => clearInterval(timer)
  }, [refresh])
  const other = networks.filter((n) => !n.usable)
  // Setup (networks, limits, when each helps, allowances) and usage.
  const [tab, setTab] = useState<NetTab>(() => {
    try {
      const saved = sessionStorage.getItem('fuselane.netTab')
      return saved === 'usage' ? 'usage' : 'setup'
    } catch {
      return 'setup'
    }
  })
  const pick = (next: NetTab) => {
    setTab(next)
    try {
      sessionStorage.setItem('fuselane.netTab', next)
    } catch {
      /* private mode: the tab lasts while the page is open */
    }
  }
  return (
    <section className="page" aria-labelledby="nets-title">
      <header className="page-head with-tabs">
        <h1 id="nets-title">{t('Networks')}</h1>
        <div
          className="segmented"
          role="radiogroup"
          aria-label={t('Networks view')}
          onKeyDown={(e) => {
            if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return
            e.preventDefault()
            const i = NET_TABS.findIndex(([id]) => id === tab)
            const next =
              NET_TABS[
                (i + (e.key === 'ArrowRight' ? 1 : NET_TABS.length - 1)) % NET_TABS.length
              ]![0]
            pick(next)
            e.currentTarget.querySelector<HTMLButtonElement>(`[data-id="${next}"]`)?.focus()
          }}
        >
          {NET_TABS.map(([id, label]) => (
            <button
              key={id}
              type="button"
              role="radio"
              data-id={id}
              aria-checked={tab === id}
              tabIndex={tab === id ? 0 : -1}
              onClick={() => pick(id)}
            >
              {t(label)}
            </button>
          ))}
        </div>
        <div className="page-head-end">
          <button className="btn btn-ghost" onClick={() => void refresh()}>
            <ArrowClockwise size={16} aria-hidden /> {t('Refresh')}
          </button>
        </div>
      </header>
      {tab === 'usage' && <DataUsed />}
      {tab === 'setup' && (
        <>
          <NetworkSetup />
          {other.length > 0 && (
            <details className="other-nets">
              <summary>{t('Not used ({n})', { n: other.length })}</summary>
              <ul className="netlist">
                {other.map((n) => (
                  <li key={n.name} data-down>
                    <span className="orb-slot">
                      <NetIcon kind={n.kind} />
                    </span>
                    <span className="netlist-text">
                      <span className="net-name" translate="no">
                        {netTitle(n)}
                      </span>
                      <span className="net-kind">
                        {kindLabel(n.kind)}, <span translate="no">{n.name}</span>.{' '}
                        {n.kind === 'vpn'
                          ? t('Tunnels are skipped so traffic stays where you expect.')
                          : t('Not connected to the internet.')}
                      </span>
                    </span>
                  </li>
                ))}
              </ul>
            </details>
          )}
        </>
      )}
    </section>
  )
}
