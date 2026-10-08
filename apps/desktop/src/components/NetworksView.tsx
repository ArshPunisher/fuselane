import { useEffect, useState } from 'react'
import { ArrowClockwise, PencilSimple } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { assignLanes, kindLabel, LANES, netTitle, type Lane } from '../lib/lanes'
import type { AllowanceView, NetView } from '../lib/types'
import { bytes as bytesText, rateText } from '../lib/format'
import { NetIcon } from './NetIcon'
import { Orb } from './Orb'
import { LimitField } from './LimitField'

/** Live speed per network, summed over running downloads. */
function useLiveRates(): Record<string, number> {
  const live = useApp((s) => s.live)
  const jobs = useApp((s) => s.jobs)
  const running = new Set(jobs.filter((j) => j.status === 'running').map((j) => j.id))
  const out: Record<string, number> = {}
  for (const l of Object.values(live)) {
    if (!running.has(l.id)) continue
    for (const n of l.networks) out[n.name] = (out[n.name] ?? 0) + (n.dead ? 0 : n.rate)
  }
  return out
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
        if (await save({ name: net.name, label: label.trim() || null, lane: color })) onDone()
      }}
    >
      <label htmlFor={id}>Name</label>
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
          Use up to 40 characters.
        </p>
      )}
      <fieldset className="swatches">
        <legend>Colour</legend>
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
            <span className="sr-only">{l}</span>
          </label>
        ))}
      </fieldset>
      <div className="net-editor-foot">
        <button
          type="button"
          className="btn btn-ghost"
          onClick={async () => {
            if (await save({ name: net.name, label: null, lane: null })) onDone()
          }}
        >
          Reset
        </button>
        <button type="button" className="btn btn-ghost" onClick={onDone}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={tooLong}>
          Save
        </button>
      </div>
    </form>
  )
}

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
        No networks found. Join Wi-Fi, plug in Ethernet, or tether a phone over USB.
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
                <span className="net-kind num">
                  {r > 0 ? rateText(r) : compact ? 'Ready' : `${kindLabel(n.kind)}, ${n.name}`}
                </span>
              </span>
              {!compact && (
                <>
                  <NetIcon kind={n.kind} />
                  <button
                    className="icon-btn"
                    aria-label={`Rename or recolour ${netTitle(n)}`}
                    title="Rename or recolour"
                    aria-expanded={editing === n.name}
                    onClick={() => setEditing(editing === n.name ? null : n.name)}
                  >
                    <PencilSimple size={16} aria-hidden />
                  </button>
                </>
              )}
            </div>
            {editing === n.name && (
              <NetEditor net={n} lane={lanes[i] ?? 'steel'} onDone={() => setEditing(null)} />
            )}
          </li>
        )
      })}
    </ul>
  )
}

/** One limit per usable network, saved together. */
function NetworkLimits() {
  const networks = useApp((s) => s.networks).filter((n) => n.usable)
  const limits = useApp((s) => s.limits)
  const save = useApp((s) => s.saveLimits)
  const current = (name: string) => limits.networks.find((l) => l.name === name)?.rate ?? 0
  const [draft, setDraft] = useState<Record<string, number | null>>({})
  const [status, setStatus] = useState('')
  const value = (name: string) => (name in draft ? draft[name] : current(name))
  const invalid = networks.some((n) => value(n.name) === null)
  const changed = networks.some((n) => value(n.name) !== current(n.name))
  if (!networks.length) return null
  return (
    <form
      className="net-limits"
      aria-labelledby="limits-title"
      onSubmit={async (e) => {
        e.preventDefault()
        if (invalid) return
        setStatus('')
        const next = networks
          .map((n) => ({ name: n.name, rate: value(n.name) ?? 0 }))
          .filter((n) => n.rate > 0)
        // Keep limits for networks that aren't connected right now.
        const others = limits.networks.filter((l) => !networks.some((n) => n.name === l.name))
        if (await save({ ...limits, networks: [...others, ...next] })) {
          setDraft({})
          setStatus('Saved. Running downloads follow these now.')
        }
      }}
    >
      <h2 id="limits-title" className="section-title">
        Speed limit per network
      </h2>
      <p className="muted">
        Useful for a phone on a data plan: cap it, and the other networks carry the rest.
      </p>
      <ul className="limit-list">
        {networks.map((n) => (
          <li key={n.name}>
            <span className="net-name">{netTitle(n)}</span>
            <LimitField
              label={`${netTitle(n)} speed limit`}
              hideLabel
              rate={current(n.name)}
              onChange={(r) => setDraft((d) => ({ ...d, [n.name]: r }))}
            />
          </li>
        ))}
      </ul>
      <div className="net-limits-foot">
        <p className="muted" role="status">
          {status}
        </p>
        <button type="submit" className="btn" disabled={invalid || !changed}>
          Save limits
        </button>
      </div>
    </form>
  )
}

const GB = 1024 ** 3
const MB = 1024 ** 2

function shortDate(iso: string): string {
  const [y, m, d] = iso.split('-').map(Number)
  if (!y || !m || !d) return iso
  return new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short' }).format(
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
        <span className="net-name">{title}</span>
        <span className="muted num">
          {view.allowance
            ? `${bytesText(view.used)} of ${bytesText(view.allowance)}`
            : `${bytesText(view.used)} used`}
          , resets {shortDate(view.resetsOn)}
        </span>
      </div>
      {view.allowance ? (
        <div className="bar allowance-bar" aria-hidden="true">
          <div className="bar-fill" style={{ width: `${pct}%` }} />
        </div>
      ) : null}
      {view.reached && (
        <p className="allowance-note">
          Allowance reached: Fuselane won&apos;t use this network until {shortDate(view.resetsOn)}.
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
          {title} monthly allowance
        </label>
        <div className="limit-inputs">
          <input
            id={id}
            inputMode="decimal"
            autoComplete="off"
            placeholder="No limit"
            value={text}
            aria-invalid={valid ? undefined : true}
            onChange={(e) => setText(e.target.value)}
          />
          <select
            aria-label={`${title} allowance unit`}
            value={unit}
            onChange={(e) => setUnit(e.target.value as 'MB' | 'GB')}
          >
            <option value="MB">MB</option>
            <option value="GB">GB</option>
          </select>
        </div>
        <label className="reset-day">
          <span className="muted">Resets on day</span>
          <select
            aria-label={`${title} reset day`}
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
          Save
        </button>
      </form>
      {!valid && <p className="field-error">Enter a number, like 5 or 2.5.</p>}
    </li>
  )
}

function Allowances() {
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
    const t = setInterval(load, 5000)
    return () => {
      live = false
      clearInterval(t)
    }
  }, [backend])
  if (!views.length) return null
  return (
    <section className="net-limits" aria-labelledby="allow-title">
      <h2 id="allow-title" className="section-title">
        Monthly data allowance
      </h2>
      <p className="muted">
        For a phone on a data plan: when a network reaches its allowance, Fuselane stops using it
        until the reset day.
      </p>
      <ul className="allowance-list">
        {views.map((v) => (
          <AllowanceRow
            key={`${v.name}-${v.allowance}-${v.resetDay}`}
            view={v}
            onSaved={setViews}
          />
        ))}
      </ul>
    </section>
  )
}

export function NetworksView() {
  const networks = useApp((s) => s.networks)
  const refresh = useApp((s) => s.refreshNetworks)
  useEffect(() => {
    void refresh()
    const t = setInterval(() => void refresh(), 10000)
    return () => clearInterval(t)
  }, [refresh])
  const other = networks.filter((n) => !n.usable)
  return (
    <section className="page" aria-labelledby="nets-title">
      <header className="page-head">
        <h1 id="nets-title">Networks</h1>
        <button className="btn btn-ghost" onClick={() => void refresh()}>
          <ArrowClockwise size={16} aria-hidden /> Refresh
        </button>
      </header>
      <p className="page-lead">
        Every network here can carry part of each download. Plug in a phone or join another network
        and it joins in.
      </p>
      <NetworkList />
      <NetworkLimits />
      <Allowances />
      {other.length > 0 && (
        <details className="other-nets">
          <summary>Not used ({other.length})</summary>
          <ul className="netlist">
            {other.map((n) => (
              <li key={n.name} data-down>
                <span className="orb-slot">
                  <NetIcon kind={n.kind} />
                </span>
                <span className="netlist-text">
                  <span className="net-name">{netTitle(n)}</span>
                  <span className="net-kind">
                    {kindLabel(n.kind)}, {n.name}.{' '}
                    {n.kind === 'vpn'
                      ? 'Tunnels are skipped so traffic stays where you expect.'
                      : 'Not connected to the internet.'}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  )
}
