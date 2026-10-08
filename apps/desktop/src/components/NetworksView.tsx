import { useEffect, useState } from 'react'
import { ArrowClockwise } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { assignLanes, kindLabel, netTitle } from '../lib/lanes'
import { rateText } from '../lib/format'
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

export function NetworkList({ compact = false }: { compact?: boolean }) {
  const networks = useApp((s) => s.networks)
  const rates = useLiveRates()
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
          <li key={n.name}>
            <Orb lane={lanes[i] ?? 'steel'} speed={r / max} state={r > 0 ? 'live' : 'idle'} />
            <span className="netlist-text">
              <span className="net-name">{netTitle(n)}</span>
              <span className="net-kind num">
                {r > 0 ? rateText(r) : compact ? 'Ready' : `${kindLabel(n.kind)}, ${n.name}`}
              </span>
            </span>
            {!compact && <NetIcon kind={n.kind} />}
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
