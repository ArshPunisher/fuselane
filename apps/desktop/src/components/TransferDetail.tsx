import { useEffect, useState } from 'react'
import {
  ArrowLeft,
  ArrowClockwise,
  ArrowSquareOut,
  BatteryLow,
  FolderOpen,
  Lightning,
  Pause,
  ShareFat,
  ShieldCheck,
  ArrowLineUp,
  Play,
  Trash,
  WarningCircle,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { toUiError } from '../lib/backend'
import { bytes, eta, nextAt, percent, rate, rateText, readyBy, startsAt } from '../lib/format'
import { assignLanes, kindLabel, netTitle } from '../lib/lanes'
import { FuseCore } from './FuseCore'
import { Stream } from './Stream'
import { Orb } from './Orb'
import { statusWord } from './status'
import { LimitField } from './LimitField'
import { LiveRate } from './LiveRate'
import { BIN, RemoveDialog } from './RemoveDialog'
import { HandoffDialog } from './HandoffDialog'
import { NetworkNotes } from './NetworkNotes'
import type { JobView, Live, ReportView } from '../lib/types'
import { mark, t, tn, tr } from '../lib/i18n'

/** The platform's own words for showing a file in its folder (marked: show it with t()). */
export const REVEAL_LABEL = /Mac/i.test(navigator.platform)
  ? mark('Show in Finder')
  : /Win/i.test(navigator.platform)
    ? mark('Show in Explorer')
    : mark('Show in folder')

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
            ? t('{times}x faster than {network}', {
                times: faster.toFixed(1),
                network: netTitle(best),
              })
            : tn(live1.length, '{n} network', '{n} networks')}
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
        {job.startAt ? startsAt(job.startAt) : statusWord(job.status)}
      </p>
    </>
  )
}

/** Speeds read out politely, at most every few seconds (DESIGN-SYSTEM.md §11). */
function useAnnounce(job: JobView, live: Live | undefined) {
  const [text, setText] = useState('')
  useEffect(() => {
    const timer = setInterval(() => {
      const s = useApp.getState()
      const l = s.live[job.id]
      if (job.status === 'running' && l) {
        const p = percent(l.written, l.total)
        setText(
          p === null
            ? rateText(l.rate)
            : t('{p} percent, {rate}', { p: Math.floor(p), rate: rateText(l.rate) }),
        )
      }
    }, 5000)
    return () => clearInterval(timer)
  }, [job.id, job.status])
  useEffect(() => {
    setText(`${job.name}: ${statusWord(job.status)}`)
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
            {job.retryIn < 60
              ? t(
                  'Fuselane tries again by itself in under a minute, or as soon as a network comes back.',
                )
              : t(
                  'Fuselane tries again by itself in about {n} min, or as soon as a network comes back.',
                  { n: Math.round(job.retryIn / 60) },
                )}
          </p>
        )}
        {action === 'fix-link' && job.resumable && (
          <form className="fix-link" onSubmit={fix} noValidate>
            <label htmlFor={`fix-${job.id}`}>{t('New link to the same file')}</label>
            <div className="field-row">
              <input
                id={`fix-${job.id}`}
                type="url"
                inputMode="url"
                autoComplete="off"
                spellCheck={false}
                value={link}
                // i18n-ignore: a link
                placeholder="https://…"
                aria-invalid={problem ? true : undefined}
                aria-describedby={problem ? `fix-${job.id}-err` : `fix-${job.id}-help`}
                onChange={(e) => {
                  setLink(e.target.value)
                  setProblem(null)
                }}
              />
              <button type="submit" className="btn btn-primary" disabled={busy}>
                {busy ? t('Checking…') : t('Continue')}
              </button>
            </div>
            {problem ? (
              <p id={`fix-${job.id}-err`} className="field-error" aria-live="polite">
                {problem}
              </p>
            ) : (
              <p id={`fix-${job.id}-help`} className="field-help">
                {t(
                  "Saved progress is kept if it's the same file. A different file is never mixed in.",
                )}
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
            <ArrowClockwise size={16} aria-hidden /> {t('Start over')}
          </button>
        )}
        {action === 'allowance' && (
          <button className="btn" onClick={() => useApp.getState().setView('networks')}>
            {t('Open Networks')}
          </button>
        )}
        {action === 'free-space' && (
          <p className="field-help">
            {t('Free up space on that disk, or choose another folder, then try again.')}
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
      <LimitField
        label={t('Speed limit for this download')}
        rate={job.speedLimit}
        onChange={setDraft}
      />
      <button type="submit" className="btn" disabled={!changed}>
        {draft === 0 && job.speedLimit > 0 ? t('Remove limit') : t('Set limit')}
      </button>
      <p className="field-help">
        {t('Empty for no limit. The overall and network limits still apply.')}
      </p>
    </form>
  )
}

const READY_WORD = {
  'on-track': mark('On track'),
  'at-risk': mark('At risk: it goes first, and runs outside the schedule if it has to'),
  missed: mark('The time has passed; it carries on'),
} as const

/** Ready by (B9.4): a time this download should be finished. */
function ReadyBy({ job }: { job: JobView }) {
  const act = useApp((s) => s.act)
  const [draft, setDraft] = useState('')
  // A time field takes 24-hour "HH:MM" whatever the system's clock style.
  // Without a time yet, it offers two hours from now, on the hour.
  useEffect(() => {
    const d = job.readyBy ? new Date(job.readyBy * 1000) : new Date(Date.now() + 2 * 3600_000)
    if (!job.readyBy) d.setMinutes(0)
    setDraft(`${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`)
  }, [job.id, job.readyBy])
  const at = nextAt(draft)
  const id = `ready-${job.id}`
  return (
    <form
      className="job-limit ready-by"
      onSubmit={(e) => {
        e.preventDefault()
        if (at !== null) void act((b) => b.setReadyBy(job.id, at))
      }}
    >
      <div className="ready-field">
        <label htmlFor={id}>{t('Ready by')}</label>
        <input
          id={id}
          type="time"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          aria-describedby={`${id}-state`}
        />
      </div>
      <div className="ready-actions">
        <button
          type="submit"
          className="btn"
          disabled={at === null || (job.readyBy !== null && at === job.readyBy)}
        >
          {t('Set')}
        </button>
        {job.readyBy !== null && (
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => void act((b) => b.setReadyBy(job.id, null))}
          >
            {t('Clear')}
          </button>
        )}
      </div>
      <p id={`${id}-state`} className="field-help" data-state={job.readyState ?? undefined}>
        {job.readyBy && job.readyState
          ? t('{ready}. {state}.', {
              ready: readyBy(job.readyBy),
              state: t(READY_WORD[job.readyState]),
            })
          : t('Downloads with a time go first, earliest first.')}
      </p>
    </form>
  )
}

/** Continue on another computer (B9.9): a paused download with something saved. */
function HandoffButton({ job }: { job: JobView }) {
  const [open, setOpen] = useState(false)
  return (
    <>
      <button
        className="btn btn-ghost"
        title={t("Send it, with what's downloaded so far, to another computer with Fuselane")}
        onClick={() => setOpen(true)}
      >
        <ShareFat size={16} aria-hidden /> {t('Continue elsewhere')}
      </button>
      <HandoffDialog job={job} open={open} onClose={() => setOpen(false)} />
    </>
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
        {t('Remove')}
      </button>
      <RemoveDialog
        open={open}
        onClose={() => setOpen(false)}
        title={done ? t('Remove this download?') : t('Stop and remove this download?')}
        text={
          done
            ? t('{name} leaves your list. Keep the file, or move it to the {bin}?', {
                name: job.name,
                bin: t(BIN),
              })
            : t(
                "{name} stops and leaves your list. The unfinished file can't be used, so it's deleted.",
                { name: job.name },
              )
        }
        where={where}
        facts={
          done
            ? bytes(job.total ?? job.written)
            : job.total
              ? t('{done} of {total} downloaded', {
                  done: bytes(job.written),
                  total: bytes(job.total),
                })
              : t('{done} downloaded', { done: bytes(job.written) })
        }
        choices={
          done
            ? [
                { label: t('Keep file'), run: () => act((b) => b.remove(job.id)) },
                {
                  label: t('Move file to {bin}', { bin: t(BIN) }),
                  danger: true,
                  run: () => act((b) => b.trashFile(job.id)),
                },
              ]
            : [
                {
                  label: t('Delete unfinished file'),
                  danger: true,
                  run: () => act((b) => b.remove(job.id)),
                },
              ]
        }
      />
    </>
  )
}

/** "45 s", "4 min", "1 h 5 min": the same words as the notification. */
function took(secs: number): string {
  const s = Math.max(0, Math.round(secs))
  if (s < 60) return t('{n} s', { n: s })
  const m = Math.round(s / 60)
  if (m < 60) return t('{n} min', { n: m })
  return m % 60
    ? tn(Math.floor(m / 60), '1 h {m} min', '{n} h {m} min', { m: m % 60 })
    : tn(m / 60, '1 h', '{n} h')
}

/** After a download: what each network carried, and how long it would have taken without it. */
function Savings({ report }: { report: ReportView }) {
  const total = report.nets.reduce((a, n) => a + n.bytes, 0)
  const best = report.nets.reduce<ReportView['nets'][number] | null>(
    (a, n) => ((n.savedSecs ?? 0) > (a?.savedSecs ?? 0) ? n : a),
    null,
  )
  return (
    <section className="savings" aria-labelledby="savings-title">
      <h2 className="group" id="savings-title">
        {t('What each network saved')}
      </h2>
      <p className="savings-lead">
        {best?.savedSecs && best.savedSecs >= 30
          ? tr('Finished in {time}. Without {network} it would have taken about {longer}.', {
              time: <strong className="num">{took(report.secs)}</strong>,
              network: <span translate="no">{best.label}</span>,
              longer: <strong className="num">{took(report.secs + best.savedSecs)}</strong>,
            })
          : tr('Finished in {time}.', {
              time: <strong className="num">{took(report.secs)}</strong>,
            })}
      </p>
      <ul className="savings-list">
        {report.nets.map((n) => (
          <li key={n.label}>
            <span className="savings-name" translate="no">
              {n.label}
            </span>
            <span className="savings-bar" aria-hidden>
              <span style={{ width: `${total ? (n.bytes / total) * 100 : 0}%` }} />
            </span>
            <span className="num">{bytes(n.bytes)}</span>
            <span className="num savings-saved">
              {n.savedSecs === null ? '' : t('{time} saved', { time: took(n.savedSecs) })}
            </span>
          </li>
        ))}
      </ul>
    </section>
  )
}

/** Whether the file is (or will be) checked against a SHA-256, and from where. */
function ChecksumBadge({ job }: { job: JobView }) {
  const source = job.checksumFrom
  const against = t('against {source}', { source: source ?? t('your SHA-256') })
  if (job.verified)
    return (
      <p
        className="checksum-badge"
        data-verified
        title={
          source
            ? t('The finished file matches the SHA-256 from {source}.', { source })
            : t('The finished file matches the SHA-256 you gave.')
        }
      >
        <ShieldCheck size={14} weight="fill" aria-hidden /> {t('Verified')}
        <span className="muted">{against}</span>
      </p>
    )
  if (job.status === 'completed') return null
  return (
    <p
      className="checksum-badge"
      title={
        source
          ? t('When it finishes, the file is checked against the SHA-256 from {source}.', {
              source,
            })
          : t('When it finishes, the file is checked against the SHA-256 you gave.')
      }
    >
      <ShieldCheck size={14} aria-hidden /> {t('Checked when done')}
      <span className="muted">{against}</span>
    </p>
  )
}

const WAITS: JobView['errorAction'][] = ['focus', 'battery', 'handoff']

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
  // "Do this one now" only means something while something else wants the networks.
  const others = useApp(
    (s) =>
      s.jobs.filter((j) => j.id !== job.id && (j.status === 'running' || j.status === 'queued'))
        .length,
  )
  const canFocus =
    !job.focused && others > 0 && (canPause || (job.resumable && job.errorAction !== 'fix-link'))
  const left = eta((total ?? 0) - written, live?.rate ?? 0)

  return (
    <article className="detail" aria-labelledby="detail-title">
      <header className="detail-head">
        {onBack && (
          <button className="icon-btn" onClick={onBack} aria-label={t('Back to downloads')}>
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
          {job.verify && <ChecksumBadge job={job} />}
        </div>
        <div className="detail-actions">
          {job.status === 'queued' && (
            <button
              className="btn"
              title={t('Starts as soon as a download finishes')}
              onClick={() => act((b) => b.reorder([job.id]))}
            >
              <ArrowLineUp size={16} aria-hidden /> {t('Start next')}
            </button>
          )}
          {canFocus && (
            <button
              className="btn"
              title={t(
                'Every network goes to this download; the others wait and carry on after it',
              )}
              onClick={() => act((b) => b.focus(job.id))}
            >
              <Lightning size={16} aria-hidden /> {t('Do this now')}
            </button>
          )}
          {job.focused && (
            <button
              className="btn btn-focus"
              aria-pressed="true"
              title={t('Let the other downloads run again')}
              onClick={() => act((b) => b.unfocus())}
            >
              <Lightning size={16} weight="fill" aria-hidden /> {t('Every network')}
            </button>
          )}
          {canPause && (
            <button className="btn" onClick={() => act((b) => b.pause(job.id))}>
              <Pause size={16} aria-hidden /> {t('Pause')}
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
              {job.status === 'failed'
                ? t('Try again')
                : job.startAt
                  ? t('Start now')
                  : t('Resume')}
            </button>
          )}
          {job.status === 'completed' && (
            <>
              <button className="btn btn-primary" onClick={() => act((b) => b.openFile(job.id))}>
                <ArrowSquareOut size={16} aria-hidden /> {t('Open')}
              </button>
              <button className="btn" onClick={() => act((b) => b.reveal(job.id))}>
                <FolderOpen size={16} aria-hidden /> {t(REVEAL_LABEL)}
              </button>
            </>
          )}
          {job.status === 'paused' && job.written > 0 && <HandoffButton job={job} />}
          <RemoveButton job={job} />
        </div>
      </header>

      {/* Waiting for something that passes by itself: a calm note, not an error. */}
      {job.error && job.status !== 'running' && WAITS.includes(job.errorAction) && (
        <p className="notice notice-wait" role="status">
          {job.errorAction === 'battery' ? (
            <BatteryLow size={18} aria-hidden />
          ) : (
            <Lightning size={18} aria-hidden />
          )}
          <span>{job.error}</span>
        </p>
      )}
      {job.error && job.status !== 'running' && !WAITS.includes(job.errorAction) && (
        <ErrorPanel job={job} />
      )}

      <p className="sr-only" aria-live="polite">
        {announce}
      </p>

      {job.status === 'completed' && job.report && job.report.nets.length > 1 && (
        <Savings report={job.report} />
      )}

      <div className="detail-body">
        <FuseCore job={job} live={live ?? finished} center={<Center job={job} live={live} />} />
        <div className="detail-side">
          <dl className="facts">
            <div>
              <dt>{t('Saved')}</dt>
              <dd className="num">
                {bytes(job.status === 'completed' ? (total ?? written) : written)}
                {total && job.status !== 'completed' ? (
                  <span className="of"> {t('of {total}', { total: bytes(total) })}</span>
                ) : null}
              </dd>
            </div>
            <div>
              <dt>{job.status === 'running' ? t('Time left') : t('Status')}</dt>
              <dd className="num">
                {job.status === 'running' ? left || t('Working it out') : statusWord(job.status)}
              </dd>
            </div>
            {live && (
              <div>
                <dt>{t('Streams')}</dt>
                <dd className="num">{nets.reduce((a, n) => a + n.streams, 0)}</dd>
              </div>
            )}
            {live && (live.retries > 0 || live.hedges > 0) && (
              <div>
                <dt>{t('Recovered')}</dt>
                <dd className="num">
                  {tn(live.retries, '{n} retry', '{n} retries')},{' '}
                  {tn(live.hedges, '{n} race', '{n} races')}
                </dd>
              </div>
            )}
          </dl>

          {job.mirrors.length > 0 && (
            <div className="mirror-note">
              <p className="field-help">
                {job.mirrors.length === 1
                  ? tr(
                      'Also from a mirror: {mirrors}. Each was checked for the same file before helping.',
                      { mirrors: <span translate="no">{job.mirrors.join(', ')}</span> },
                    )
                  : tr(
                      'Also from {n} mirrors: {mirrors}. Each was checked for the same file before helping.',
                      {
                        n: job.mirrors.length,
                        mirrors: <span translate="no">{job.mirrors.join(', ')}</span>,
                      },
                    )}
              </p>
              {job.mirrorNotes.map((n) => (
                <p key={n} className="field-help warn">
                  {n}
                </p>
              ))}
            </div>
          )}

          <NetworkNotes notes={job.networkNotes} />

          {live && <Stream history={history} lanes={lanes} />}

          {nets.length > 0 && (
            <table className="nets">
              <caption className="sr-only">{t('Networks in this download')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('Network')}</th>
                  <th scope="col" className="r">
                    {t('Share')}
                  </th>
                  <th scope="col" className="r">
                    {finished ? t('Carried') : t('Speed')}
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
                      {finished ? bytes(n.bytes) : n.dead ? t('Offline') : rateText(n.rate)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {job.status !== 'completed' && job.status !== 'cancelled' && <JobLimit job={job} />}
          {job.status !== 'completed' && job.status !== 'cancelled' && <ReadyBy job={job} />}
        </div>
      </div>
    </article>
  )
}
