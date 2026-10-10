import { useEffect, useState } from 'react'
import {
  Funnel,
  CheckCircle,
  MagnifyingGlass,
  DownloadSimple,
  Lightning,
  Pause,
  Play,
  WarningCircle,
  HourglassMedium,
  Magnet,
  Plus,
  Rss,
  Stack,
  CaretRight,
  UploadSimple,
} from '@phosphor-icons/react'
import { useApp } from '../lib/store'
import { fileType, TYPE_LABEL, typeLabel, type FileType } from '../lib/categories'
import { bytes, percent, rateText, readyBy, startsAt } from '../lib/format'
import { assignLanes } from '../lib/lanes'
import type { JobView, Live, TorrentView } from '../lib/types'
import { statusWord } from './status'
import { mark, t } from '../lib/i18n'
import { FeedsDialog } from './FeedsDialog'
import { torrentWord, torrentNets } from './TorrentDetail'

function StatusIcon({ job }: { job: JobView }) {
  const p = { size: 18, 'aria-hidden': true } as const
  switch (job.status) {
    case 'completed':
      return <CheckCircle {...p} className="ic ic-success" weight="fill" />
    case 'running':
      return <DownloadSimple {...p} className="ic ic-fuse" />
    case 'paused':
      return <Pause {...p} className="ic" />
    case 'queued':
      return <HourglassMedium {...p} className="ic" />
    default:
      return <WarningCircle {...p} className="ic ic-danger" weight="fill" />
  }
}

/** A thin bar split by the networks' shares while running. */
function Bar({ job, live }: { job: JobView; live: Live | undefined }) {
  const written = live?.written ?? job.written
  const total = live?.total ?? job.total
  const pct = job.status === 'completed' ? 100 : (percent(written, total) ?? 0)
  const nets = live?.networks ?? []
  const lanes = assignLanes(nets)
  const sum = nets.reduce((a, n) => a + n.bytes, 0)
  return (
    <div className="bar" data-status={job.status}>
      {job.status === 'running' && sum > 0 ? (
        <div className="bar-fill" style={{ width: `${pct}%` }}>
          {nets.map((n, i) => (
            <span
              key={n.name}
              style={{ width: `${(n.bytes / sum) * 100}%`, background: `var(--lane-${lanes[i]})` }}
            />
          ))}
        </div>
      ) : (
        <div className="bar-fill" style={{ width: `${pct}%` }} />
      )}
    </div>
  )
}

function meta(job: JobView, live: Live | undefined): string {
  const written = live?.written ?? job.written
  const total = live?.total ?? job.total
  if (job.status === 'completed') return total ? bytes(total) : t('Saved')
  const pct = percent(written, total)
  const size = total
    ? t('{done} of {total}', { done: bytes(written), total: bytes(total) })
    : written > 0
      ? bytes(written)
      : ''
  const head = [pct !== null ? `${Math.floor(pct)}%` : '', size].filter(Boolean).join(', ')
  if (job.status === 'running' && live) return `${head}${head ? ', ' : ''}${rateText(live.rate)}`
  return head
}

function Row({ job }: { job: JobView }) {
  const live = useApp((s) => s.live[job.id])
  const selected = useApp((s) => s.selected === job.id)
  const select = useApp((s) => s.select)
  const act = useApp((s) => s.act)
  const showLive = job.status === 'running' ? live : undefined
  const canPause = job.status === 'running' || job.status === 'queued'
  return (
    <li className="row" data-selected={selected || undefined} data-status={job.status}>
      <button
        className="row-main"
        onClick={() => select(job.id)}
        aria-current={selected ? 'true' : undefined}
      >
        <StatusIcon job={job} />
        <span className="row-text">
          <span className="row-name" title={job.name} translate="no">
            {job.name}
          </span>
          <span className="row-meta">
            <span className="row-state">
              {job.focused && (
                <>
                  <Lightning
                    size={12}
                    weight="fill"
                    className="row-focus"
                    aria-label={t('Has every network')}
                  />{' '}
                </>
              )}
              {job.startAt ? startsAt(job.startAt) : statusWord(job.status)}
            </span>
            <span className="num">{meta(job, showLive)}</span>
          </span>
          {job.readyBy !== null && (
            <span className="row-ready" data-state={job.readyState ?? undefined}>
              {job.readyState === 'at-risk'
                ? t('{ready}, at risk', { ready: readyBy(job.readyBy) })
                : readyBy(job.readyBy)}
            </span>
          )}
          <Bar job={job} live={showLive} />
        </span>
      </button>
      {canPause || job.resumable ? (
        <button
          className="icon-btn row-action"
          aria-label={
            canPause
              ? t('Pause {name}', { name: job.name })
              : t('Resume {name}', { name: job.name })
          }
          title={canPause ? t('Pause') : t('Resume')}
          onClick={() => act((b) => (canPause ? b.pause(job.id) : b.resume(job.id)))}
        >
          {canPause ? <Pause size={16} aria-hidden /> : <Play size={16} aria-hidden />}
        </button>
      ) : (
        <span className="row-action-slot" aria-hidden="true" />
      )}
    </li>
  )
}

function torrentMeta(tor: TorrentView): string {
  if (tor.status === 'completed') return bytes(tor.total)
  if (tor.status === 'seeding')
    return t('{size}, shared {shared}', { size: bytes(tor.total), shared: bytes(tor.uploaded) })
  const pct = percent(tor.done, tor.total)
  const head = `${pct === null ? 0 : Math.floor(pct)}%, ${t('{done} of {total}', { done: bytes(tor.done), total: bytes(tor.total) })}`
  return tor.status === 'downloading' ? `${head}, ${rateText(tor.rate)}` : head
}

/** Rename a group, or let its downloads go back to being single ones. */
function GroupTools({ id, name }: { id: number; name: string }) {
  const act = useApp((s) => s.act)
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(name)
  if (editing)
    return (
      <form
        className="group-tools"
        onSubmit={(e) => {
          e.preventDefault()
          void act(async (b) => {
            await b.renameGroup(id, draft)
            setEditing(false)
          })
        }}
      >
        <input
          aria-label={t('Group name')}
          value={draft}
          maxLength={80}
          autoFocus
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === 'Escape' && setEditing(false)}
        />
        <button type="submit" className="btn btn-sm" disabled={!draft.trim()}>
          {t('Save')}
        </button>
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => setEditing(false)}>
          {t('Cancel')}
        </button>
      </form>
    )
  return (
    <div className="group-tools">
      <button
        type="button"
        className="link-btn"
        onClick={() => {
          setDraft(name)
          setEditing(true)
        }}
      >
        {t('Rename')}
      </button>
      <button type="button" className="link-btn" onClick={() => void act((b) => b.ungroup(id))}>
        {t('Ungroup')}
      </button>
    </div>
  )
}

/**
 * Links added together (B9.2): one row with one progress, opened to show each
 * download. Pause all / Resume all act on the whole group.
 */
function GroupRow({ id, name, jobs }: { id: number; name: string; jobs: JobView[] }) {
  const selected = useApp((s) => s.selected)
  const live = useApp((s) => s.live)
  const act = useApp((s) => s.act)
  const holdsSelected = jobs.some((j) => j.id === selected)
  const [open, setOpen] = useState(holdsSelected)
  // Picking one of its downloads (or adding the group) opens it.
  useEffect(() => {
    if (holdsSelected) setOpen(true)
  }, [holdsSelected])
  const done = jobs.filter((j) => j.status === 'completed').length
  const written = jobs.reduce(
    (a, j) =>
      a + (j.status === 'completed' ? (j.total ?? j.written) : (live[j.id]?.written ?? j.written)),
    0,
  )
  const sized = jobs.every((j) => j.total !== null)
  const total = jobs.reduce((a, j) => a + (j.total ?? 0), 0)
  const rate = jobs.reduce((a, j) => a + (j.status === 'running' ? (live[j.id]?.rate ?? 0) : 0), 0)
  const canPause = jobs.some((j) => j.status === 'running' || j.status === 'queued')
  const canResume = jobs.some((j) => j.resumable)
  const failed = jobs.some((j) => j.status === 'failed' || j.status === 'failed-final')
  const pct = done === jobs.length ? 100 : sized && total ? (written / total) * 100 : 0
  const status =
    done === jobs.length ? 'completed' : canPause ? 'running' : failed ? 'failed' : 'paused'
  const membersId = `group-${id}`
  return (
    <li className="group-block" data-open={open || undefined}>
      <div className="row group-row" data-status={status}>
        <button
          className="row-main"
          aria-expanded={open}
          aria-controls={membersId}
          onClick={() => setOpen(!open)}
        >
          <CaretRight size={14} aria-hidden className="group-caret" />
          <Stack size={18} aria-hidden className={done === jobs.length ? 'ic ic-success' : 'ic'} />
          <span className="row-text">
            <span className="row-name" title={name}>
              {name}
            </span>
            <span className="row-meta">
              <span className="row-state">
                {t('{done} of {total} done', { done, total: jobs.length })}
              </span>
              <span className="num">
                {sized
                  ? t('{done} of {total}', { done: bytes(written), total: bytes(total) })
                  : bytes(written)}
                {rate > 0 ? `, ${rateText(rate)}` : ''}
              </span>
            </span>
            <div className="bar" data-status={status}>
              <div className="bar-fill" style={{ width: `${pct}%` }} />
            </div>
          </span>
        </button>
        {canPause || canResume ? (
          <button
            className="icon-btn row-action"
            aria-label={
              canPause ? t('Pause all in {name}', { name }) : t('Resume all in {name}', { name })
            }
            title={canPause ? t('Pause all') : t('Resume all')}
            onClick={() => act((b) => (canPause ? b.pauseGroup(id) : b.resumeGroup(id)))}
          >
            {canPause ? <Pause size={16} aria-hidden /> : <Play size={16} aria-hidden />}
          </button>
        ) : (
          <span className="row-action-slot" aria-hidden="true" />
        )}
      </div>
      {open && <GroupTools id={id} name={name} />}
      {open && (
        <ul className="list group-members" id={membersId} aria-label={name}>
          {[...jobs]
            .sort((a, b) => a.position - b.position || a.id - b.id)
            .map((j) => (
              <Row key={j.id} job={j} />
            ))}
        </ul>
      )}
    </li>
  )
}

function TorrentRow({ t: tor }: { t: TorrentView }) {
  const selected = useApp((s) => s.selectedTorrent === tor.id)
  const select = useApp((s) => s.selectTorrent)
  const act = useApp((s) => s.act)
  const known = useApp((s) => s.networks)
  const nets = torrentNets(tor.networks, known)
  const lanes = assignLanes(nets)
  const pct =
    tor.status === 'completed' || tor.status === 'seeding'
      ? 100
      : (percent(tor.done, tor.total) ?? 0)
  const canPause = tor.status === 'downloading' || tor.status === 'checking'
  const canResume = tor.status === 'paused' || tor.status === 'failed'
  const status =
    tor.status === 'downloading' ? 'running' : tor.status === 'seeding' ? 'completed' : tor.status
  const p = { size: 18, 'aria-hidden': true } as const
  return (
    <li className="row" data-selected={selected || undefined} data-status={status}>
      <button
        className="row-main"
        onClick={() => select(tor.id)}
        aria-current={selected ? 'true' : undefined}
      >
        {tor.status === 'completed' ? (
          <CheckCircle {...p} className="ic ic-success" weight="fill" />
        ) : tor.status === 'seeding' ? (
          <UploadSimple {...p} className="ic ic-success" />
        ) : tor.status === 'failed' ? (
          <WarningCircle {...p} className="ic ic-danger" weight="fill" />
        ) : (
          <Magnet {...p} className={tor.status === 'downloading' ? 'ic ic-fuse' : 'ic'} />
        )}
        <span className="row-text">
          <span className="row-name" title={tor.name} translate="no">
            {tor.name}
          </span>
          <span className="row-meta">
            <span className="row-state">{torrentWord(tor.status)}</span>
            <span className="num">{torrentMeta(tor)}</span>
          </span>
          <div className="bar" data-status={status}>
            <div className="bar-fill" style={{ width: `${pct}%` }}>
              {tor.status === 'downloading' &&
                tor.done > 0 &&
                nets.map((n, i) => (
                  <span
                    key={n.name}
                    style={{
                      width: `${(n.credited / tor.done) * 100}%`,
                      background: `var(--lane-${lanes[i]})`,
                    }}
                  />
                ))}
            </div>
          </div>
        </span>
      </button>
      {canPause || canResume ? (
        <button
          className="icon-btn row-action"
          aria-label={
            canPause
              ? t('Pause {name}', { name: tor.name })
              : t('Resume {name}', { name: tor.name })
          }
          title={canPause ? t('Pause') : t('Resume')}
          onClick={() => act((b) => (canPause ? b.pauseTorrent(tor.id) : b.resumeTorrent(tor.id)))}
        >
          {canPause ? <Pause size={16} aria-hidden /> : <Play size={16} aria-hidden />}
        </button>
      ) : (
        <span className="row-action-slot" aria-hidden="true" />
      )}
    </li>
  )
}

type Filter = 'all' | 'active' | 'done' | 'failed'
const FILTERS: { id: Filter; label: string }[] = [
  { id: 'all', label: mark('All') },
  { id: 'active', label: mark('Active') },
  { id: 'done', label: mark('Finished') },
  { id: 'failed', label: mark('Failed') },
]

function jobKind(j: JobView): Filter {
  if (j.status === 'running' || j.status === 'queued' || j.status === 'paused') return 'active'
  if (j.status === 'completed') return 'done'
  return 'failed'
}

function torrentKind(tor: TorrentView): Filter {
  if (tor.status === 'completed' || tor.status === 'seeding') return 'done'
  if (tor.status === 'failed') return 'failed'
  return 'active'
}

export function TransferList() {
  const allJobs = useApp((s) => s.jobs)
  const allTorrents = useApp((s) => s.torrents)
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<Filter>('all')
  const [type, setType] = useState<FileType | 'all'>('all')
  const ready = useApp((s) => s.ready)
  const setAdding = useApp((s) => s.setAdding)
  const [feedsOpen, setFeedsOpen] = useState(false)
  const feedDraft = useApp((s) => s.feedDraft)
  useEffect(() => {
    if (feedDraft) setFeedsOpen(true)
  }, [feedDraft])
  const feedsDialog = <FeedsDialog open={feedsOpen} onClose={() => setFeedsOpen(false)} />
  if (!ready) {
    return (
      <ul className="list" aria-busy="true" aria-label={t('Loading downloads')}>
        {[0, 1, 2].map((i) => (
          <li key={i} className="row skeleton">
            <span className="sk sk-icon" />
            <span className="row-text">
              <span className="sk sk-line" />
              <span className="sk sk-line short" />
            </span>
          </li>
        ))}
      </ul>
    )
  }
  if (!allJobs.length && !allTorrents.length) {
    return (
      <div className="empty">
        <p className="empty-title">{t('Nothing downloading yet')}</p>
        <p className="empty-body">
          {t(
            'Paste a link or a magnet, or drop a .torrent file. Fuselane spreads it across every network you have, then fuses the parts into one file.',
          )}
        </p>
        <div className="empty-actions">
          <button className="btn btn-primary" onClick={() => setAdding(true)}>
            <Plus size={16} aria-hidden /> {t('New download')}
          </button>
          <button className="btn" onClick={() => setFeedsOpen(true)}>
            <Rss size={16} aria-hidden /> {t('Follow a feed')}
          </button>
        </div>
        {feedsDialog}
      </div>
    )
  }
  const q = query.trim().toLowerCase()
  const matches = (text: string) => !q || text.toLowerCase().includes(q)
  const counts: Record<Filter, number> = { all: 0, active: 0, done: 0, failed: 0 }
  for (const k of [...allJobs.map(jobKind), ...allTorrents.map(torrentKind)]) {
    counts.all++
    counts[k]++
  }
  // Types present, with how many of each, for the type filter (B8.8).
  const typeCounts = new Map<FileType, number>()
  for (const j of allJobs)
    typeCounts.set(fileType(j.name), (typeCounts.get(fileType(j.name)) ?? 0) + 1)
  if (allTorrents.length) typeCounts.set('torrents', allTorrents.length)
  const typeOk = (ft: FileType) => type === 'all' || type === ft
  const jobs = allJobs.filter(
    (j) =>
      (filter === 'all' || jobKind(j) === filter) &&
      typeOk(fileType(j.name)) &&
      (matches(j.name) || matches(j.url)),
  )
  const torrents = allTorrents.filter(
    (tor) =>
      (filter === 'all' || torrentKind(tor) === filter) && typeOk('torrents') && matches(tor.name),
  )
  // Groups (B9.2) show as one row each; their downloads aren't listed again below.
  const groups = new Map<number, { name: string; jobs: JobView[] }>()
  for (const j of jobs)
    if (j.groupId !== null) {
      const g = groups.get(j.groupId) ?? { name: j.groupName ?? '', jobs: [] }
      g.jobs.push(j)
      groups.set(j.groupId, g)
    }
  const single = jobs.filter((j) => j.groupId === null)
  // Running first, then the queue in the order it will start.
  const active = single
    .filter((j) => j.status === 'running' || j.status === 'queued')
    .sort((a, b) =>
      a.status === b.status ? a.position - b.position : a.status === 'running' ? -1 : 1,
    )
  const rest = single.filter((j) => !(j.status === 'running' || j.status === 'queued'))
  const nothing = !jobs.length && !torrents.length
  return (
    <div className="list-wrap">
      {feedsDialog}
      <div className="list-tools">
        <div className="list-tools-row">
          <label className="search">
            <MagnifyingGlass size={16} aria-hidden />
            <input
              type="search"
              name="search"
              autoComplete="off"
              spellCheck={false}
              placeholder={t('Search downloads…')}
              aria-label={t('Search downloads')}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Escape' && query) {
                  e.stopPropagation()
                  setQuery('')
                }
              }}
            />
          </label>
          <button
            type="button"
            className="btn btn-sm"
            title={t('Follow podcasts, releases and other feeds: new files download by themselves')}
            onClick={() => setFeedsOpen(true)}
          >
            <Rss size={16} aria-hidden /> {t('Feeds')}
          </button>
        </div>
        <div className="filters" role="radiogroup" aria-label={t('Show')}>
          {FILTERS.map((f) => (
            <button
              key={f.id}
              role="radio"
              aria-checked={filter === f.id}
              className="filter"
              onClick={() => setFilter(f.id)}
            >
              {t(f.label)} <span className="num">{counts[f.id]}</span>
            </button>
          ))}
          {typeCounts.size > 1 && (
            <label className="type-filter">
              <span className="sr-only">{t('Type')}</span>
              <Funnel size={14} aria-hidden />
              <select
                value={type}
                aria-label={t('Type')}
                onChange={(e) => setType(e.target.value as FileType | 'all')}
              >
                <option value="all">{t('All types')}</option>
                {(Object.keys(TYPE_LABEL) as FileType[])
                  .filter((ft) => typeCounts.has(ft))
                  .map((ft) => (
                    <option key={ft} value={ft}>
                      {typeLabel(ft)} ({typeCounts.get(ft)})
                    </option>
                  ))}
              </select>
            </label>
          )}
        </div>
      </div>
      {nothing && (
        <p className="list-empty" role="status">
          {q ? t('Nothing matches "{query}".', { query: query.trim() }) : t('Nothing here.')}{' '}
          <button
            className="link-btn"
            onClick={() => {
              setQuery('')
              setFilter('all')
              setType('all')
            }}
          >
            {t('Show everything')}
          </button>
        </p>
      )}
      {groups.size > 0 && (
        <section aria-labelledby="g-groups">
          <h2 id="g-groups" className="group">
            {t('Groups')} <span className="num">{groups.size}</span>
          </h2>
          <ul className="list">
            {[...groups].map(([id, g]) => (
              <GroupRow key={id} id={id} name={g.name} jobs={g.jobs} />
            ))}
          </ul>
        </section>
      )}
      {active.length > 0 && (
        <section aria-labelledby="g-active">
          <h2 id="g-active" className="group">
            {t('Active')} <span className="num">{active.length}</span>
          </h2>
          <ul className="list">
            {active.map((j) => (
              <Row key={j.id} job={j} />
            ))}
          </ul>
        </section>
      )}
      {torrents.length > 0 && (
        <section aria-labelledby="g-torrents">
          <h2 id="g-torrents" className="group">
            {t('Torrents')} <span className="num">{torrents.length}</span>
          </h2>
          <ul className="list">
            {torrents.map((tor) => (
              <TorrentRow key={tor.id} t={tor} />
            ))}
          </ul>
        </section>
      )}
      {rest.length > 0 && (
        <section aria-labelledby="g-rest">
          <h2 id="g-rest" className="group">
            {t('Recent')} <span className="num">{rest.length}</span>
          </h2>
          <ul className="list">
            {rest.map((j) => (
              <Row key={j.id} job={j} />
            ))}
          </ul>
        </section>
      )}
    </div>
  )
}
