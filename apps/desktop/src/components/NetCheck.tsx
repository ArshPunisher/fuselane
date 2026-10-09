import { useEffect } from 'react'
import { FileText, Gauge, Stop } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import type { CheckRun, NetResult, Outage } from '../lib/types'

const mbps = (bps: number | null) => (bps === null ? '–' : `${((bps * 8) / 1e6).toFixed(1)}`)
const ms = (v: number | null) => (v === null ? '–' : `${Math.round(v)}`)

function when(unix: number): string {
  return new Date(unix * 1000).toLocaleString([], {
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/** Plain words for a result, so the numbers mean something. */
function verdict(r: NetResult): string {
  if (r.problem) return r.problem
  const g = r.grade ?? ''
  if (g === 'D' || g === 'F')
    return 'Gets very slow while busy: calls and games stutter when something downloads.'
  if (g === 'C') return 'Slows down while busy: a big download makes calls lag.'
  if ((r.loss ?? 0) > 0.1) return 'Drops some connections.'
  if ((r.jitterMs ?? 0) > 30) return 'Uneven: fine for downloads, choppy for calls.'
  return 'Healthy.'
}

function Row({ r, best }: { r: NetResult; best: number }) {
  const share = r.downBps && best ? (r.downBps / best) * 100 : 0
  return (
    <tr>
      <th scope="row">
        <span className="nc-name" translate="no">
          {r.label}
        </span>
        <span className="nc-verdict">{verdict(r)}</span>
      </th>
      <td className="num">
        <span className="nc-speed">{mbps(r.downBps)}</span>
        <span className="nc-bar" aria-hidden>
          <span style={{ width: `${share}%` }} />
        </span>
      </td>
      <td className="num">{ms(r.idleMs)}</td>
      <td className="num">{ms(r.jitterMs)}</td>
      <td className="num">
        {ms(r.loadedMs)}{' '}
        {r.grade && (
          <span className="nc-grade" data-grade={r.grade[0]}>
            {r.grade}
          </span>
        )}
      </td>
      <td className="num">{ms(r.dnsMs)}</td>
    </tr>
  )
}

function Results({ run }: { run: CheckRun }) {
  const best = Math.max(0, run.togetherBps ?? 0, ...run.results.map((r) => r.downBps ?? 0))
  return (
    <div className="nc-table-wrap">
      <table className="nc-table">
        <thead>
          <tr>
            <th scope="col">Network</th>
            <th scope="col">Download (Mbps)</th>
            <th scope="col">Latency (ms)</th>
            <th scope="col">Jitter (ms)</th>
            <th scope="col">Under load (ms)</th>
            <th scope="col">DNS (ms)</th>
          </tr>
        </thead>
        <tbody>
          {run.results.map((r) => (
            <Row key={r.name} r={r} best={best} />
          ))}
          {run.togetherBps !== null && (
            <tr className="nc-all">
              <th scope="row">
                <span className="nc-name">Every network together</span>
                <span className="nc-verdict">What Fuselane can use at once.</span>
              </th>
              <td className="num">
                <span className="nc-speed">{mbps(run.togetherBps)}</span>
                <span className="nc-bar" aria-hidden>
                  <span style={{ width: '100%' }} />
                </span>
              </td>
              <td colSpan={4} />
            </tr>
          )}
        </tbody>
      </table>
    </div>
  )
}

/** "Wi-Fi: no internet 2 times this week, 9 min in total." */
function Outages({ outages }: { outages: Outage[] }) {
  const week = Date.now() / 1000 - 7 * 86400
  const recent = outages.filter((o) => o.from >= week)
  if (!recent.length) return <p className="muted nc-outages-none">No outages in the last 7 days.</p>
  const by = new Map<string, { label: string; n: number; secs: number }>()
  for (const o of recent) {
    const e = by.get(o.name) ?? { label: o.label, n: 0, secs: 0 }
    e.n++
    e.secs += (o.to ?? Date.now() / 1000) - o.from
    by.set(o.name, e)
  }
  return (
    <details className="nc-outages">
      <summary>
        {[...by.values()]
          .map(
            (e) =>
              `${e.label}: no internet ${e.n === 1 ? 'once' : `${e.n} times`} this week, ${Math.max(1, Math.round(e.secs / 60))} min in total`,
          )
          .join('. ')}
        .
      </summary>
      <ul>
        {[...recent].reverse().map((o) => (
          <li key={`${o.name}-${o.from}`}>
            <span translate="no">{o.label}</span>:{' '}
            {o.kind === 'sign-in' ? 'sign-in page' : 'offline'} from {when(o.from)},{' '}
            {o.to === null
              ? 'still going on'
              : `${Math.max(1, Math.round((o.to - o.from) / 60))} min`}
          </li>
        ))}
      </ul>
    </details>
  )
}

/**
 * Network check (B10.1): "why is my internet bad?", per network. Speed,
 * latency, jitter, latency under load and DNS, every network together, the
 * outage log, and a report to send an internet provider.
 */
export function NetCheck() {
  const backend = useApp((s) => s.backend)
  const view = useApp((s) => s.netCheck)
  const act = useApp((s) => s.act)
  useEffect(() => {
    if (!backend) return
    void backend.netCheckState().then((v) => useApp.setState({ netCheck: v }))
  }, [backend])
  const run = view?.running ? view.current : (view?.history[0] ?? null)
  return (
    <section className="net-limits net-check" aria-labelledby="nc-title">
      <div className="nc-head">
        <div>
          <h2 id="nc-title" className="section-title">
            Network check
          </h2>
          <p className="muted">
            Why is the internet slow? Each network is measured on its own: speed, delay, how it
            copes when busy, and name lookups. About a minute; uses roughly 25 MB per network.
          </p>
        </div>
        <div className="nc-actions">
          {view?.running ? (
            <button className="btn" onClick={() => void act((b) => b.netCheckCancel())}>
              <Stop size={16} aria-hidden /> Stop
            </button>
          ) : (
            <button
              className="btn btn-primary"
              onClick={() =>
                void act(async (b) => useApp.setState({ netCheck: await b.netCheckStart() }))
              }
            >
              <Gauge size={16} aria-hidden /> {view?.history.length ? 'Check again' : 'Run a check'}
            </button>
          )}
          {(view?.history.length ?? 0) > 0 && !view?.running && (
            <button
              className="btn"
              title="A dated page with every check and outage, to send your internet provider (print it to PDF)"
              onClick={() => void act((b) => b.netCheckReport())}
            >
              <FileText size={16} aria-hidden /> Report for your provider
            </button>
          )}
        </div>
      </div>
      <p className="nc-status muted" role="status">
        {view?.running ? (view.phase ?? 'Checking…') : run ? `Last checked ${when(run.at)}.` : ''}
      </p>
      {run && run.results.length > 0 && <Results run={run} />}
      {view && <Outages outages={view.outages} />}
    </section>
  )
}
