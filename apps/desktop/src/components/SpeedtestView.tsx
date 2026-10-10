import { useEffect } from 'react'
import { ArrowDown, ArrowUp, FileText, Lightning, Stop } from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import type { CheckRun, NetResult, Outage, SpeedLive } from '../lib/types'
import { intlLocale, t, tb, tr } from '../lib/i18n'
import { NetIcon } from './NetIcon'
import { SpeedGauge, SpeedTrace, mbpsOf, speedText } from './SpeedGauge'

const ms = (v: number | null | undefined) =>
  v === null || v === undefined ? '–' : t('{n} ms', { n: Math.round(v) })
const mb = (bps: number | null | undefined) =>
  bps === null || bps === undefined ? '–' : speedText(mbpsOf(bps))

function when(unix: number): string {
  return new Date(unix * 1000).toLocaleString(intlLocale(), {
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function dataUsed(bytes: number): string {
  return bytes >= 1e9
    ? `${(bytes / 1e9).toFixed(1)} GB`
    : `${Math.max(1, Math.round(bytes / 1e6))} MB`
}

/** Plain words for a result, so the numbers mean something. */
function verdict(r: NetResult): string {
  if (r.problem) return tb(r.problem)
  const g = r.grade ?? ''
  if (g === 'D' || g === 'F')
    return t('Gets very slow while busy: calls and games stutter when something downloads.')
  if (g === 'C') return t('Slows down while busy: a big download makes calls lag.')
  if ((r.loss ?? 0) > 0.1) return t('Drops some connections.')
  if ((r.jitterMs ?? 0) > 30) return t('Uneven: fine for downloads, choppy for calls.')
  return t('Healthy.')
}

const STEPS: { id: SpeedLive['step']; label: string; Icon: typeof ArrowDown }[] = [
  { id: 'ping', label: 'Ping', Icon: Lightning },
  { id: 'down', label: 'Download', Icon: ArrowDown },
  { id: 'up', label: 'Upload', Icon: ArrowUp },
  { id: 'together', label: 'Together', Icon: Lightning },
]

/** Where the run is: Ping, Download, Upload, Together, the current one lit. */
function StepRail({ live, many }: { live: SpeedLive | null; many: boolean }) {
  const steps = many ? STEPS : STEPS.slice(0, 3)
  const at = live ? steps.findIndex((s) => s.id === live.step) : -1
  return (
    <ol className="step-rail" aria-label={t('Steps')}>
      {steps.map(({ id, label }, i) => (
        <li
          key={id}
          data-state={i < at ? 'done' : i === at ? 'now' : undefined}
          aria-current={i === at ? 'step' : undefined}
        >
          <span className="step-dot" aria-hidden>
            {i === at && live && (
              <span className="step-fill" style={{ transform: `scaleX(${live.progress})` }} />
            )}
          </span>
          {t(label)}
        </li>
      ))}
    </ol>
  )
}

function NetCard({ r, lane }: { r: NetResult; lane: number }) {
  return (
    <article
      className="speed-card"
      style={{ '--lane': `var(--lane-${LANES[lane % LANES.length]})` } as React.CSSProperties}
    >
      <header className="speed-card-head">
        <span className="speed-card-icon" aria-hidden>
          <NetIcon kind={r.kind} />
        </span>
        <span className="speed-card-name">
          <span translate="no">{r.label}</span>
          {(r.isp || r.server) && (
            <span className="muted speed-card-isp">
              {r.isp && <span translate="no">{r.isp}</span>}
              {r.isp && r.server ? ', ' : ''}
              {r.server && tr('server in {city}', { city: <span translate="no">{r.server}</span> })}
            </span>
          )}
        </span>
      </header>
      <div className="speed-card-big">
        <div>
          <span className="speed-card-label">
            <ArrowDown size={13} weight="bold" aria-hidden /> {t('Download')}
          </span>
          <span className="speed-card-num num">{mb(r.downBps)}</span>
          <span className="speed-card-unit">Mbps</span>
        </div>
        <div>
          <span className="speed-card-label">
            <ArrowUp size={13} weight="bold" aria-hidden /> {t('Upload')}
          </span>
          <span className="speed-card-num num">{mb(r.upBps)}</span>
          <span className="speed-card-unit">Mbps</span>
        </div>
      </div>
      <dl className="speed-card-stats">
        <div>
          <dt>{t('Ping')}</dt>
          <dd className="num">{ms(r.idleMs)}</dd>
        </div>
        <div>
          <dt>{t('Jitter')}</dt>
          <dd className="num">{ms(r.jitterMs)}</dd>
        </div>
        <div>
          <dt>{t('While busy')}</dt>
          <dd className="num">
            {ms(r.loadedMs)}{' '}
            {r.grade && (
              <span
                className="nc-grade"
                data-grade={r.grade[0]}
                title={t('How well it copes when busy, A+ to F')}
              >
                {r.grade}
              </span>
            )}
          </dd>
        </div>
        <div>
          <dt>{t('DNS')}</dt>
          <dd className="num">{ms(r.dnsMs)}</dd>
        </div>
        <div>
          <dt>{t('Loss')}</dt>
          <dd className="num">{r.loss === null ? '–' : `${Math.round((r.loss ?? 0) * 100)}%`}</dd>
        </div>
      </dl>
      <p className="speed-card-verdict">
        {verdict(r)}
        {(r.bytes ?? 0) > 0 && (
          <span className="muted"> {t('Used {size}.', { size: dataUsed(r.bytes ?? 0) })}</span>
        )}
      </p>
    </article>
  )
}

const LANES = ['tide', 'iris', 'mint', 'rose', 'sky', 'lilac', 'volt']

function Together({ run }: { run: CheckRun }) {
  if (run.togetherBps === null) return null
  const best = Math.max(0, ...run.results.map((r) => r.downBps ?? 0))
  const times = best > 0 ? run.togetherBps / best : 0
  return (
    <article className="speed-together">
      <span className="speed-together-icon" aria-hidden>
        <Lightning size={22} weight="fill" />
      </span>
      <div>
        <h3>{t('Every network together')}</h3>
        <p className="muted">{t('What Fuselane can pull in at once, for one download.')}</p>
      </div>
      <div className="speed-together-num">
        <span className="num">{mb(run.togetherBps)}</span>{' '}
        <span className="speed-card-unit">Mbps</span>
        {times >= 1.05 && (
          <span className="speed-together-x">
            {t('{x}× your fastest network', { x: times.toFixed(1) })}
          </span>
        )}
      </div>
    </article>
  )
}

function History({ runs }: { runs: CheckRun[] }) {
  if (runs.length < 2) return null
  return (
    <details className="speed-history">
      <summary>{t('Earlier tests ({n})', { n: runs.length - 1 })}</summary>
      <table>
        <thead>
          <tr>
            <th scope="col">{t('When')}</th>
            <th scope="col">{t('Network')}</th>
            <th scope="col">{t('Download (Mbps)')}</th>
            <th scope="col">{t('Upload (Mbps)')}</th>
            <th scope="col">{t('Ping (ms)')}</th>
          </tr>
        </thead>
        <tbody>
          {runs.slice(1).flatMap((run) =>
            run.results.map((r, i) => (
              <tr key={`${run.at}-${r.name}`}>
                <td>{i === 0 ? when(run.at) : ''}</td>
                <td translate="no">{r.label}</td>
                <td className="num">{mb(r.downBps)}</td>
                <td className="num">{mb(r.upBps)}</td>
                <td className="num">{r.idleMs === null ? '–' : Math.round(r.idleMs)}</td>
              </tr>
            )),
          )}
        </tbody>
      </table>
    </details>
  )
}

/** "Wi-Fi: offline from Mon 12 Oct, 14:05, 9 min". */
function outageLine(o: Outage) {
  const vars = {
    name: <span translate="no">{o.label}</span>,
    from: when(o.from),
    m: o.to === null ? '' : String(Math.max(1, Math.round((o.to - o.from) / 60))),
  }
  if (o.kind === 'sign-in')
    return o.to === null
      ? tr('{name}: sign-in page from {from}, still going on', vars)
      : tr('{name}: sign-in page from {from}, {m} min', vars)
  return o.to === null
    ? tr('{name}: offline from {from}, still going on', vars)
    : tr('{name}: offline from {from}, {m} min', vars)
}

/** "Wi-Fi: no internet 2 times this week, 9 min in total." */
function Outages({ outages }: { outages: Outage[] }) {
  const week = Date.now() / 1000 - 7 * 86400
  const recent = outages.filter((o) => o.from >= week)
  if (!recent.length)
    return <p className="muted nc-outages-none">{t('No outages in the last 7 days.')}</p>
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
          .map((e) => {
            const vars = { name: e.label, n: e.n, m: Math.max(1, Math.round(e.secs / 60)) }
            return e.n === 1
              ? t('{name}: no internet once this week, {m} min in total.', vars)
              : t('{name}: no internet {n} times this week, {m} min in total.', vars)
          })
          .join(' ')}
      </summary>
      <ul>
        {[...recent].reverse().map((o) => (
          <li key={`${o.name}-${o.from}`}>{outageLine(o)}</li>
        ))}
      </ul>
    </details>
  )
}

/**
 * Speedtest (B10.1): every network measured on its own (ping, download,
 * upload, latency while busy, DNS), then all together, on a live speedometer.
 * Also the outage log and a report to send an internet provider.
 */
export function SpeedtestView() {
  const backend = useApp((s) => s.backend)
  const view = useApp((s) => s.netCheck)
  const live = useApp((s) => s.speedLive)
  const networks = useApp((s) => s.networks)
  const act = useApp((s) => s.act)
  useEffect(() => {
    if (!backend) return
    void backend.netCheckState().then((v) => useApp.setState({ netCheck: v }))
  }, [backend])
  const running = view?.running ?? false
  const run = running ? view?.current : (view?.history[0] ?? null)
  const usable = networks.filter((n) => n.usable).length
  const many = usable > 1 || (run?.results.length ?? 0) > 1
  const settled = run
    ? mbpsOf(run.togetherBps ?? Math.max(0, ...run.results.map((r) => r.downBps ?? 0)))
    : null
  const start = () => void act(async (b) => useApp.setState({ netCheck: await b.netCheckStart() }))
  return (
    <section className="page speed-page" aria-labelledby="speed-title">
      <header className="page-head">
        <h1 id="speed-title">{t('Speedtest')}</h1>
        {(view?.history.length ?? 0) > 0 && !running && (
          <button
            className="btn btn-ghost"
            title={t(
              'A dated page with every check and outage, to send your internet provider (print it to PDF)',
            )}
            onClick={() => void act((b) => b.netCheckReport())}
          >
            <FileText size={16} aria-hidden /> {t('Report for your provider')}
          </button>
        )}
      </header>

      <div className="speed-stage" data-running={running || undefined}>
        <div className="speed-overall" aria-hidden>
          <span style={{ transform: `scaleX(${running ? (live?.overall ?? 0) : 0})` }} />
        </div>
        <SpeedGauge
          live={running ? live : null}
          settled={running ? null : settled}
          settledLabel={
            !running && run?.togetherBps !== null && run?.togetherBps !== undefined
              ? t('Every network together')
              : undefined
          }
        >
          <button className="speed-go" onClick={start} disabled={!backend}>
            <span className="speed-go-ring" aria-hidden />
            <span className="speed-go-label">{run ? t('Again') : t('Start')}</span>
          </button>
        </SpeedGauge>
        <p className="speed-now" role="status">
          {running
            ? live?.network
              ? tr('Testing {network}', { network: <strong translate="no">{live.network}</strong> })
              : (tb(view?.phase) ?? t('Starting…'))
            : run
              ? t('Last test {when}.', { when: when(run.at) })
              : t(
                  'Measures each network on its own (ping, download, upload, and how it copes when busy), then every network together.',
                )}
        </p>
        {running && (
          <SpeedTrace
            trace={live?.trace ?? []}
            step={live?.step ?? null}
            progress={live?.progress ?? 0}
          />
        )}
        {running && <StepRail live={live} many={many} />}
        {running ? (
          <button className="btn" onClick={() => void act((b) => b.netCheckCancel())}>
            <Stop size={16} aria-hidden /> {t('Stop')}
          </button>
        ) : (
          <p className="speed-fine muted">
            {t(
              'About {s} seconds. Each network runs flat out for a few seconds, the way public speed tests do, so it uses data on metered connections.',
              { s: Math.round(Math.max(1, usable) * 16.5 + (usable > 1 ? 8 : 0)) },
            )}
          </p>
        )}
      </div>

      {run && run.results.length > 0 && (
        <div className="speed-results">
          <Together run={run} />
          <div className="speed-cards">
            {run.results.map((r, i) => (
              <NetCard key={r.name} r={r} lane={i} />
            ))}
          </div>
        </div>
      )}
      {view && <History runs={view.history} />}
      {view && (
        <section className="speed-outages" aria-labelledby="speed-outages">
          <h2 id="speed-outages" className="group">
            {t('Outages')}
          </h2>
          <Outages outages={view.outages} />
        </section>
      )}
    </section>
  )
}
