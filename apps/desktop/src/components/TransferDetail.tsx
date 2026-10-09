import { useEffect, useState } from 'react'
import {
  ArrowLeft,
  ArrowClockwise,
  ArrowSquareOut,
  FolderOpen,
  Pause,
  ArrowLineUp,
  Play,
  Trash,
  WarningCircle,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import { bytes, eta, percent, rate, rateText, startsAt } from '../lib/format'
import { assignLanes, kindLabel, netTitle } from '../lib/lanes'
import { FuseCore } from './FuseCore'
import { Stream } from './Stream'
import { Orb } from './Orb'
import { STATUS_WORD } from './status'
import { LimitField } from './LimitField'
import { LiveRate } from './LiveRate'
import { BIN, RemoveDialog } from './RemoveDialog'
import type { JobView, Live } from '../lib/types'

/** The platform's own words for showing a file in its folder. */
export const REVEAL_LABEL = /Mac/i.test(navigator.platform)
  ? 'Show in Finder'
  : /Win/i.test(navigator.platform)
    ? 'Show in Explorer'
    : 'Show in folder'

function Center({ job, live }: { job: JobView; live: Live | undefined }) {
  if (job.status === 'running' && live) {
    const live1 = live.networks.filter((n) => !n.dead)
    const best = live1.reduce<(typeof live1)[number] | null>(
      (a, n) => (!a || n.rate > a.rate ? n : a),
      null,
    )
    // The centre number is the sum of the speeds shown around the ring (B8.1).
    const total = live1.reduce((a, n) => a + n.rate, 0)
    const faster = live1.length > 1 && best && best.rate > 0 ? total / best.rate : 0
    return (
      <>
        <LiveRate value={total} />
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
        {job.startAt ? startsAt(job.startAt) : STATUS_WORD[job.status]}
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

/** The failure in plain words plus the one fix that fits it (ERRORS.md §2). */
function ErrorPanel({ job }: { job: JobView }) {
  const backend = useApp((s) => s.backend)
  const act = useApp((s) => s.act)
  const select = useApp((s) => s.select)
  const [link, setLink] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const action = job.errorAction

  async function fix(e: React.FormEvent) {
    e.preventDefault()
    if (!backend || busy) return
    setBusy(true)
    setProblem(null)
    try {
      await backend.fixLink(job.id, link)
      setLink('')
    } catch (err) {
      const u = toUiError(err)
      setProblem([u.message, u.hint].filter(Boolean).join(' '))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="notice" role="alert">
      <WarningCircle size={18} weight="fill" aria-hidden className="ic-danger" />
      <div className="notice-body">
        <p>{job.error}</p>
        {job.retryIn !== null && (
          <p className="field-help">
            Fuselane tries again by itself{' '}
            {job.retryIn < 60
              ? 'in under a minute'
              : `in about ${Math.round(job.retryIn / 60)} min`}
            , or as soon as a network comes back.
          </p>
        )}
        {action === 'fix-link' && job.resumable && (
          <form className="fix-link" onSubmit={fix} noValidate>
            <label htmlFor={`fix-${job.id}`}>New link to the same file</label>
            <div className="field-row">
              <input
                id={`fix-${job.id}`}
                type="url"
                inputMode="url"
                autoComplete="off"
                spellCheck={false}
                value={link}
                placeholder="https://…"
                aria-invalid={problem ? true : undefined}
                aria-describedby={problem ? `fix-${job.id}-err` : `fix-${job.id}-help`}
                onChange={(e) => {
                  setLink(e.target.value)
                  setProblem(null)
                }}
              />
              <button type="submit" className="btn btn-primary" disabled={busy}>
                {busy ? 'Checking…' : 'Continue'}
              </button>
            </div>
            {problem ? (
              <p id={`fix-${job.id}-err`} className="field-error" aria-live="polite">
                {problem}
              </p>
            ) : (
              <p id={`fix-${job.id}-help`} className="field-help">
                Saved progress is kept if it's the same file. A different file is never mixed in.
              </p>
            )}
          </form>
        )}
        {action === 'start-over' && (
          <button
            className="btn"
            onClick={() =>
              act(async (b) => {
                select(await b.startOver(job.id))
              })
            }
          >
            <ArrowClockwise size={16} aria-hidden /> Start over
          </button>
        )}
        {action === 'allowance' && (
          <button className="btn" onClick={() => useApp.getState().setView('networks')}>
            Open Networks
          </button>
        )}
        {action === 'free-space' && (
          <p className="field-help">
            Free up space on that disk, or choose another folder, then try again.
          </p>
        )}
      </div>
    </div>
  )
}

/** This download's own speed limit; other limits still apply on top. */
function JobLimit({ job }: { job: JobView }) {
  const act = useApp((s) => s.act)
  const [draft, setDraft] = useState<number | null>(job.speedLimit)
  useEffect(() => setDraft(job.speedLimit), [job.id, job.speedLimit])
  const changed = draft !== null && draft !== job.speedLimit
  return (
    <form
      className="job-limit"
      onSubmit={(e) => {
        e.preventDefault()
        if (changed) void act((b) => b.setJobLimit(job.id, draft))
      }}
    >
      <LimitField label="Speed limit for this download" rate={job.speedLimit} onChange={setDraft} />
      <button type="submit" className="btn" disabled={!changed}>
        {draft === 0 && job.speedLimit > 0 ? 'Remove limit' : 'Set limit'}
      </button>
      <p className="field-help">Empty for no limit. The overall and network limits still apply.</p>
    </form>
  )
}

function RemoveButton({ job }: { job: JobView }) {
  const act = useApp((s) => s.act)
  const [open, setOpen] = useState(false)
  const done = job.status === 'completed'
  const where = job.finalPath ?? `${job.dir}/${job.name}`
  return (
    <>
      <button className="btn btn-ghost" onClick={() => setOpen(true)}>
        <Trash size={16} aria-hidden />
        Remove
      </button>
      <RemoveDialog
        open={open}
        onClose={() => setOpen(false)}
        title={done ? 'Remove this download?' : 'Stop and remove this download?'}
        text={
          done
            ? `${job.name} leaves your list. Keep the file, or move it to the ${BIN}?`
            : `${job.name} stops and leaves your list. The unfinished file can't be used, so it's deleted.`
        }
        where={where}
        facts={
          done
            ? bytes(job.total ?? job.written)
            : `${bytes(job.written)}${job.total ? ` of ${bytes(job.total)}` : ''} downloaded`
        }
        choices={
          done
            ? [
                { label: 'Keep file', run: () => act((b) => b.remove(job.id)) },
                {
                  label: `Move file to ${BIN}`,
                  danger: true,
                  run: () => act((b) => b.trashFile(job.id)),
                },
              ]
            : [
                {
                  label: 'Delete unfinished file',
                  danger: true,
                  run: () => act((b) => b.remove(job.id)),
                },
              ]
        }
      />
    </>
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
          {job.status === 'queued' && (
            <button
              className="btn"
              title="Starts as soon as a download finishes"
              onClick={() => act((b) => b.reorder([job.id]))}
            >
              <ArrowLineUp size={16} aria-hidden /> Start next
            </button>
          )}
          {canPause && (
            <button className="btn" onClick={() => act((b) => b.pause(job.id))}>
              <Pause size={16} aria-hidden /> Pause
            </button>
          )}
          {job.resumable && (
            <button
              // One primary action at a time: a fresh link is the real fix for an expired one.
              className={job.errorAction === 'fix-link' ? 'btn' : 'btn btn-primary'}
              onClick={() => act((b) => b.resume(job.id))}
            >
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

      {job.error && job.status !== 'running' && <ErrorPanel job={job} />}

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

          {job.status !== 'completed' && job.status !== 'cancelled' && <JobLimit job={job} />}
        </div>
      </div>
    </article>
  )
}
