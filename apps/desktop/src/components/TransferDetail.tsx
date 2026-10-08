import { useEffect, useState } from 'react'
import {
  ArrowLeft,
  ArrowClockwise,
  ArrowSquareOut,
  FolderOpen,
  Pause,
  Play,
  Trash,
  WarningCircle,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { bytes, eta, percent, rate, rateText } from '../lib/format'
import { assignLanes, kindLabel, netTitle } from '../lib/lanes'
import { FuseCore } from './FuseCore'
import { Stream } from './Stream'
import { Orb } from './Orb'
import { STATUS_WORD } from './status'
import type { JobView, Live } from '../lib/types'

/** The platform's own words for showing a file in its folder. */
const REVEAL_LABEL = /Mac/i.test(navigator.platform)
  ? 'Show in Finder'
  : /Win/i.test(navigator.platform)
    ? 'Show in Explorer'
    : 'Show in folder'

function Center({ job, live }: { job: JobView; live: Live | undefined }) {
  if (job.status === 'running' && live) {
    const r = rate(live.rate)
    const live1 = live.networks.filter((n) => !n.dead)
    const best = live1.reduce<(typeof live1)[number] | null>(
      (a, n) => (!a || n.rate > a.rate ? n : a),
      null,
    )
    const faster = live1.length > 1 && best && best.rate > 0 ? live.rate / best.rate : 0
    return (
      <>
        <p className="speed num">
          {r.value}
          <span className="unit">{r.unit}</span>
        </p>
        <p className="speed-sub">
          {faster >= 1.1 && best
            ? `${faster.toFixed(1)}x faster than ${netTitle(best)}`
            : `${live1.length} network${live1.length === 1 ? '' : 's'}`}
        </p>
      </>
    )
  }
  const pct = job.status === 'completed' ? 100 : percent(job.written, job.total)
  return (
    <>
      <p className="speed num">
        {pct === null ? '0' : Math.floor(pct)}
        <span className="unit">%</span>
      </p>
      <p className="speed-sub" data-status={job.status}>
        {STATUS_WORD[job.status]}
      </p>
    </>
  )
}

/** Speeds read out politely, at most every few seconds (DESIGN-SYSTEM.md §11). */
function useAnnounce(job: JobView, live: Live | undefined) {
  const [text, setText] = useState('')
  useEffect(() => {
    const t = setInterval(() => {
      const s = useApp.getState()
      const l = s.live[job.id]
      if (job.status === 'running' && l) {
        const p = percent(l.written, l.total)
        setText(`${p === null ? '' : `${Math.floor(p)} percent, `}${rateText(l.rate)}`)
      }
    }, 5000)
    return () => clearInterval(t)
  }, [job.id, job.status])
  useEffect(() => {
    setText(`${job.name}: ${STATUS_WORD[job.status]}`)
  }, [job.name, job.status])
  void live
  return text
}

function RemoveButton({ job }: { job: JobView }) {
  const act = useApp((s) => s.act)
  const [confirm, setConfirm] = useState(false)
  useEffect(() => {
    if (!confirm) return
    const t = setTimeout(() => setConfirm(false), 4000)
    return () => clearTimeout(t)
  }, [confirm])
  const done = job.status === 'completed'
  return (
    <button
      className={confirm ? 'btn btn-danger' : 'btn btn-ghost'}
      onClick={() => (confirm ? act((b) => b.remove(job.id)) : setConfirm(true))}
      aria-live="polite"
    >
      <Trash size={16} aria-hidden />
      {confirm ? (done ? 'Remove from list' : 'Delete partial file') : 'Remove'}
    </button>
  )
}

export function TransferDetail({ job, onBack }: { job: JobView; onBack: (() => void) | null }) {
  const liveAll = useApp((s) => s.live[job.id])
  const history = useApp((s) => s.history[job.id])
  const act = useApp((s) => s.act)
  const live = job.status === 'running' ? liveAll : undefined
  // After completion the last snapshot still tells who carried what.
  const finished = job.status === 'completed' ? liveAll : undefined
  const announce = useAnnounce(job, live)
  const written = live?.written ?? job.written
  const total = live?.total ?? job.total
  const nets = (live ?? finished)?.networks ?? []
  const lanes = assignLanes(nets)
  const sum = nets.reduce((a, n) => a + n.bytes, 0)
  const maxRate = Math.max(1, ...nets.map((n) => n.rate))
  const canPause = job.status === 'running' || job.status === 'queued'
  const left = eta((total ?? 0) - written, live?.rate ?? 0)

  return (
    <article className="detail" aria-labelledby="detail-title">
      <header className="detail-head">
        {onBack && (
          <button className="icon-btn" onClick={onBack} aria-label="Back to downloads">
            <ArrowLeft size={18} aria-hidden />
          </button>
        )}
        <div className="detail-title-wrap">
          <h1 id="detail-title" className="detail-title" title={job.name} translate="no">
            {job.name}
          </h1>
          <p className="detail-sub" title={job.finalPath ?? job.dir} translate="no">
            {job.finalPath ?? job.dir}
          </p>
        </div>
        <div className="detail-actions">
          {canPause && (
            <button className="btn" onClick={() => act((b) => b.pause(job.id))}>
              <Pause size={16} aria-hidden /> Pause
            </button>
          )}
          {job.resumable && (
            <button className="btn btn-primary" onClick={() => act((b) => b.resume(job.id))}>
              {job.status === 'failed' ? (
                <ArrowClockwise size={16} aria-hidden />
              ) : (
                <Play size={16} aria-hidden />
              )}
              {job.status === 'failed' ? 'Try again' : 'Resume'}
            </button>
          )}
          {job.status === 'completed' && (
            <>
              <button className="btn btn-primary" onClick={() => act((b) => b.openFile(job.id))}>
                <ArrowSquareOut size={16} aria-hidden /> Open
              </button>
              <button className="btn" onClick={() => act((b) => b.reveal(job.id))}>
                <FolderOpen size={16} aria-hidden /> {REVEAL_LABEL}
              </button>
            </>
          )}
          <RemoveButton job={job} />
        </div>
      </header>

      {job.error && job.status !== 'running' && (
        <div className="notice" role="alert">
          <WarningCircle size={18} weight="fill" aria-hidden className="ic-danger" />
          <p>{job.error}</p>
        </div>
      )}

      <p className="sr-only" aria-live="polite">
        {announce}
      </p>

      <div className="detail-body">
        <FuseCore job={job} live={live ?? finished} center={<Center job={job} live={live} />} />
        <div className="detail-side">
          <dl className="facts">
            <div>
              <dt>Saved</dt>
              <dd className="num">
                {bytes(job.status === 'completed' ? (total ?? written) : written)}
                {total && job.status !== 'completed' ? (
                  <span className="of"> of {bytes(total)}</span>
                ) : null}
              </dd>
            </div>
            <div>
              <dt>{job.status === 'running' ? 'Time left' : 'Status'}</dt>
              <dd className="num">
                {job.status === 'running' ? left || 'Working it out' : STATUS_WORD[job.status]}
              </dd>
            </div>
            {live && (
              <div>
                <dt>Streams</dt>
                <dd className="num">{nets.reduce((a, n) => a + n.streams, 0)}</dd>
              </div>
            )}
            {live && (live.retries > 0 || live.hedges > 0) && (
              <div>
                <dt>Recovered</dt>
                <dd className="num">
                  {live.retries} {live.retries === 1 ? 'retry' : 'retries'}, {live.hedges}{' '}
                  {live.hedges === 1 ? 'race' : 'races'}
                </dd>
              </div>
            )}
          </dl>

          {live && <Stream history={history} lanes={lanes} />}

          {nets.length > 0 && (
            <table className="nets">
              <caption className="sr-only">Networks in this download</caption>
              <thead>
                <tr>
                  <th scope="col">Network</th>
                  <th scope="col" className="r">
                    Share
                  </th>
                  <th scope="col" className="r">
                    {finished ? 'Carried' : 'Speed'}
                  </th>
                </tr>
              </thead>
              <tbody>
                {nets.map((n, i) => (
                  <tr key={n.name} data-down={n.dead || undefined}>
                    <td>
                      <span className="net-cell">
                        <Orb
                          lane={lanes[i] ?? 'steel'}
                          speed={n.rate / maxRate}
                          state={finished ? 'idle' : n.dead ? 'down' : 'live'}
                        />
                        <span>
                          <span className="net-name">{netTitle(n)}</span>
                          <span className="net-kind">
                            {kindLabel(n.kind)}, {n.name}
                          </span>
                        </span>
                      </span>
                    </td>
                    <td className="r num">
                      {sum > 0 ? `${Math.round((n.bytes / sum) * 100)}%` : ''}
                    </td>
                    <td className="r num">
                      {finished ? bytes(n.bytes) : n.dead ? 'Offline' : rateText(n.rate)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </article>
  )
}
